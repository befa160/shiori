//! Lectures pour l'API : bibliothèques, séries, tomes et pages, dans l'ordre établi par le
//! scan et le parseur. Le lecteur n'infère rien : tout ordre vient d'ici (principe 3).

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;
use shiori_core::{
    Issue, IssueCode, NameKind, PageId, PageStatus, ParsedName, Status, VolumeId,
    compare_entry_names, compare_in_series,
};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, from_db, stamp};

#[derive(Debug, Serialize)]
pub struct LibrarySummary {
    pub id: i64,
    pub name: String,
    pub root: String,
    pub series_count: i64,
    pub volume_count: i64,
}

#[derive(Debug, Serialize)]
pub struct SeriesSummary {
    pub id: i64,
    pub name: String,
    pub volume_count: usize,
    /// Page 0 du premier tome de la série.
    pub cover_page_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SeriesDetail {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub folder: Option<String>,
    pub volumes: Vec<SeriesVolume>,
}

/// Un tome dans sa série. Deux copies identiques d'un fichier n'apparaissent qu'une fois.
#[derive(Debug, Serialize)]
pub struct SeriesVolume {
    pub volume_id: String,
    pub kind: NameKind,
    pub number: Option<f64>,
    pub number_end: Option<f64>,
    pub status: Status,
    pub page_count: i64,
    pub cover_page_id: Option<String>,
    pub file_name: String,
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
    pub status: PageStatus,
    pub media_type: Option<String>,
    pub archive_index: usize,
    pub entry_name_raw: Vec<u8>,
    pub crc32: u32,
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

fn by_name(a: &str, b: &str) -> Ordering {
    compare_entry_names(a, a.as_bytes(), b, b.as_bytes())
}

impl Db {
    pub async fn library_summaries(&self) -> Result<Vec<LibrarySummary>, Error> {
        let rows = sqlx::query(
            "SELECT b.id, b.name, b.root,
                    (SELECT COUNT(*) FROM series s WHERE s.library_id = b.id) AS series_count,
                    (SELECT COUNT(DISTINCT l.volume_id) FROM file_locations l
                     WHERE l.library_id = b.id) AS volume_count
             FROM libraries b ORDER BY b.name, b.id",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .iter()
            .map(|r| LibrarySummary {
                id: r.get("id"),
                name: r.get("name"),
                root: r.get("root"),
                series_count: r.get("series_count"),
                volume_count: r.get("volume_count"),
            })
            .collect())
    }

    /// Séries d'une bibliothèque, triées par nom (tri naturel) ; `None` si elle n'existe pas.
    // shortcut: liste complète sans pagination, suffisant pour quelques milliers de séries ; paginer si une bibliothèque réelle le demande.
    pub async fn series_of(&self, library_id: i64) -> Result<Option<Vec<SeriesSummary>>, Error> {
        if sqlx::query("SELECT 1 FROM libraries WHERE id = ?")
            .bind(library_id)
            .fetch_optional(&self.pool)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        let series = sqlx::query("SELECT id, name FROM series WHERE library_id = ?")
            .bind(library_id)
            .fetch_all(&self.pool)
            .await?;
        let rows = sqlx::query(located_volumes!(
            "l.series_id IN (SELECT id FROM series WHERE library_id = ?)"
        ))
        .bind(library_id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(located)
        .collect::<Result<Vec<_>, _>>()?;
        let rows = in_series_order(rows);
        let mut out: Vec<SeriesSummary> = series
            .iter()
            .map(|s| {
                let id: i64 = s.get("id");
                let volumes: Vec<&Located> = rows.iter().filter(|r| r.series_id == id).collect();
                SeriesSummary {
                    id,
                    name: s.get("name"),
                    volume_count: volumes.len(),
                    cover_page_id: volumes.first().and_then(|r| r.volume.cover_page_id.clone()),
                }
            })
            .collect();
        out.sort_by(|a, b| by_name(&a.name, &b.name).then(a.id.cmp(&b.id)));
        Ok(Some(out))
    }

    pub async fn series_detail(&self, id: i64) -> Result<Option<SeriesDetail>, Error> {
        let Some(series) = sqlx::query("SELECT library_id, name, folder FROM series WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let rows = sqlx::query(located_volumes!("l.series_id = ?"))
            .bind(id)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(located)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Some(SeriesDetail {
            id,
            library_id: series.get("library_id"),
            name: series.get("name"),
            folder: series.get("folder"),
            volumes: in_series_order(rows)
                .into_iter()
                .map(|r| r.volume)
                .collect(),
        }))
    }

    pub async fn volume_detail(&self, id: &VolumeId) -> Result<Option<VolumeDetail>, Error> {
        let id = id.to_string();
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
        Ok(Some(VolumeDetail {
            id,
            size: v.get("size"),
            format: v.get("format"),
            status: from_db(v.get::<&str, _>("status"))?,
            page_count: v.get("page_count"),
            scanned_at: v.get("scanned_at"),
            locations,
            issues,
        }))
    }

    /// Page et fichier où la lire ; `None` si l'ID est inconnu.
    pub async fn page_source(&self, id: &PageId) -> Result<Option<PageSource>, Error> {
        let Some(page) = sqlx::query(
            "SELECT volume_id, status, media_type, archive_index, entry_name_raw, crc32
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
            status: from_db(page.get::<&str, _>("status"))?,
            media_type: page.get("media_type"),
            archive_index: page.get::<i64, _>("archive_index") as usize,
            entry_name_raw: page.get("entry_name_raw"),
            crc32: page.get("crc32"),
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
