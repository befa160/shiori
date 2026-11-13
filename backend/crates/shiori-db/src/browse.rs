//! Lectures pour l'API : bibliothèques, séries, tomes et pages, dans l'ordre établi par le
//! scan et le parseur. Le lecteur n'infère rien : tout ordre vient d'ici (principe 3).

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use shiori_core::{
    Issue, IssueCode, NameKind, PageId, PageStatus, ParsedName, Status, VolumeId,
    compare_entry_names, compare_in_series,
};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, LibraryKind, Progress, ReadingSettings, from_db, stamp, to_db};

#[derive(Debug, Serialize)]
pub struct LibrarySummary {
    pub id: i64,
    pub name: String,
    pub root: String,
    pub kind: LibraryKind,
    pub series_count: i64,
    pub volume_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SeriesSummary {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub volume_count: usize,
    /// Tomes de la série dont la dernière page a été atteinte.
    pub read_count: usize,
    /// Pages de tous ses tomes, et pages lues : un tome fini compte en entier, un tome en cours
    /// jusqu'à la page atteinte. Donne l'avancement de la série, même d'un seul tome (webtoon).
    pub page_count: i64,
    pub pages_read: i64,
    pub reading_status: ReadingStatus,
    pub publication_status: Option<PublicationStatus>,
    pub created_at: String,
    /// Dernière page enregistrée dans un tome de la série.
    pub last_read_at: Option<String>,
    /// Page 0 du premier tome de la série.
    pub cover_page_id: Option<String>,
    /// Tome qu'ouvre « Lire » : voir [`resume`].
    pub resume_volume_id: Option<String>,
}

/// Où en est la lecture d'une série, calculé d'après la progression de ses tomes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReadingStatus {
    /// Aucun tome ouvert.
    Unread,
    InProgress,
    /// Tous les tomes lus jusqu'au bout.
    Read,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PublicationStatus {
    Ongoing,
    Completed,
    Hiatus,
    Cancelled,
}

/// Une entrée de « Continuer la lecture » : une par série, voir [`up_next`].
#[derive(Debug, Serialize)]
pub struct ContinueItem {
    pub series_id: i64,
    pub series_name: String,
    pub library_id: i64,
    pub volume: SeriesVolume,
    /// Dernière lecture dans la série : l'ordre de la liste.
    pub updated_at: String,
}

#[derive(Debug, Serialize)]
pub struct SeriesDetail {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub folder: Option<String>,
    pub publication_status: Option<PublicationStatus>,
    pub settings: ReadingSettings,
    /// Tome qu'ouvre « Lire » : même règle que dans [`SeriesSummary`].
    pub resume_volume_id: Option<String>,
    /// Comme dans [`SeriesSummary`].
    pub page_count: i64,
    pub pages_read: i64,
    pub volumes: Vec<SeriesVolume>,
}

/// Un tome dans sa série. Deux copies identiques d'un fichier n'apparaissent qu'une fois.
#[derive(Debug, Clone, Serialize)]
pub struct SeriesVolume {
    pub volume_id: String,
    pub kind: NameKind,
    pub number: Option<f64>,
    pub number_end: Option<f64>,
    pub status: Status,
    pub page_count: i64,
    pub cover_page_id: Option<String>,
    pub file_name: String,
    /// Rempli seulement dans le détail d'une série.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<Progress>,
}

#[derive(Debug, Serialize)]
pub struct VolumeDetail {
    pub id: String,
    pub size: i64,
    pub format: String,
    pub status: Status,
    pub page_count: i64,
    pub scanned_at: String,
    pub locations: Vec<VolumeLocation>,
    pub issues: Vec<Issue>,
    /// Réglages de sa série (la première par chemin), défaut d'une bibliothèque de manga sinon.
    pub settings: ReadingSettings,
    pub progress: Option<Progress>,
}

#[derive(Debug, Serialize)]
pub struct VolumeLocation {
    pub path: String,
    pub library_id: Option<i64>,
    pub series_id: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PageInfo {
    pub id: String,
    pub ordinal: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub media_type: Option<String>,
    pub status: PageStatus,
}

/// Où lire une page, et de quoi vérifier qu'il s'agit bien d'elle.
#[derive(Debug)]
pub struct PageSource {
    pub volume_id: VolumeId,
    pub status: PageStatus,
    pub media_type: Option<String>,
    pub width: Option<u32>,
    pub archive_index: usize,
    pub entry_name_raw: Vec<u8>,
    pub crc32: u32,
    /// Taille décompressée déclarée par l'archive.
    pub size: u64,
    pub file: PageFile,
}

/// État des fichiers connus du tome d'une page.
#[derive(Debug, PartialEq, Eq)]
pub enum PageFile {
    /// Un emplacement a encore la taille et la date (ns) du scan.
    Ready(PathBuf, (i64, i64)),
    /// Les fichiers existent mais ont changé depuis le scan : il faut rescanner.
    Changed,
    /// Aucun emplacement connu n'existe plus.
    Missing,
}

/// Emplacements rattachés à des séries, avec leur tome et sa couverture.
macro_rules! located_volumes {
    ($filter:literal) => {
        concat!(
            "SELECT l.series_id, l.path, l.kind, l.number, l.number_end, v.id AS volume_id,
                    v.status, v.page_count,
                    (SELECT p.id FROM pages p WHERE p.volume_id = v.id AND p.ordinal = 0) AS cover
             FROM file_locations l JOIN volumes v ON v.id = l.volume_id
             WHERE ",
            $filter
        )
    };
}

struct Located {
    series_id: i64,
    path: String,
    parsed: ParsedName,
    volume: SeriesVolume,
}

fn located(row: &SqliteRow) -> Result<Located, Error> {
    let path: String = row.get("path");
    let kind = from_db(row.get::<&str, _>("kind"))?;
    let (number, number_end) = (row.get("number"), row.get("number_end"));
    let file_name = path.rsplit('/').next().unwrap_or_default().to_owned();
    Ok(Located {
        series_id: row.get("series_id"),
        parsed: ParsedName {
            folder: None,
            series: String::new(),
            kind,
            number,
            number_end,
        },
        volume: SeriesVolume {
            volume_id: row.get("volume_id"),
            kind,
            number,
            number_end,
            status: from_db(row.get::<&str, _>("status"))?,
            page_count: row.get("page_count"),
            cover_page_id: row.get("cover"),
            file_name,
            progress: None,
        },
        path,
    })
}

/// Trie dans l'ordre de la série et garde une seule entrée par tome.
fn in_series_order(mut rows: Vec<Located>) -> Vec<Located> {
    rows.sort_by(|a, b| {
        a.series_id
            .cmp(&b.series_id)
            .then_with(|| compare_in_series(&a.parsed, &a.path, &b.parsed, &b.path))
    });
    let mut seen = HashSet::new();
    rows.retain(|r| seen.insert((r.series_id, r.volume.volume_id.clone())));
    rows
}

/// Tomes de chaque série, dans l'ordre de la série.
fn by_series(rows: &[Located]) -> HashMap<i64, Vec<&Located>> {
    let mut out: HashMap<i64, Vec<&Located>> = HashMap::new();
    for row in rows {
        out.entry(row.series_id).or_default().push(row);
    }
    out
}

fn finished(progress: &HashMap<String, Progress>, volume: &Located) -> bool {
    progress
        .get(&volume.volume.volume_id)
        .is_some_and(|p| p.finished)
}

/// Suite de la lecture d'une série (Q24) : le tome ouvert en dernier s'il n'est pas arrêté sur
/// sa dernière page (une relecture compte, même si le tome a déjà été fini), sinon le suivant
/// dans l'ordre qui n'est ni lu ni en erreur. Renvoie aussi la date de cette
/// dernière lecture. `None` si aucun tome n'a été ouvert, ou s'il n'y a pas de suite.
fn up_next<'a>(
    volumes: &[&'a Located],
    progress: &HashMap<String, Progress>,
) -> Option<(&'a Located, String)> {
    let (last, p) = volumes
        .iter()
        .enumerate()
        .filter_map(|(i, v)| progress.get(&v.volume.volume_id).map(|p| (i, p)))
        .max_by(|a, b| a.1.updated_at.cmp(&b.1.updated_at))?;
    if p.ordinal + 1 < p.page_count {
        return Some((volumes[last], p.updated_at.clone()));
    }
    volumes[last + 1..]
        .iter()
        .find(|v| v.volume.status != Status::Error && !finished(progress, v))
        .map(|v| (*v, p.updated_at.clone()))
}

/// Pages lues et pages en tout : un tome fini compte en entier, un tome en cours jusqu'à la
/// page atteinte. Une seule règle pour la liste des séries et leur détail.
fn pages(volumes: &[&Located], progress: &HashMap<String, Progress>) -> (i64, i64) {
    volumes.iter().fold((0, 0), |(read, total), v| {
        let n = v.volume.page_count;
        let done = match progress.get(&v.volume.volume_id) {
            Some(p) if p.finished => n,
            Some(p) => p.ordinal + 1,
            None => 0,
        };
        (read + done, total + n)
    })
}

/// Tome qu'ouvre « Lire » : la suite de la lecture, sinon le premier tome ni lu ni en erreur,
/// sinon (tout est lu) le premier tome, pour relire.
fn resume(volumes: &[&Located], progress: &HashMap<String, Progress>) -> Option<String> {
    up_next(volumes, progress)
        .map(|(v, _)| v)
        .or_else(|| {
            volumes
                .iter()
                .find(|v| v.volume.status != Status::Error && !finished(progress, v))
                .copied()
        })
        .or_else(|| volumes.first().copied())
        .map(|v| v.volume.volume_id.clone())
}

fn by_name(a: &str, b: &str) -> Ordering {
    compare_entry_names(a, a.as_bytes(), b, b.as_bytes())
}

impl Db {
    pub async fn library_summaries(&self) -> Result<Vec<LibrarySummary>, Error> {
        let rows = sqlx::query(
            "SELECT b.id, b.name, b.root, b.kind,
                    (SELECT COUNT(*) FROM series s WHERE s.library_id = b.id) AS series_count,
                    (SELECT COUNT(DISTINCT l.volume_id) FROM file_locations l
                     WHERE l.library_id = b.id) AS volume_count
             FROM libraries b ORDER BY b.name, b.id",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(LibrarySummary {
                    id: r.get("id"),
                    name: r.get("name"),
                    root: r.get("root"),
                    kind: from_db(r.get("kind"))?,
                    series_count: r.get("series_count"),
                    volume_count: r.get("volume_count"),
                })
            })
            .collect()
    }

    /// Séries d'une bibliothèque, triées par nom (tri naturel) ; `None` si elle n'existe pas.
    pub async fn series_of(&self, library_id: i64) -> Result<Option<Vec<SeriesSummary>>, Error> {
        if sqlx::query("SELECT 1 FROM libraries WHERE id = ?")
            .bind(library_id)
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        self.summaries(Some(library_id)).await.map(Some)
    }

    /// Séries de toutes les bibliothèques, triées par nom (tri naturel).
    pub async fn all_series(&self) -> Result<Vec<SeriesSummary>, Error> {
        self.summaries(None).await
    }

    // shortcut: liste complète sans pagination (recherche et filtres côté client), suffisant pour quelques milliers de séries ; paginer et filtrer côté serveur au-delà.
    async fn summaries(&self, library: Option<i64>) -> Result<Vec<SeriesSummary>, Error> {
        let series = sqlx::query(
            "SELECT id, library_id, name, publication_status, created_at FROM series
             WHERE ?1 IS NULL OR library_id = ?1",
        )
        .bind(library)
        .fetch_all(&self.pool)
        .await?;
        let rows = sqlx::query(located_volumes!(
            "l.series_id IN (SELECT id FROM series WHERE ?1 IS NULL OR library_id = ?1)"
        ))
        .bind(library)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(located)
        .collect::<Result<Vec<_>, _>>()?;
        let rows = in_series_order(rows);
        let volumes = by_series(&rows);
        let progress = self.progress_in_libraries(library).await?;
        let mut out = series
            .iter()
            .map(|s| {
                let id: i64 = s.get("id");
                let vols = volumes.get(&id).map_or(&[][..], Vec::as_slice);
                let read_count = vols.iter().filter(|v| finished(&progress, v)).count();
                let (pages_read, page_count) = pages(vols, &progress);
                let read_at = vols
                    .iter()
                    .filter_map(|v| progress.get(&v.volume.volume_id))
                    .map(|p| &p.updated_at)
                    .max();
                let reading_status = match read_at {
                    None => ReadingStatus::Unread,
                    Some(_) if read_count == vols.len() => ReadingStatus::Read,
                    Some(_) => ReadingStatus::InProgress,
                };
                Ok(SeriesSummary {
                    id,
                    library_id: s.get("library_id"),
                    name: s.get("name"),
                    volume_count: vols.len(),
                    read_count,
                    page_count,
                    pages_read,
                    reading_status,
                    publication_status: s
                        .get::<Option<&str>, _>("publication_status")
                        .map(from_db)
                        .transpose()?,
                    created_at: s.get("created_at"),
                    last_read_at: read_at.cloned(),
                    cover_page_id: vols.first().and_then(|r| r.volume.cover_page_id.clone()),
                    resume_volume_id: resume(vols, &progress),
                })
            })
            .collect::<Result<Vec<_>, Error>>()?;
        out.sort_by(|a, b| by_name(&a.name, &b.name).then(a.id.cmp(&b.id)));
        Ok(out)
    }

    /// « Continuer la lecture » : la suite de chaque série commencée ([`up_next`]), de la
    /// lecture la plus récente à la plus ancienne.
    pub async fn continue_reading(&self, limit: usize) -> Result<Vec<ContinueItem>, Error> {
        // Séries dont au moins un tome a été ouvert.
        macro_rules! started {
            () => {
                "(SELECT l2.series_id FROM file_locations l2
                  JOIN reading_progress r ON r.volume_id = l2.volume_id)"
            };
        }
        let series = sqlx::query(concat!(
            "SELECT id, name, library_id FROM series WHERE id IN ",
            started!()
        ))
        .fetch_all(&self.pool)
        .await?;
        let rows = sqlx::query(concat!(located_volumes!("l.series_id IN "), started!()))
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(located)
            .collect::<Result<Vec<_>, _>>()?;
        let rows = in_series_order(rows);
        let volumes = by_series(&rows);
        let progress = self.progress_in_libraries(None).await?;
        let mut out: Vec<ContinueItem> = series
            .iter()
            .filter_map(|s| {
                let id: i64 = s.get("id");
                let (next, updated_at) = up_next(volumes.get(&id)?, &progress)?;
                Some(ContinueItem {
                    series_id: id,
                    series_name: s.get("name"),
                    library_id: s.get("library_id"),
                    volume: SeriesVolume {
                        progress: progress.get(&next.volume.volume_id).cloned(),
                        ..next.volume.clone()
                    },
                    updated_at,
                })
            })
            .collect();
        out.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then(a.series_id.cmp(&b.series_id))
        });
        out.truncate(limit);
        Ok(out)
    }

    /// Change le statut de publication d'une série ; `false` si elle n'existe pas.
    pub async fn set_publication_status(
        &self,
        series: i64,
        status: Option<PublicationStatus>,
    ) -> Result<bool, Error> {
        let done = sqlx::query("UPDATE series SET publication_status = ? WHERE id = ?")
            .bind(status.as_ref().map(to_db))
            .bind(series)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    pub async fn series_detail(&self, id: i64) -> Result<Option<SeriesDetail>, Error> {
        let Some(series) = sqlx::query(
            "SELECT s.library_id, s.name, s.folder, s.publication_status, s.reading_mode,
                    s.page_layout, b.kind
             FROM series s JOIN libraries b ON b.id = s.library_id WHERE s.id = ?",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let mut progress = self.progress_in_series(id).await?;
        let rows = sqlx::query(located_volumes!("l.series_id = ?"))
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(located)
            .collect::<Result<Vec<_>, _>>()?;
        let rows = in_series_order(rows);
        let all: Vec<&Located> = rows.iter().collect();
        let resume_volume_id = resume(&all, &progress);
        let (pages_read, page_count) = pages(&all, &progress);
        Ok(Some(SeriesDetail {
            id,
            library_id: series.get("library_id"),
            name: series.get("name"),
            folder: series.get("folder"),
            publication_status: series
                .get::<Option<&str>, _>("publication_status")
                .map(from_db)
                .transpose()?,
            settings: ReadingSettings::of(&series)?,
            resume_volume_id,
            page_count,
            pages_read,
            volumes: rows
                .into_iter()
                .map(|r| SeriesVolume {
                    progress: progress.remove(&r.volume.volume_id),
                    ..r.volume
                })
                .collect(),
        }))
    }

    pub async fn volume_detail(&self, volume: &VolumeId) -> Result<Option<VolumeDetail>, Error> {
        let id = volume.to_string();
        let Some(v) = sqlx::query(
            "SELECT size, format, status, page_count, scanned_at FROM volumes WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let locations = sqlx::query(
            "SELECT path, library_id, series_id FROM file_locations WHERE volume_id = ?
             ORDER BY path",
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| VolumeLocation {
            path: r.get("path"),
            library_id: r.get("library_id"),
            series_id: r.get("series_id"),
        })
        .collect();
        let issues = sqlx::query(
            "SELECT code, entry_name, message FROM scan_issues WHERE volume_id = ? ORDER BY id",
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| {
            let code: IssueCode = from_db(r.get("code"))?;
            Ok(Issue {
                code,
                severity: code.severity(),
                entry: r.get("entry_name"),
                message: r.get("message"),
            })
        })
        .collect::<Result<_, Error>>()?;
        let settings = self.volume_settings(&id).await?;
        let progress = self.progress(volume).await?;
        Ok(Some(VolumeDetail {
            id,
            size: v.get("size"),
            format: v.get("format"),
            status: from_db(v.get::<&str, _>("status"))?,
            page_count: v.get("page_count"),
            scanned_at: v.get("scanned_at"),
            locations,
            issues,
            settings,
            progress,
        }))
    }

    /// Page et fichier où la lire ; `None` si l'ID est inconnu.
    pub async fn page_source(&self, id: &PageId) -> Result<Option<PageSource>, Error> {
        let Some(page) = sqlx::query(
            "SELECT volume_id, status, media_type, width, archive_index, entry_name_raw, crc32, size
             FROM pages WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        else {
            return Ok(None);
        };
        let locations = sqlx::query(
            "SELECT path, size, mtime_ns FROM file_locations WHERE volume_id = ? ORDER BY path",
        )
        .bind(page.get::<&str, _>("volume_id"))
        .fetch_all(&self.pool)
        .await?;
        let mut file = PageFile::Missing;
        for l in &locations {
            let path = PathBuf::from(l.get::<String, _>("path"));
            match stamp(&path) {
                Some(found) if found == (l.get("size"), l.get("mtime_ns")) => {
                    file = PageFile::Ready(path, found);
                    break;
                }
                Some(_) => file = PageFile::Changed,
                None => {}
            }
        }
        Ok(Some(PageSource {
            volume_id: super::from_hex(page.get("volume_id"))?,
            status: from_db(page.get::<&str, _>("status"))?,
            media_type: page.get("media_type"),
            width: page.get("width"),
            archive_index: page.get::<i64, _>("archive_index") as usize,
            entry_name_raw: page.get("entry_name_raw"),
            crc32: page.get("crc32"),
            size: page.get::<i64, _>("size") as u64,
            file,
        }))
    }

    /// Pages d'un tome par ordinal ; `None` si le tome n'existe pas.
    pub async fn pages_of(&self, id: &VolumeId) -> Result<Option<Vec<PageInfo>>, Error> {
        let id = id.to_string();
        if sqlx::query("SELECT 1 FROM volumes WHERE id = ?")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let rows = sqlx::query(
            "SELECT id, ordinal, width, height, media_type, status FROM pages
             WHERE volume_id = ? ORDER BY ordinal",
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(PageInfo {
                    id: r.get("id"),
                    ordinal: r.get("ordinal"),
                    width: r.get("width"),
                    height: r.get("height"),
                    media_type: r.get("media_type"),
                    status: from_db(r.get::<&str, _>("status"))?,
                })
            })
            .collect::<Result<_, Error>>()
            .map(Some)
    }
}

#[cfg(test)]
mod tests;
