//! Sources web d'une série (lot 5.5b, migration 0010) : liens, candidats à confirmer, échecs.
//! Les fiches gardées alimentent la fusion ([`crate::metadata`]) sans nouvel appel réseau.

use serde::{Deserialize, Serialize};
use shiori_meta::{SourceId, WebMetadata};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, from_db, to_db};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchedBy {
    /// Rapprochement automatique (Q30).
    Auto,
    /// Choisi par l'utilisateur.
    User,
}

/// État d'une source pour une série.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SourceState {
    pub source: SourceId,
    /// Fiche liée ; `None` : pas (encore) liée.
    pub external_id: Option<String>,
    pub matched_by: Option<MatchedBy>,
    pub checked_at: String,
    /// Code stable de la dernière tentative ratée (`offline`, `rate_limited`…).
    pub last_error: Option<String>,
    /// Fiches possibles quand le rapprochement n'a pas pu choisir ; vide : rien trouvé ou liée.
    /// Sans adresse de couverture : le navigateur ne contacte jamais les sources.
    pub candidates: Vec<WebMetadata>,
}

/// Ce qu'une recherche ou une lecture a donné, à enregistrer.
#[derive(Debug, Clone)]
pub enum Outcome {
    Linked {
        external_id: String,
        matched_by: MatchedBy,
        data: Box<WebMetadata>,
    },
    /// Recherche sans choix possible : fiches à confirmer (vide : rien trouvé).
    Candidates(Vec<WebMetadata>),
    /// Code stable de l'échec ; une source liée garde sa dernière fiche.
    Failed(&'static str),
}

/// Une série à identifier : son id et les noms qui servent à la chercher et à la rapprocher
/// (titre affiché d'abord).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToIdentify {
    pub series_id: i64,
    pub names: Vec<String>,
}

/// Une source qui demande l'attention : candidats à confirmer ou dernier essai raté.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SourceIssue {
    pub series_id: i64,
    pub series_name: String,
    pub source: SourceId,
    pub candidates: usize,
    pub last_error: Option<String>,
}

fn json<T: serde::de::DeserializeOwned>(text: Option<&str>) -> Result<Option<T>, Error> {
    text.map(serde_json::from_str)
        .transpose()
        .map_err(|e| Error::Decode(Box::new(e)))
}

fn to_json<T: Serialize>(value: &T) -> String {
    serde_json::to_string(value).expect("une fiche se sérialise")
}

fn state(row: &SqliteRow) -> Result<SourceState, Error> {
    let mut candidates: Vec<WebMetadata> = json(row.get("candidates"))?.unwrap_or_default();
    for c in &mut candidates {
        c.cover_url = None;
    }
    Ok(SourceState {
        source: from_db(row.get("source"))?,
        external_id: row.get("external_id"),
        matched_by: row
            .get::<Option<&str>, _>("matched_by")
            .map(from_db)
            .transpose()?,
        checked_at: row.get("checked_at"),
        last_error: row.get("last_error"),
        candidates,
    })
}

impl Db {
    /// Enregistre le résultat d'une recherche ou d'une lecture, puis recalcule la série ; sans
    /// effet si la série n'existe plus.
    pub async fn record_source(
        &self,
        series: i64,
        source: SourceId,
        outcome: Outcome,
    ) -> Result<(), Error> {
        let source = to_db(&source);
        let query = match &outcome {
            Outcome::Linked {
                external_id,
                matched_by,
                data,
            } => sqlx::query(concat!(
                "INSERT INTO series_sources (series_id, source, external_id, matched_by, data, checked_at)
                 SELECT ?1, ?2, ?3, ?4, ?5, ",
                now!(),
                " WHERE EXISTS (SELECT 1 FROM series WHERE id = ?1) ON CONFLICT (series_id, source) DO UPDATE SET external_id = excluded.external_id,
                   matched_by = excluded.matched_by, data = excluded.data, candidates = NULL,
                   checked_at = excluded.checked_at, last_error = NULL"
            ))
            .bind(series)
            .bind(source)
            .bind(external_id)
            .bind(to_db(matched_by))
            .bind(to_json(data)),
            Outcome::Candidates(candidates) => sqlx::query(concat!(
                "INSERT INTO series_sources (series_id, source, candidates, checked_at) SELECT ?1, ?2, ?3, ",
                now!(),
                " WHERE EXISTS (SELECT 1 FROM series WHERE id = ?1) ON CONFLICT (series_id, source) DO UPDATE SET external_id = NULL, matched_by = NULL,
                   data = NULL, candidates = excluded.candidates, checked_at = excluded.checked_at,
                   last_error = NULL"
            ))
            .bind(series)
            .bind(source)
            .bind(to_json(candidates)),
            Outcome::Failed(code) => sqlx::query(concat!(
                "INSERT INTO series_sources (series_id, source, last_error, checked_at) SELECT ?1, ?2, ?3, ",
                now!(),
                " WHERE EXISTS (SELECT 1 FROM series WHERE id = ?1) ON CONFLICT (series_id, source) DO UPDATE SET last_error = excluded.last_error,
                   checked_at = excluded.checked_at"
            ))
            .bind(series)
            .bind(source)
            .bind(*code),
        };
        // Série supprimée entre-temps (scan pendant une identification) : rien à noter.
        if query.execute(&self.pool).await?.rows_affected() == 0 {
            return Ok(());
        }
        self.refresh_series_metadata(series).await
    }

    /// Délie une source (sa fiche ne compte plus) ; `false` si elle n'était pas enregistrée.
    pub async fn unlink_source(&self, series: i64, source: SourceId) -> Result<bool, Error> {
        let done = sqlx::query("DELETE FROM series_sources WHERE series_id = ? AND source = ?")
            .bind(series)
            .bind(to_db(&source))
            .execute(&self.pool)
            .await?;
        self.refresh_series_metadata(series).await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn source_states(&self, series: i64) -> Result<Vec<SourceState>, Error> {
        sqlx::query(
            "SELECT source, external_id, matched_by, checked_at, last_error, candidates
             FROM series_sources WHERE series_id = ? ORDER BY source",
        )
        .bind(series)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(state)
        .collect()
    }

    /// Séries à identifier chez `source` : jamais cherchées, ou dont le dernier essai a raté
    /// sans lien ; `series` restreint à une série (bouton « Identifier »), `None` : toutes.
    pub async fn to_identify(
        &self,
        source: SourceId,
        series: Option<i64>,
    ) -> Result<Vec<ToIdentify>, Error> {
        let rows = sqlx::query(
            "SELECT s.id, s.name, m.merged FROM series s
             LEFT JOIN series_metadata m ON m.series_id = s.id
             LEFT JOIN series_sources w ON w.series_id = s.id AND w.source = ?1
             WHERE (?2 IS NULL OR s.id = ?2)
               AND (w.series_id IS NULL OR (w.external_id IS NULL AND w.last_error IS NOT NULL))
             ORDER BY s.id",
        )
        .bind(to_db(&source))
        .bind(series)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                let merged: crate::SeriesMetadata = json(r.get("merged"))?.unwrap_or_default();
                let mut names: Vec<String> = merged.title.into_iter().collect();
                names.push(r.get("name"));
                names.extend(merged.alt_titles);
                names.dedup();
                Ok(ToIdentify {
                    series_id: r.get("id"),
                    names,
                })
            })
            .collect()
    }

    /// Fiches liées d'une série (source, identifiant), pour les relire.
    pub async fn linked_sources(&self, series: i64) -> Result<Vec<(SourceId, String)>, Error> {
        sqlx::query(
            "SELECT source, external_id FROM series_sources
             WHERE series_id = ? AND external_id IS NOT NULL ORDER BY source",
        )
        .bind(series)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| Ok((from_db(r.get("source"))?, r.get("external_id"))))
        .collect()
    }

    /// Sources qui demandent l'attention, toutes séries confondues : à confirmer, ou en échec.
    pub async fn source_issues(&self) -> Result<Vec<SourceIssue>, Error> {
        sqlx::query(
            "SELECT w.series_id, COALESCE(json_extract(m.merged, '$.title'), s.name) AS name,
                    w.source, w.candidates, w.last_error
             FROM series_sources w JOIN series s ON s.id = w.series_id
             LEFT JOIN series_metadata m ON m.series_id = s.id
             WHERE w.last_error IS NOT NULL OR (w.external_id IS NULL AND w.candidates <> '[]')
             ORDER BY name, w.source",
        )
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| {
            let candidates: Vec<serde_json::Value> = json(r.get("candidates"))?.unwrap_or_default();
            Ok(SourceIssue {
                series_id: r.get("series_id"),
                series_name: r.get("name"),
                source: from_db(r.get("source"))?,
                candidates: candidates.len(),
                last_error: r.get("last_error"),
            })
        })
        .collect()
    }
}

const MAX_COVER_SIDE: usize = 4096;

/// Couverture gardée : type d'image, largeur, contenu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cover {
    pub media_type: &'static str,
    pub width: u32,
    pub bytes: Vec<u8>,
}

impl Db {
    /// Garde une couverture téléchargée et rend son identifiant (hash du contenu) ; `None` si
    /// ce n'est pas une image JPEG, PNG ou WebP lisible, de 4 096 px de côté au plus.
    // shortcut: une couverture remplacée ou d'une source déliée reste en base (quelques centaines de Ko) ; purger celles qu'aucune série ne référence si la base grossit.
    pub async fn store_cover(&self, bytes: Vec<u8>) -> Result<Option<String>, Error> {
        use imagesize::ImageType;
        let media_type = match imagesize::image_type(&bytes) {
            Ok(ImageType::Jpeg) => "image/jpeg",
            Ok(ImageType::Png) => "image/png",
            Ok(ImageType::Webp) => "image/webp",
            _ => return Ok(None),
        };
        // Image venue du réseau : au-delà de 4 096 px de côté, la décoder pour un rendu pourrait
        // épuiser la mémoire (un PNG de quelques Mo peut annoncer 20 000 × 20 000 px).
        let Ok(size) = imagesize::blob_size(&bytes) else {
            return Ok(None);
        };
        if size.width > MAX_COVER_SIDE || size.height > MAX_COVER_SIDE {
            return Ok(None);
        }
        let id = blake3::hash(&bytes).to_hex().to_string();
        sqlx::query(concat!(
            "INSERT INTO covers (id, media_type, width, height, data, created_at) VALUES (?, ?, ?, ?, ?, ",
            now!(),
            ") ON CONFLICT (id) DO NOTHING"
        ))
        .bind(&id)
        .bind(media_type)
        .bind(size.width as i64)
        .bind(size.height as i64)
        .bind(bytes)
        .execute(&self.pool)
        .await?;
        Ok(Some(id))
    }

    pub async fn cover(&self, id: &str) -> Result<Option<Cover>, Error> {
        let row = sqlx::query("SELECT media_type, width, data FROM covers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.map(|r| Cover {
            media_type: match r.get::<&str, _>("media_type") {
                "image/png" => "image/png",
                "image/webp" => "image/webp",
                _ => "image/jpeg",
            },
            width: r.get::<i64, _>("width") as u32,
            bytes: r.get("data"),
        }))
    }

    /// Adresse de couverture d'une fiche déjà vue (liée ou candidate) chez `source`.
    pub async fn cover_url(
        &self,
        source: SourceId,
        external_id: &str,
    ) -> Result<Option<String>, Error> {
        let rows = sqlx::query("SELECT data, candidates FROM series_sources WHERE source = ?")
            .bind(to_db(&source))
            .fetch_all(&self.pool)
            .await?;
        for row in rows {
            let linked: Option<WebMetadata> = json(row.get("data"))?;
            let candidates: Vec<WebMetadata> = json(row.get("candidates"))?.unwrap_or_default();
            if let Some(found) = linked
                .into_iter()
                .chain(candidates)
                .find(|w| w.id == external_id)
            {
                return Ok(found.cover_url);
            }
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests;
