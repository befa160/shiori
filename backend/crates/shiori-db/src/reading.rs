//! Lecture : progression par tome et réglages de lecture par série (phase 3, lot 3.3).
//!
//! Un seul utilisateur implicite tant qu'il n'y a pas de comptes (Q11). La progression est
//! rangée sous l'identité du tome et désigne la page par son ID : jamais un index brut.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use shiori_core::{PageId, VolumeId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, from_db, to_db};

/// Type de bibliothèque : donne le sens de lecture par défaut de ses séries (Q12).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LibraryKind {
    #[default]
    Manga,
    Comics,
    Webtoon,
}

impl LibraryKind {
    pub fn reading_mode(self) -> ReadingMode {
        match self {
            Self::Manga => ReadingMode::Rtl,
            Self::Comics => ReadingMode::Ltr,
            Self::Webtoon => ReadingMode::Vertical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingMode {
    /// Manga : de droite à gauche.
    Rtl,
    /// BD : de gauche à droite.
    Ltr,
    /// Webtoon : défilement vertical continu.
    Vertical,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageLayout {
    Single,
    Double,
    /// Double en paysage, simple en portrait (Q13).
    #[default]
    Auto,
}

/// Sens dans lequel on tourne les pages (taps, balayage, flèches, curseur), séparé du sens de
/// lecture, qui garde l'ordre des doubles pages et des moitiés coupées (lot 4.11).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageTurn {
    /// Comme le sens de lecture.
    #[default]
    Reading,
    /// Toucher à droite avance.
    Ltr,
    /// Toucher à gauche avance (gauchers, même en BD).
    Rtl,
}

/// Réglages d'une série : les valeurs appliquées, et ce que la série impose elle-même
/// (`None` = défaut de la bibliothèque, ou défaut global pour `page_turn`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReadingSettings {
    pub reading_mode: ReadingMode,
    pub page_layout: PageLayout,
    pub page_turn: PageTurn,
    pub reading_mode_override: Option<ReadingMode>,
    pub page_layout_override: Option<PageLayout>,
    pub page_turn_override: Option<PageTurn>,
}

impl ReadingSettings {
    /// D'après une ligne qui a `kind`, `reading_mode`, `page_layout`, `page_turn` (de la série)
    /// et `default_page_turn` (réglage global).
    pub(crate) fn of(row: &SqliteRow) -> Result<Self, Error> {
        let optional = |column| row.get::<Option<&str>, _>(column);
        let kind: LibraryKind = from_db(row.get("kind"))?;
        let mode = optional("reading_mode").map(from_db).transpose()?;
        let layout = optional("page_layout").map(from_db).transpose()?;
        let turn = optional("page_turn").map(from_db).transpose()?;
        Ok(Self {
            reading_mode: mode.unwrap_or(kind.reading_mode()),
            page_layout: layout.unwrap_or_default(),
            page_turn: turn.unwrap_or(from_db(row.get("default_page_turn"))?),
            reading_mode_override: mode,
            page_layout_override: layout,
            page_turn_override: turn,
        })
    }
}

impl Default for ReadingSettings {
    /// Tome rattaché à aucune série : défaut d'une bibliothèque de manga.
    // shortcut: ignore le défaut global de `page_turn` (cas rare, tome scanné hors bibliothèque) ; le lire en base si un cas réel apparaît.
    fn default() -> Self {
        Self {
            reading_mode: LibraryKind::default().reading_mode(),
            page_layout: PageLayout::default(),
            page_turn: PageTurn::default(),
            reading_mode_override: None,
            page_layout_override: None,
            page_turn_override: None,
        }
    }
}

/// Colonnes de [`ReadingSettings::of`], pour une requête où `s` est la série et `b` sa
/// bibliothèque.
macro_rules! settings_columns {
    () => {
        "b.kind, s.reading_mode, s.page_layout, s.page_turn,
         (SELECT page_turn FROM app_settings WHERE id = 1) AS default_page_turn"
    };
}
pub(crate) use settings_columns;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Progress {
    pub page_id: String,
    pub ordinal: i64,
    pub page_count: i64,
    /// La dernière page a été atteinte au moins une fois.
    pub finished: bool,
    pub updated_at: String,
}

impl Progress {
    fn of(row: &SqliteRow) -> Self {
        Self {
            page_id: row.get("page_id"),
            ordinal: row.get("ordinal"),
            page_count: row.get("page_count"),
            finished: row.get::<Option<String>, _>("finished_at").is_some(),
            updated_at: row.get("updated_at"),
        }
    }
}

/// Résultat d'un enregistrement de progression.
#[derive(Debug, PartialEq, Eq)]
pub enum SaveProgress {
    Saved(Progress),
    UnknownVolume,
    /// La page n'appartient pas à ce tome.
    PageNotInVolume,
}

/// Modification de réglages : `None` = inchangé, `Some(None)` = revenir au défaut.
#[derive(Debug, Default, Clone, Copy)]
pub struct SettingsChange {
    pub reading_mode: Option<Option<ReadingMode>>,
    pub page_layout: Option<Option<PageLayout>>,
    pub page_turn: Option<Option<PageTurn>>,
}

macro_rules! progress_of {
    ($filter:literal) => {
        concat!(
            "SELECT r.volume_id, r.page_id, r.ordinal, r.finished_at, r.updated_at, v.page_count
             FROM reading_progress r JOIN volumes v ON v.id = r.volume_id WHERE ",
            $filter
        )
    };
}

impl Db {
    /// Le tome est connu de la base.
    pub async fn volume_exists(&self, volume: &VolumeId) -> Result<bool, Error> {
        Ok(sqlx::query("SELECT 1 FROM volumes WHERE id = ?")
            .bind(volume.to_string())
            .fetch_optional(&self.pool)
            .await?
            .is_some())
    }

    /// Progression d'un tome ; `None` s'il n'a jamais été ouvert (ou n'existe pas).
    pub async fn progress(&self, volume: &VolumeId) -> Result<Option<Progress>, Error> {
        let row = sqlx::query(progress_of!("r.volume_id = ?"))
            .bind(volume.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(Progress::of))
    }

    /// Progression des tomes d'une série, par ID de tome.
    pub(crate) async fn progress_in_series(
        &self,
        series: i64,
    ) -> Result<HashMap<String, Progress>, Error> {
        let rows = sqlx::query(progress_of!(
            "r.volume_id IN (SELECT volume_id FROM file_locations WHERE series_id = ?)"
        ))
        .bind(series)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("volume_id"), Progress::of(r)))
            .collect())
    }

    /// Progression des tomes rattachés à une série, dans une bibliothèque ou dans toutes.
    pub(crate) async fn progress_in_libraries(
        &self,
        library: Option<i64>,
    ) -> Result<HashMap<String, Progress>, Error> {
        let rows = sqlx::query(progress_of!(
            "r.volume_id IN (SELECT volume_id FROM file_locations
             WHERE series_id IS NOT NULL AND (?1 IS NULL OR library_id = ?1))"
        ))
        .bind(library)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| (r.get("volume_id"), Progress::of(r)))
            .collect())
    }

    /// Enregistre la dernière page vue. Atteindre la dernière page marque le tome comme lu,
    /// et il le reste même si on revient en arrière (seul [`Db::clear_progress`] l'efface).
    pub async fn save_progress(
        &self,
        volume: &VolumeId,
        page: &PageId,
    ) -> Result<SaveProgress, Error> {
        let volume = volume.to_string();
        let Some(v) = sqlx::query("SELECT page_count FROM volumes WHERE id = ?")
            .bind(&volume)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(SaveProgress::UnknownVolume);
        };
        let Some(p) = sqlx::query("SELECT ordinal FROM pages WHERE id = ? AND volume_id = ?")
            .bind(page.to_string())
            .bind(&volume)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(SaveProgress::PageNotInVolume);
        };
        let ordinal: i64 = p.get("ordinal");
        let last = ordinal + 1 >= v.get::<i64, _>("page_count");
        let sql = concat!(
            "INSERT INTO reading_progress (volume_id, page_id, ordinal, finished_at, updated_at)
             VALUES (?, ?, ?, CASE WHEN ? THEN ",
            now!(),
            " END, ",
            now!(),
            ")
             ON CONFLICT (volume_id) DO UPDATE SET page_id = excluded.page_id,
               ordinal = excluded.ordinal, updated_at = excluded.updated_at,
               finished_at = COALESCE(reading_progress.finished_at, excluded.finished_at)"
        );
        sqlx::query(sql)
            .bind(&volume)
            .bind(page.to_string())
            .bind(ordinal)
            .bind(last)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query(progress_of!("r.volume_id = ?"))
            .bind(&volume)
            .fetch_one(&self.pool)
            .await?;
        Ok(SaveProgress::Saved(Progress::of(&row)))
    }

    /// « Marquer comme non lu ». `false` si le tome n'existe pas.
    pub async fn clear_progress(&self, volume: &VolumeId) -> Result<bool, Error> {
        sqlx::query("DELETE FROM reading_progress WHERE volume_id = ?")
            .bind(volume.to_string())
            .execute(&self.pool)
            .await?;
        self.volume_exists(volume).await
    }

    /// Réglages appliqués à une série ; `None` si elle n'existe pas.
    pub async fn series_settings(&self, series: i64) -> Result<Option<ReadingSettings>, Error> {
        let row = sqlx::query(concat!(
            "SELECT ",
            settings_columns!(),
            " FROM series s JOIN libraries b ON b.id = s.library_id WHERE s.id = ?"
        ))
        .bind(series)
        .fetch_optional(&self.pool)
        .await?;
        row.as_ref().map(ReadingSettings::of).transpose()
    }

    /// Modifie les réglages d'une série ; `None` si elle n'existe pas.
    pub async fn change_series_settings(
        &self,
        series: i64,
        change: SettingsChange,
    ) -> Result<Option<ReadingSettings>, Error> {
        let Some(current) = self.series_settings(series).await? else {
            return Ok(None);
        };
        let mode = change.reading_mode.unwrap_or(current.reading_mode_override);
        let layout = change.page_layout.unwrap_or(current.page_layout_override);
        let turn = change.page_turn.unwrap_or(current.page_turn_override);
        sqlx::query(
            "UPDATE series SET reading_mode = ?, page_layout = ?, page_turn = ? WHERE id = ?",
        )
        .bind(mode.as_ref().map(to_db))
        .bind(layout.as_ref().map(to_db))
        .bind(turn.as_ref().map(to_db))
        .bind(series)
        .execute(&self.pool)
        .await?;
        self.series_settings(series).await
    }

    /// Réglages qui s'appliquent à un tome : ceux de sa première série (par chemin), sinon
    /// le défaut d'une bibliothèque de manga.
    pub(crate) async fn volume_settings(&self, volume: &str) -> Result<ReadingSettings, Error> {
        let row = sqlx::query(concat!(
            "SELECT ",
            settings_columns!(),
            " FROM file_locations l JOIN series s ON s.id = l.series_id
             JOIN libraries b ON b.id = s.library_id
             WHERE l.volume_id = ? ORDER BY l.path LIMIT 1"
        ))
        .bind(volume)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row
            .as_ref()
            .map(ReadingSettings::of)
            .transpose()?
            .unwrap_or_default())
    }
}

#[cfg(test)]
mod tests;
