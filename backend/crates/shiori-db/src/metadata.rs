//! Métadonnées éditables des séries et des tomes (lot 5.3) : la vue des sources, recouverte
//! par les champs édités à la main. Un champ édité est verrouillé : aucune source ne le
//! remplace tant qu'il n'est pas rendu (`unlock`).

use std::collections::HashMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use shiori_core::{Author, LocalMetadata, MetadataSource, VolumeId};
use sqlx::Row;

use crate::{Db, Error, PublicationStatus, from_db, to_db};

/// Champs édités : nom → valeur (`null` compris, c'est une valeur verrouillée).
pub type Edited = Map<String, Value>;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct SeriesMetadata {
    /// Titre affiché ; vide : le nom tiré du dossier.
    pub title: Option<String>,
    pub alt_titles: Vec<String>,
    pub summary: Option<String>,
    pub authors: Vec<Author>,
    pub genres: Vec<String>,
    pub tags: Vec<String>,
    pub year: Option<i32>,
    pub publisher: Option<String>,
    pub age_rating: Option<String>,
    pub publication_status: Option<PublicationStatus>,
    pub links: Vec<String>,
    /// Page choisie comme couverture ; vide : page 0 du premier tome.
    pub cover_page_id: Option<String>,
    /// Sens de lecture proposé par les fichiers : une suggestion, jamais imposée, pas éditable.
    pub right_to_left: Option<bool>,
}

const SERIES_FIELDS: &[&str] = &[
    "title",
    "alt_titles",
    "summary",
    "authors",
    "genres",
    "tags",
    "year",
    "publisher",
    "age_rating",
    "publication_status",
    "links",
    "cover_page_id",
];

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct VolumeMetadata {
    pub title: Option<String>,
    /// Numéro écrit dans le fichier : affiché seulement, l'ordre vient du scan (principe 3).
    pub number: Option<String>,
    pub summary: Option<String>,
    pub release_date: Option<String>,
    pub authors: Vec<Author>,
}

const VOLUME_FIELDS: &[&str] = &["title", "summary", "release_date", "authors"];

#[derive(Debug, thiserror::Error)]
pub enum EditError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Db(#[from] Error),
}

/// Vue des fichiers locaux d'une série nommée `name` (`locals` dans l'ordre de ses tomes).
/// Pour chaque champ : le premier `index.json` qui l'a (il décrit la série), sinon le premier
/// `ComicInfo.xml`. Titre : celui des fichiers, sinon (plus tard le web, puis) le nom du
/// dossier ; les autres titres, nom du dossier compris, restent cherchables en alternatifs.
fn from_locals(name: &str, locals: &[(MetadataSource, LocalMetadata)]) -> SeriesMetadata {
    let of = |source| locals.iter().filter(move |(s, _)| *s == source);
    let ordered: Vec<&LocalMetadata> = of(MetadataSource::IndexJson)
        .chain(of(MetadataSource::Comicinfo))
        .map(|(_, m)| m)
        .collect();
    let text = |f: fn(&LocalMetadata) -> &Option<String>| ordered.iter().find_map(|m| f(m).clone());
    let list = |f: fn(&LocalMetadata) -> &Vec<String>| {
        ordered
            .iter()
            .map(|m| f(m))
            .find(|l| !l.is_empty())
            .cloned()
            .unwrap_or_default()
    };
    let title = text(|m| &m.series);
    let shown = title.as_deref().unwrap_or(name).to_owned();
    let mut alt_titles: Vec<String> = Vec::new();
    let candidates = ordered
        .iter()
        .flat_map(|m| m.series.iter().chain(&m.alt_titles));
    for t in candidates.chain([&name.to_owned()]) {
        let known = |a: &String| a.eq_ignore_ascii_case(t);
        if !t.eq_ignore_ascii_case(&shown) && !alt_titles.iter().any(known) {
            alt_titles.push(t.clone());
        }
    }
    SeriesMetadata {
        title,
        alt_titles,
        summary: text(|m| &m.summary),
        authors: ordered
            .iter()
            .map(|m| &m.authors)
            .find(|a| !a.is_empty())
            .cloned()
            .unwrap_or_default(),
        genres: list(|m| &m.genres),
        tags: list(|m| &m.tags),
        year: ordered
            .iter()
            .find_map(|m| m.release_date.as_ref()?.get(..4)?.parse().ok()),
        publisher: text(|m| &m.publisher),
        age_rating: text(|m| &m.age_rating),
        publication_status: ordered
            .iter()
            .find_map(|m| from_db(m.status.as_deref()?).ok()),
        links: list(|m| &m.links),
        cover_page_id: None,
        right_to_left: ordered.iter().find_map(|m| m.right_to_left),
    }
}

/// Vue d'un tome : son `ComicInfo.xml` (l'`index.json` décrit la série, pas le tome).
fn volume_from_locals(locals: &[(MetadataSource, LocalMetadata)]) -> VolumeMetadata {
    let Some((_, m)) = locals.iter().find(|(s, _)| *s == MetadataSource::Comicinfo) else {
        return VolumeMetadata::default();
    };
    VolumeMetadata {
        title: m.title.clone(),
        number: m.number.clone(),
        summary: m.summary.clone(),
        release_date: m.release_date.clone(),
        authors: m.authors.clone(),
    }
}

/// `base` recouvert par les champs édités ; une valeur du mauvais type est une erreur.
fn overlay<T: Serialize + DeserializeOwned>(base: T, edited: &Edited) -> Result<T, String> {
    let mut value = serde_json::to_value(base).expect("une structure se sérialise");
    if let Value::Object(fields) = &mut value {
        fields.extend(edited.clone());
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Chaque champ de `patch` est édité (donc verrouillé) ; ceux de `unlock` sont rendus aux sources.
fn apply(edited: &mut Edited, patch: Edited, fields: &[&str]) -> Result<(), String> {
    let known = |name: &str| match fields.contains(&name) {
        true => Ok(()),
        false => Err(format!("champ inconnu : {name}")),
    };
    for (name, value) in patch {
        if name == "unlock" {
            let names: Vec<String> = serde_json::from_value(value)
                .map_err(|_| "unlock : liste de noms de champs attendue".to_owned())?;
            for name in names {
                known(&name)?;
                edited.remove(&name);
            }
        } else {
            known(&name)?;
            edited.insert(name, tidy(value));
        }
    }
    Ok(())
}

/// Texte saisi sans espaces autour ; un texte vide vaut `null` (le titre retombe alors sur
/// le nom du dossier au lieu d'afficher une série sans nom), un élément de liste vide disparaît.
fn tidy(value: Value) -> Value {
    match value {
        Value::String(s) if s.trim().is_empty() => Value::Null,
        Value::String(s) => Value::String(s.trim().to_owned()),
        Value::Array(items) => Value::Array(
            items
                .into_iter()
                .map(tidy)
                .filter(|v| !v.is_null())
                .collect(),
        ),
        other => other,
    }
}

fn parse_edited(text: Option<&str>) -> Result<Edited, Error> {
    serde_json::from_str(text.unwrap_or("{}")).map_err(|e| Error::Decode(Box::new(e)))
}

fn decode(e: String) -> Error {
    Error::Decode(e.into())
}

fn local(row: &sqlx::sqlite::SqliteRow) -> Result<(MetadataSource, LocalMetadata), Error> {
    let data = serde_json::from_str(row.get("data")).map_err(|e| Error::Decode(Box::new(e)))?;
    Ok((from_db(row.get("source"))?, data))
}

impl Db {
    /// Recalcule la vue fusionnée des séries d'une bibliothèque (toutes si `None`).
    pub async fn refresh_metadata(&self, library: Option<i64>) -> Result<(), Error> {
        self.refresh(library, None).await
    }

    async fn refresh(&self, library: Option<i64>, series: Option<i64>) -> Result<(), Error> {
        macro_rules! filter {
            () => {
                "(?1 IS NULL OR s.library_id = ?1) AND (?2 IS NULL OR s.id = ?2)"
            };
        }
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query(concat!(
            "SELECT s.id, s.name, m.edited FROM series s
             LEFT JOIN series_metadata m ON m.series_id = s.id WHERE ",
            filter!()
        ))
        .bind(library)
        .bind(series)
        .fetch_all(&mut *tx)
        .await?;
        let mut locals: HashMap<i64, Vec<(MetadataSource, LocalMetadata)>> = HashMap::new();
        // Ordre des tomes approché par leur numéro puis leur chemin : il ne départage que des
        // fichiers qui se contredisent.
        let sql = concat!(
            "SELECT l.series_id, v.source, v.data FROM file_locations l
             JOIN volume_metadata v ON v.volume_id = l.volume_id
             JOIN series s ON s.id = l.series_id WHERE ",
            filter!(),
            " ORDER BY l.number IS NULL, l.number, l.path"
        );
        for row in sqlx::query(sql)
            .bind(library)
            .bind(series)
            .fetch_all(&mut *tx)
            .await?
        {
            locals
                .entry(row.get("series_id"))
                .or_default()
                .push(local(&row)?);
        }
        for row in rows {
            let id: i64 = row.get("id");
            let base = from_locals(row.get("name"), locals.get(&id).map_or(&[], Vec::as_slice));
            let mut merged = overlay(base, &parse_edited(row.get("edited"))?).map_err(decode)?;
            // Une couverture choisie disparaît avec sa page (fichier modifié : nouveaux IDs).
            if let Some(page) = &merged.cover_page_id
                && !page_in_series(&mut tx, page, id).await?
            {
                merged.cover_page_id = None;
            }
            sqlx::query(
                "INSERT INTO series_metadata (series_id, merged) VALUES (?, ?)
                 ON CONFLICT (series_id) DO UPDATE SET merged = excluded.merged",
            )
            .bind(id)
            .bind(serde_json::to_string(&merged).expect("une structure se sérialise"))
            .execute(&mut *tx)
            .await?;
            sqlx::query("UPDATE series SET publication_status = ? WHERE id = ?")
                .bind(merged.publication_status.as_ref().map(to_db))
                .bind(id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await
    }

    /// Modifie les métadonnées d'une série ; `false` si elle n'existe pas.
    pub async fn edit_series(&self, id: i64, patch: Edited) -> Result<bool, EditError> {
        // Lecture et écriture dans une même transaction : deux appareils qui éditent des
        // champs différents en même temps ne s'effacent pas.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(row) = sqlx::query(
            "SELECT m.edited FROM series s LEFT JOIN series_metadata m ON m.series_id = s.id
             WHERE s.id = ?",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(false);
        };
        let mut edited = parse_edited(row.get("edited"))?;
        apply(&mut edited, patch, SERIES_FIELDS).map_err(EditError::Invalid)?;
        let check = overlay(SeriesMetadata::default(), &edited).map_err(EditError::Invalid)?;
        if let Some(page) = &check.cover_page_id
            && !page_in_series(&mut tx, page, id).await?
        {
            return Err(EditError::Invalid(format!(
                "la page {page} n'est pas dans cette série"
            )));
        }
        sqlx::query(
            "INSERT INTO series_metadata (series_id, edited) VALUES (?, ?)
             ON CONFLICT (series_id) DO UPDATE SET edited = excluded.edited",
        )
        .bind(id)
        .bind(Value::Object(edited).to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.refresh(None, Some(id)).await?;
        Ok(true)
    }

    /// Métadonnées fusionnées d'une série et ses champs verrouillés.
    pub(crate) async fn series_metadata(
        &self,
        id: i64,
    ) -> Result<(SeriesMetadata, Vec<String>), Error> {
        let row = sqlx::query("SELECT edited, merged FROM series_metadata WHERE series_id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let Some(row) = row else {
            return Ok(Default::default());
        };
        let merged =
            serde_json::from_str(row.get("merged")).map_err(|e| Error::Decode(Box::new(e)))?;
        Ok((
            merged,
            parse_edited(row.get("edited"))?
                .into_iter()
                .map(|(k, _)| k)
                .collect(),
        ))
    }

    /// Modifie les métadonnées d'un tome ; `false` s'il n'existe pas.
    pub async fn edit_volume(&self, id: &VolumeId, patch: Edited) -> Result<bool, EditError> {
        let id = id.to_string();
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(row) = sqlx::query(
            "SELECT e.edited FROM volumes v LEFT JOIN volume_edits e ON e.volume_id = v.id
             WHERE v.id = ?",
        )
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?
        else {
            return Ok(false);
        };
        let mut edited = parse_edited(row.get("edited"))?;
        apply(&mut edited, patch, VOLUME_FIELDS).map_err(EditError::Invalid)?;
        overlay(VolumeMetadata::default(), &edited).map_err(EditError::Invalid)?;
        sqlx::query(
            "INSERT INTO volume_edits (volume_id, edited) VALUES (?, ?)
             ON CONFLICT (volume_id) DO UPDATE SET edited = excluded.edited",
        )
        .bind(&id)
        .bind(Value::Object(edited).to_string())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Métadonnées fusionnées d'un tome et ses champs verrouillés.
    pub(crate) async fn volume_metadata(
        &self,
        id: &str,
    ) -> Result<(VolumeMetadata, Vec<String>), Error> {
        let locals = sqlx::query("SELECT source, data FROM volume_metadata WHERE volume_id = ?")
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(local)
            .collect::<Result<Vec<_>, Error>>()?;
        let edited = sqlx::query("SELECT edited FROM volume_edits WHERE volume_id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        let edited = parse_edited(edited.as_ref().map(|r| r.get("edited")))?;
        let merged = overlay(volume_from_locals(&locals), &edited).map_err(decode)?;
        Ok((merged, edited.into_iter().map(|(k, _)| k).collect()))
    }
}

async fn page_in_series(
    conn: &mut sqlx::SqliteConnection,
    page: &str,
    series: i64,
) -> Result<bool, Error> {
    let found = sqlx::query(
        "SELECT 1 FROM pages p JOIN file_locations l ON l.volume_id = p.volume_id
         WHERE p.id = ? AND l.series_id = ?",
    )
    .bind(page)
    .bind(series)
    .fetch_optional(conn)
    .await?;
    Ok(found.is_some())
}

#[cfg(test)]
mod tests;
