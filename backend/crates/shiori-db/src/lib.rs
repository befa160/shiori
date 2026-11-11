//! Base SQLite de Shiori : persistance des scans et détection de changement
//! (principes 3 et 8).
//!
//! - Un fichier déjà connu dont le chemin, la taille et la date de modification n'ont pas
//!   changé n'est ni rehashé ni rescanné : son rapport est relu depuis la base.
//! - Un rescan met les pages à jour sur place par leur ID, sans jamais les supprimer puis
//!   les réinsérer : les tables qui référencent `pages.id` (progression, plus tard) ne
//!   perdent rien.

use std::io;
use std::path::{MAIN_SEPARATOR, Path};
use std::time::{Duration, UNIX_EPOCH};

use serde::Serialize;
use serde::de::DeserializeOwned;
use shiori_core::{
    FileReport, Issue, IssueCode, Page, SCANNER_VERSION, ScanOptions, Status, VolumeId, scan_file,
};
use sqlx::sqlite::{
    SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions, SqliteSynchronous,
};
use sqlx::{Row, Sqlite, Transaction};

pub use sqlx::Error;

/// Horodatage RFC 3339 UTC calculé par SQLite. Macro et non constante : sqlx n'accepte
/// que du SQL statique (`concat!`), jamais une chaîne construite à l'exécution.
macro_rules! now {
    () => {
        "strftime('%Y-%m-%dT%H:%M:%fZ', 'now')"
    };
}

mod browse;
mod library;
mod runs;

pub use browse::{
    LibrarySummary, PageFile, PageInfo, PageSource, SeriesDetail, SeriesSummary, SeriesVolume,
    VolumeDetail, VolumeLocation,
};
pub use library::{Library, LibraryError};
pub use runs::{RunCounts, RunOrigin, RunStatus};

pub struct Db {
    pool: SqlitePool,
}

/// Résultat du scan d'un fichier.
#[derive(Debug)]
pub struct Scanned {
    pub report: FileReport,
    /// Vrai si le fichier n'a pas changé : rapport relu depuis la base, sans hash.
    pub skipped: bool,
}

/// Taille et date de modification (ns depuis l'époque Unix) d'un fichier.
type Stamp = (i64, i64);

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = std::fs::metadata(path).ok()?;
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_nanos() as i64);
    Some((meta.len() as i64, mtime))
}

/// Chemin absolu avec `/` comme séparateur, quel que soit l'OS.
// shortcut: chemin non UTF-8 converti avec perte ; deux noms mal encodés peuvent partager
// une clé et se rescanner mutuellement. Suffixer les octets bruts si le cas se présente.
fn location_key(path: &Path) -> String {
    let abs = std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf());
    let key = abs.to_string_lossy();
    if MAIN_SEPARATOR == '/' {
        key.into_owned()
    } else {
        key.replace(MAIN_SEPARATOR, "/")
    }
}

/// Nom serde (`snake_case`) d'un enum du cœur : une seule source de vérité pour les noms.
fn to_db<T: Serialize>(value: &T) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(s)) => s,
        other => unreachable!("enum attendu, obtenu {other:?}"),
    }
}

fn from_db<T: DeserializeOwned>(s: &str) -> Result<T, Error> {
    serde_json::from_value(serde_json::Value::String(s.to_owned()))
        .map_err(|e| Error::Decode(Box::new(e)))
}

fn media_type(s: Option<&str>) -> Option<&'static str> {
    ["image/jpeg", "image/png", "image/webp", "image/avif"]
        .into_iter()
        .find(|m| Some(*m) == s)
}

impl Db {
    /// Ouvre (ou crée) la base et applique les migrations.
    pub async fn open(path: &Path) -> Result<Self, Error> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .synchronous(SqliteSynchronous::Normal)
            .foreign_keys(true)
            // SQLite n'a qu'un écrivain : les scans parallèles attendent leur tour au lieu
            // d'échouer (une écriture de 12 000 pages prend plus d'une seconde).
            .busy_timeout(Duration::from_secs(60));
        let pool = SqlitePoolOptions::new().connect_with(options).await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Scanne un fichier, ou relit son rapport s'il n'a pas changé (sauf `rehash`).
    pub async fn scan(
        &self,
        run: i64,
        path: &Path,
        options: ScanOptions,
        rehash: bool,
    ) -> Result<Scanned, Error> {
        let key = location_key(path);
        let before = stamp(path);
        if !rehash
            && let Some(stamp) = before
            && let Some(volume) = self.known_volume(&key, stamp).await?
            && let Some(report) = self.load_report(&volume, path).await?
        {
            let sql = concat!(
                "UPDATE file_locations SET last_seen_run = ?, last_seen_at = ",
                now!(),
                " WHERE path = ?"
            );
            sqlx::query(sql)
                .bind(run)
                .bind(&key)
                .execute(&self.pool)
                .await?;
            return Ok(Scanned {
                report,
                skipped: true,
            });
        }

        let owned = path.to_path_buf();
        let report = tokio::task::spawn_blocking(move || scan_file(&owned, options))
            .await
            .map_err(|e| Error::Io(io::Error::other(e)))?;
        self.save(run, &key, before, &report).await?;
        Ok(Scanned {
            report,
            skipped: false,
        })
    }

    async fn known_volume(
        &self,
        key: &str,
        (size, mtime): Stamp,
    ) -> Result<Option<VolumeId>, Error> {
        let row = sqlx::query(
            "SELECT l.volume_id FROM file_locations l JOIN volumes v ON v.id = l.volume_id
             WHERE l.path = ? AND l.size = ? AND l.mtime_ns = ? AND v.scanner_version = ?",
        )
        .bind(key)
        .bind(size)
        .bind(mtime)
        .bind(SCANNER_VERSION)
        .fetch_optional(&self.pool)
        .await?;
        row.map(|r| from_hex(r.get("volume_id"))).transpose()
    }

    /// Enregistre un rapport en une transaction.
    async fn save(
        &self,
        run: i64,
        key: &str,
        stamp: Option<Stamp>,
        report: &FileReport,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;
        // Un fichier modifié pendant le scan n'a pas de tome fiable : rien n'est rattaché.
        let changed = report
            .issues
            .iter()
            .any(|i| i.code == IssueCode::FileChangedDuringScan);
        let volume = report.volume_id.filter(|_| !changed).map(|v| v.to_string());

        sqlx::query("DELETE FROM scan_issues WHERE path = ? AND volume_id IS NULL")
            .bind(key)
            .execute(&mut *tx)
            .await?;
        match (&volume, stamp) {
            (Some(volume), Some((size, mtime))) => {
                save_volume(&mut tx, volume, report).await?;
                let sql = concat!(
                    "INSERT INTO file_locations
                       (path, volume_id, size, mtime_ns, last_seen_run, last_seen_at)
                     VALUES (?, ?, ?, ?, ?, ",
                    now!(),
                    ")
                     ON CONFLICT (path) DO UPDATE SET volume_id = excluded.volume_id,
                       size = excluded.size, mtime_ns = excluded.mtime_ns,
                       last_seen_run = excluded.last_seen_run, last_seen_at = excluded.last_seen_at"
                );
                sqlx::query(sql)
                    .bind(key)
                    .bind(volume)
                    .bind(size)
                    .bind(mtime)
                    .bind(run)
                    .execute(&mut *tx)
                    .await?;
                sqlx::query("DELETE FROM scan_issues WHERE volume_id = ?")
                    .bind(volume)
                    .execute(&mut *tx)
                    .await?;
            }
            _ => {
                // Pas de tome fiable : le prochain scan devra tout refaire.
                sqlx::query("DELETE FROM file_locations WHERE path = ?")
                    .bind(key)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        let volume = volume.filter(|_| stamp.is_some());
        for issue in &report.issues {
            sqlx::query(
                "INSERT INTO scan_issues
                   (scan_run_id, volume_id, path, severity, code, entry_name, message)
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(run)
            .bind(&volume)
            .bind(key)
            .bind(to_db(&issue.severity))
            .bind(to_db(&issue.code))
            .bind(&issue.entry)
            .bind(&issue.message)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await
    }

    /// Relit le rapport d'un tome depuis la base.
    pub async fn load_report(
        &self,
        volume: &VolumeId,
        path: &Path,
    ) -> Result<Option<FileReport>, Error> {
        let id = volume.to_string();
        let Some(row) = sqlx::query("SELECT size, status FROM volumes WHERE id = ?")
            .bind(&id)
            .fetch_optional(&self.pool)
            .await?
        else {
            return Ok(None);
        };
        let pages = sqlx::query(
            "SELECT id, ordinal, entry_name, entry_name_raw, archive_index, media_type, width,
                    height, compressed_size, size, crc32, status
             FROM pages WHERE volume_id = ? ORDER BY ordinal",
        )
        .bind(&id)
        .fetch_all(&self.pool)
        .await?
        .iter()
        .map(|r| {
            Ok(Page {
                id: r
                    .get::<&str, _>("id")
                    .parse()
                    .map_err(|e| Error::Decode(Box::new(e)))?,
                ordinal: r.get::<i64, _>("ordinal") as usize,
                entry_name: r.get("entry_name"),
                entry_name_raw: r.get("entry_name_raw"),
                archive_index: r.get::<i64, _>("archive_index") as usize,
                media_type: media_type(r.get("media_type")),
                width: r.get("width"),
                height: r.get("height"),
                compressed_size: r.get::<i64, _>("compressed_size") as u64,
                size: r.get::<i64, _>("size") as u64,
                crc32: r.get("crc32"),
                status: from_db(r.get("status"))?,
            })
        })
        .collect::<Result<_, Error>>()?;
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
        Ok(Some(FileReport {
            path: path.to_path_buf(),
            volume_id: Some(*volume),
            size: row.get::<i64, _>("size") as u64,
            status: from_db::<Status>(row.get("status"))?,
            pages,
            issues,
        }))
    }
}

fn from_hex(s: &str) -> Result<VolumeId, Error> {
    s.parse().map_err(|e| Error::Decode(Box::new(e)))
}

/// Met à jour le tome et ses pages sur place.
async fn save_volume(
    tx: &mut Transaction<'_, Sqlite>,
    volume: &str,
    report: &FileReport,
) -> Result<(), Error> {
    let sql = concat!(
        "INSERT INTO volumes (id, size, format, status, page_count, scanner_version, scanned_at)
         VALUES (?, ?, 'cbz', ?, ?, ?, ",
        now!(),
        ")
         ON CONFLICT (id) DO UPDATE SET size = excluded.size, status = excluded.status,
           page_count = excluded.page_count, scanner_version = excluded.scanner_version,
           scanned_at = excluded.scanned_at"
    );
    sqlx::query(sql)
        .bind(volume)
        .bind(report.size as i64)
        .bind(to_db(&report.status))
        .bind(report.pages.len() as i64)
        .bind(SCANNER_VERSION)
        .execute(&mut **tx)
        .await?;

    // Ordinaux mis hors plage le temps de la mise à jour (contrainte UNIQUE) ; les pages
    // encore négatives à la fin n'existent plus dans l'archive.
    sqlx::query("UPDATE pages SET ordinal = -1 - ordinal WHERE volume_id = ?")
        .bind(volume)
        .execute(&mut **tx)
        .await?;
    for p in &report.pages {
        sqlx::query(
            "INSERT INTO pages (id, volume_id, ordinal, entry_name, entry_name_raw, archive_index,
                                media_type, width, height, compressed_size, size, crc32, status)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (id) DO UPDATE SET ordinal = excluded.ordinal,
               entry_name = excluded.entry_name, archive_index = excluded.archive_index,
               media_type = excluded.media_type, width = excluded.width,
               height = excluded.height, compressed_size = excluded.compressed_size,
               size = excluded.size, crc32 = excluded.crc32, status = excluded.status",
        )
        .bind(p.id.to_string())
        .bind(volume)
        .bind(p.ordinal as i64)
        .bind(&p.entry_name)
        .bind(&p.entry_name_raw)
        .bind(p.archive_index as i64)
        .bind(p.media_type)
        .bind(p.width)
        .bind(p.height)
        .bind(p.compressed_size as i64)
        .bind(p.size as i64)
        .bind(p.crc32)
        .bind(to_db(&p.status))
        .execute(&mut **tx)
        .await?;
    }
    sqlx::query("DELETE FROM pages WHERE volume_id = ? AND ordinal < 0")
        .bind(volume)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests;
