//! Lecture : progression par tome et réglages de lecture par série (phase 3, lot 3.3), propres
//! à chaque compte depuis la phase 7 (Q48). La progression est rangée sous l'identité du tome
//! et désigne la page par son ID : jamais un index brut.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use shiori_core::{PageId, VolumeId};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, Language, from_db, to_db};

/// Qui lit : la progression et les réglages sont ceux de ce compte, les titres et résumés
/// venus du web dans sa langue (Q34).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewer {
    pub user: i64,
    pub language: Language,
    /// Un administrateur voit toutes les bibliothèques ; un lecteur, celles qui lui sont
    /// ouvertes (`user_libraries`).
    pub admin: bool,
}

/// Condition SQL : la bibliothèque `$col` est visible du compte (`$admin` : est-il
/// administrateur, `$user` : son id ; des paramètres numérotés, comme `"?3"`).
macro_rules! visible {
    ($col:literal, $admin:literal, $user:literal) => {
        concat!(
            "(",
            $admin,
            " OR ",
            $col,
            " IN (SELECT library_id FROM user_libraries WHERE user_id = ",
            $user,
            "))"
        )
    };
}
pub(crate) use visible;

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

/// Réglages d'une série pour un compte : les valeurs appliquées, et ce que le compte a choisi
/// pour elle (`None` = défaut de la bibliothèque, ou défaut du compte pour `page_turn`).
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
    /// D'après une ligne de [`settings_of!`].
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

/// Réglages de la série `s` pour le compte `?1` ; `$series` choisit la série d'après `?2`.
/// Le sens pour tourner les pages par défaut est celui du compte, sinon celui de l'instance.
macro_rules! settings_of {
    ($series:literal) => {
        concat!(
            "SELECT b.kind, us.reading_mode, us.page_layout, us.page_turn,
                    COALESCE(ust.page_turn, a.page_turn) AS default_page_turn
             FROM series s JOIN libraries b ON b.id = s.library_id
             JOIN app_settings a ON a.id = 1
             LEFT JOIN user_settings ust ON ust.user_id = ?1
             LEFT JOIN user_series_settings us ON us.user_id = ?1 AND us.series_id = s.id
             WHERE s.id = ",
            $series
        )
    };
}

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
    /// `read_at` n'est pas une date.
    InvalidTime,
}

/// Modification de réglages : `None` = inchangé, `Some(None)` = revenir au défaut.
#[derive(Debug, Default, Clone, Copy)]
pub struct SettingsChange {
    pub reading_mode: Option<Option<ReadingMode>>,
    pub page_layout: Option<Option<PageLayout>>,
    pub page_turn: Option<Option<PageTurn>>,
}

/// Progression du compte `?1` ; `$filter` peut utiliser `?2`.
macro_rules! progress_of {
    ($filter:literal) => {
        concat!(
            "SELECT r.volume_id, r.page_id, r.ordinal, r.finished_at, r.updated_at, v.page_count
             FROM reading_progress r JOIN volumes v ON v.id = r.volume_id
             WHERE r.user_id = ?1 AND ",
            $filter
        )
    };
}

impl Db {
    /// Lecteur `user` : sa langue, sinon celle de l'instance, sinon le français.
    pub async fn viewer(&self, user: i64) -> Result<Viewer, Error> {
        let row = sqlx::query(
            "SELECT COALESCE(us.language, a.language, 'fr') AS language, u.role
             FROM users u JOIN app_settings a ON a.id = 1
             LEFT JOIN user_settings us ON us.user_id = u.id WHERE u.id = ?",
        )
        .bind(user)
        .fetch_one(&self.pool)
        .await?;
        Ok(Viewer {
            user,
            language: from_db(row.get("language"))?,
            admin: row.get::<&str, _>("role") == "admin",
        })
    }

    /// Le tome existe et le compte peut le lire : un administrateur, tout tome connu ; un
    /// lecteur, un tome rangé dans une bibliothèque qui lui est ouverte. Sinon, pour lui, le
    /// tome n'existe pas (404, jamais « existe mais interdit »).
    pub async fn volume_visible(&self, viewer: &Viewer, volume: &VolumeId) -> Result<bool, Error> {
        self.volume_visible_str(viewer, &volume.to_string()).await
    }

    pub(crate) async fn volume_visible_str(
        &self,
        viewer: &Viewer,
        volume: &str,
    ) -> Result<bool, Error> {
        let sql = concat!(
            "SELECT 1 FROM volumes v WHERE v.id = ?1 AND (?2 OR EXISTS (
               SELECT 1 FROM file_locations l WHERE l.volume_id = v.id AND ",
            visible!("l.library_id", "0", "?3"),
            "))"
        );
        Ok(sqlx::query(sql)
            .bind(volume)
            .bind(viewer.admin)
            .bind(viewer.user)
            .fetch_optional(&self.pool)
            .await?
            .is_some())
    }

    /// La série existe et sa bibliothèque est visible du compte.
    pub async fn series_visible(&self, viewer: &Viewer, series: i64) -> Result<bool, Error> {
        let sql = concat!(
            "SELECT 1 FROM series s WHERE s.id = ?1 AND ",
            visible!("s.library_id", "?2", "?3")
        );
        Ok(sqlx::query(sql)
            .bind(series)
            .bind(viewer.admin)
            .bind(viewer.user)
            .fetch_optional(&self.pool)
            .await?
            .is_some())
    }

    /// Progression d'un tome ; `None` s'il n'a jamais été ouvert (ou n'existe pas).
    pub async fn progress(
        &self,
        viewer: &Viewer,
        volume: &VolumeId,
    ) -> Result<Option<Progress>, Error> {
        let row = sqlx::query(progress_of!("r.volume_id = ?2"))
            .bind(viewer.user)
            .bind(volume.to_string())
            .fetch_optional(&self.pool)
            .await?;
        Ok(row.as_ref().map(Progress::of))
    }

    /// Progression des tomes d'une série, par ID de tome.
    pub(crate) async fn progress_in_series(
        &self,
        viewer: &Viewer,
        series: i64,
    ) -> Result<HashMap<String, Progress>, Error> {
        let rows = sqlx::query(progress_of!(
            "r.volume_id IN (SELECT volume_id FROM file_locations WHERE series_id = ?2)"
        ))
        .bind(viewer.user)
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
        viewer: &Viewer,
        library: Option<i64>,
    ) -> Result<HashMap<String, Progress>, Error> {
        let rows = sqlx::query(progress_of!(
            "r.volume_id IN (SELECT volume_id FROM file_locations
             WHERE series_id IS NOT NULL AND (?2 IS NULL OR library_id = ?2))"
        ))
        .bind(viewer.user)
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
        viewer: &Viewer,
        volume: &VolumeId,
        page: &PageId,
    ) -> Result<SaveProgress, Error> {
        self.save_progress_at(viewer, volume, page, None).await
    }

    /// Comme [`Db::save_progress`], pour une page lue à `read_at` (ISO 8601, lecture hors ligne
    /// envoyée plus tard) : elle ne remplace pas une page lue après elle sur un autre appareil,
    /// mais marque quand même le tome comme lu. Une date future compte comme maintenant.
    pub async fn save_progress_at(
        &self,
        viewer: &Viewer,
        volume: &VolumeId,
        page: &PageId,
        read_at: Option<&str>,
    ) -> Result<SaveProgress, Error> {
        let sql = concat!(
            "SELECT CASE WHEN ?1 IS NULL THEN ",
            now!(),
            " ELSE min(strftime('%Y-%m-%dT%H:%M:%fZ', ?1), ",
            now!(),
            ") END"
        );
        let at: Option<String> = sqlx::query_scalar(sql)
            .bind(read_at)
            .fetch_one(&self.pool)
            .await?;
        let Some(at) = at else {
            return Ok(SaveProgress::InvalidTime);
        };
        if !self.volume_visible(viewer, volume).await? {
            return Ok(SaveProgress::UnknownVolume);
        }
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
        // La plus récente des deux pages l'emporte ; « lu » ne se perd jamais.
        let sql = "INSERT INTO reading_progress
               (user_id, volume_id, page_id, ordinal, finished_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, CASE WHEN ?5 THEN ?6 END, ?6)
             ON CONFLICT (user_id, volume_id) DO UPDATE SET
               page_id = CASE WHEN excluded.updated_at >= reading_progress.updated_at
                         THEN excluded.page_id ELSE reading_progress.page_id END,
               ordinal = CASE WHEN excluded.updated_at >= reading_progress.updated_at
                         THEN excluded.ordinal ELSE reading_progress.ordinal END,
               updated_at = max(reading_progress.updated_at, excluded.updated_at),
               finished_at = COALESCE(reading_progress.finished_at, excluded.finished_at)";
        sqlx::query(sql)
            .bind(viewer.user)
            .bind(&volume)
            .bind(page.to_string())
            .bind(ordinal)
            .bind(last)
            .bind(&at)
            .execute(&self.pool)
            .await?;
        let row = sqlx::query(progress_of!("r.volume_id = ?2"))
            .bind(viewer.user)
            .bind(&volume)
            .fetch_one(&self.pool)
            .await?;
        Ok(SaveProgress::Saved(Progress::of(&row)))
    }

    /// « Marquer comme non lu ». `false` si le tome n'existe pas.
    pub async fn clear_progress(&self, viewer: &Viewer, volume: &VolumeId) -> Result<bool, Error> {
        sqlx::query("DELETE FROM reading_progress WHERE user_id = ? AND volume_id = ?")
            .bind(viewer.user)
            .bind(volume.to_string())
            .execute(&self.pool)
            .await?;
        self.volume_visible(viewer, volume).await
    }

    /// Réglages appliqués à une série pour ce compte ; `None` si elle n'existe pas.
    pub async fn series_settings(
        &self,
        viewer: &Viewer,
        series: i64,
    ) -> Result<Option<ReadingSettings>, Error> {
        if !self.series_visible(viewer, series).await? {
            return Ok(None);
        }
        let row = sqlx::query(settings_of!("?2"))
            .bind(viewer.user)
            .bind(series)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(ReadingSettings::of).transpose()
    }

    /// Modifie les réglages d'une série pour ce compte ; `None` si elle n'existe pas.
    pub async fn change_series_settings(
        &self,
        viewer: &Viewer,
        series: i64,
        change: SettingsChange,
    ) -> Result<Option<ReadingSettings>, Error> {
        let Some(current) = self.series_settings(viewer, series).await? else {
            return Ok(None);
        };
        let mode = change.reading_mode.unwrap_or(current.reading_mode_override);
        let layout = change.page_layout.unwrap_or(current.page_layout_override);
        let turn = change.page_turn.unwrap_or(current.page_turn_override);
        sqlx::query(
            "INSERT INTO user_series_settings
               (user_id, series_id, reading_mode, page_layout, page_turn)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (user_id, series_id) DO UPDATE SET reading_mode = excluded.reading_mode,
               page_layout = excluded.page_layout, page_turn = excluded.page_turn",
        )
        .bind(viewer.user)
        .bind(series)
        .bind(mode.as_ref().map(to_db))
        .bind(layout.as_ref().map(to_db))
        .bind(turn.as_ref().map(to_db))
        .execute(&self.pool)
        .await?;
        self.series_settings(viewer, series).await
    }

    /// Réglages qui s'appliquent à un tome : ceux de sa première série (par chemin), sinon
    /// le défaut d'une bibliothèque de manga.
    pub(crate) async fn volume_settings(
        &self,
        viewer: &Viewer,
        volume: &str,
    ) -> Result<ReadingSettings, Error> {
        let row = sqlx::query(settings_of!(
            "(SELECT series_id FROM file_locations
              WHERE volume_id = ?2 AND series_id IS NOT NULL ORDER BY path LIMIT 1)"
        ))
        .bind(viewer.user)
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
