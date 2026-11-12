//! Passes de scan : début, compteurs, fin, journal, et conservation limitée dans le temps.

use serde::{Deserialize, Serialize};
use shiori_core::{IssueCode, Severity, Status};
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::{Db, Error, Scanned, from_db, to_db};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunOrigin {
    Cli,
    Api,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
    /// Restée `running` après un arrêt brutal du serveur.
    Interrupted,
}

/// Compteurs d'une passe. `ok`, `warning` et `error` comptent tous les fichiers vus, inchangés
/// compris ; `unchanged` en est un sous-ensemble (relus depuis la base, sans hash).
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RunCounts {
    pub ok: u64,
    pub warning: u64,
    pub error: u64,
    pub unchanged: u64,
    pub removed: u64,
}

impl RunCounts {
    pub fn add(&mut self, scanned: &Scanned) {
        match scanned.report.status {
            Status::Ok => self.ok += 1,
            Status::Warning => self.warning += 1,
            Status::Error => self.error += 1,
        }
        self.unchanged += u64::from(scanned.skipped);
    }
}

/// Une passe telle que l'historique la montre.
#[derive(Debug, Serialize)]
pub struct RunSummary {
    pub id: i64,
    pub library_id: Option<i64>,
    pub origin: RunOrigin,
    pub status: RunStatus,
    /// Dossier, fichier ou racine scannés.
    pub root: String,
    pub options: serde_json::Value,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub duration_ms: Option<i64>,
    pub files_total: Option<i64>,
    pub counts: RunCounts,
    pub error: Option<String>,
}

/// Une ligne du journal d'une passe.
#[derive(Debug, Serialize)]
pub struct LogLine {
    pub id: i64,
    pub at: String,
    pub level: Severity,
    pub path: Option<String>,
    pub code: Option<IssueCode>,
    pub message: String,
}

/// Filtre de l'historique des passes.
#[derive(Debug, Default)]
pub struct RunFilter {
    pub status: Option<RunStatus>,
    pub library_id: Option<i64>,
    /// Passes plus anciennes que celle-ci (pagination, du plus récent au plus ancien).
    pub before: Option<i64>,
    pub limit: u32,
}

macro_rules! run_columns {
    () => {
        "SELECT id, library_id, origin, status, root, options, started_at, finished_at,
                CAST(ROUND((julianday(finished_at) - julianday(started_at)) * 86400000) AS INTEGER)
                  AS duration_ms,
                files_total, files_ok, files_warning, files_error, files_unchanged, files_removed,
                error
         FROM scan_runs "
    };
}

fn run_summary(r: &SqliteRow) -> Result<RunSummary, Error> {
    let count = |c: &str| r.get::<i64, _>(c) as u64;
    Ok(RunSummary {
        id: r.get("id"),
        library_id: r.get("library_id"),
        origin: from_db(r.get::<&str, _>("origin"))?,
        status: from_db(r.get::<&str, _>("status"))?,
        root: r.get("root"),
        // Écrites par Shiori, les options sont du JSON ; une ligne abîmée reste lisible.
        options: serde_json::from_str(r.get("options")).unwrap_or_default(),
        started_at: r.get("started_at"),
        finished_at: r.get("finished_at"),
        duration_ms: r.get("duration_ms"),
        files_total: r.get("files_total"),
        counts: RunCounts {
            ok: count("files_ok"),
            warning: count("files_warning"),
            error: count("files_error"),
            unchanged: count("files_unchanged"),
            removed: count("files_removed"),
        },
        error: r.get("error"),
    })
}

impl Db {
    /// Historique des passes, de la plus récente à la plus ancienne.
    pub async fn runs(&self, filter: &RunFilter) -> Result<Vec<RunSummary>, Error> {
        let sql = concat!(
            run_columns!(),
            "WHERE (?1 IS NULL OR status = ?1) AND (?2 IS NULL OR library_id = ?2)
               AND (?3 IS NULL OR id < ?3)
             ORDER BY id DESC LIMIT ?4"
        );
        sqlx::query(sql)
            .bind(filter.status.map(|s| to_db(&s)))
            .bind(filter.library_id)
            .bind(filter.before)
            .bind(filter.limit)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(run_summary)
            .collect()
    }

    pub async fn run(&self, id: i64) -> Result<Option<RunSummary>, Error> {
        let row = sqlx::query(concat!(run_columns!(), "WHERE id = ?"))
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        row.as_ref().map(run_summary).transpose()
    }

    /// Journal d'une passe dans l'ordre, à partir de la gravité `min` ; `after` pour paginer.
    pub async fn run_logs(
        &self,
        run: i64,
        min: Severity,
        after: Option<i64>,
        limit: u32,
    ) -> Result<Vec<LogLine>, Error> {
        let rows = sqlx::query(
            "SELECT id, at, level, path, code, message FROM scan_run_logs
             WHERE scan_run_id = ?1 AND (?2 IS NULL OR id > ?2)
               AND CASE level WHEN 'info' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END >= ?3
             ORDER BY id LIMIT ?4",
        )
        .bind(run)
        .bind(after)
        .bind(min as i64)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        rows.iter()
            .map(|r| {
                Ok(LogLine {
                    id: r.get("id"),
                    at: r.get("at"),
                    level: from_db(r.get::<&str, _>("level"))?,
                    path: r.get("path"),
                    code: r.get::<Option<&str>, _>("code").map(from_db).transpose()?,
                    message: r.get("message"),
                })
            })
            .collect()
    }
}

impl Db {
    /// Passe de la CLI, sans bibliothèque.
    pub async fn start_run(&self, root: &str, options: &str) -> Result<i64, Error> {
        self.begin_run(RunOrigin::Cli, None, root, options).await
    }

    pub async fn begin_run(
        &self,
        origin: RunOrigin,
        library_id: Option<i64>,
        root: &str,
        options: &str,
    ) -> Result<i64, Error> {
        let sql = concat!(
            "INSERT INTO scan_runs (root, options, origin, library_id, status, started_at)
             VALUES (?, ?, ?, ?, 'running', ",
            now!(),
            ")"
        );
        let done = sqlx::query(sql)
            .bind(root)
            .bind(options)
            .bind(to_db(&origin))
            .bind(library_id)
            .execute(&self.pool)
            .await?;
        Ok(done.last_insert_rowid())
    }

    /// Nombre de fichiers à scanner, connu après le parcours et avant le scan.
    pub async fn set_run_total(&self, run: i64, total: u64) -> Result<(), Error> {
        sqlx::query("UPDATE scan_runs SET files_total = ? WHERE id = ?")
            .bind(total as i64)
            .bind(run)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn finish_run(
        &self,
        run: i64,
        status: RunStatus,
        counts: &RunCounts,
        error: Option<&str>,
    ) -> Result<(), Error> {
        let sql = concat!(
            "UPDATE scan_runs SET status = ?, files_ok = ?, files_warning = ?, files_error = ?,
               files_unchanged = ?, files_removed = ?, error = ?, finished_at = ",
            now!(),
            " WHERE id = ?"
        );
        sqlx::query(sql)
            .bind(to_db(&status))
            .bind(counts.ok as i64)
            .bind(counts.warning as i64)
            .bind(counts.error as i64)
            .bind(counts.unchanged as i64)
            .bind(counts.removed as i64)
            .bind(error)
            .bind(run)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Une ligne du journal de la passe.
    pub async fn log(
        &self,
        run: i64,
        level: Severity,
        path: Option<&str>,
        code: Option<IssueCode>,
        message: &str,
    ) -> Result<(), Error> {
        let sql = concat!(
            "INSERT INTO scan_run_logs (scan_run_id, at, level, path, code, message)
             VALUES (?, ",
            now!(),
            ", ?, ?, ?, ?)"
        );
        sqlx::query(sql)
            .bind(run)
            .bind(to_db(&level))
            .bind(path)
            .bind(code.map(|c| to_db(&c)))
            .bind(message)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Journalise un fichier rescanné et chacun de ses problèmes. Un fichier inchangé n'est
    /// que compté : son rapport n'a pas été recalculé.
    pub async fn log_file(&self, run: i64, scanned: &Scanned) -> Result<(), Error> {
        if scanned.skipped {
            return Ok(());
        }
        let report = &scanned.report;
        let path = report.path.to_string_lossy();
        let pages = report.pages.len();
        let message = format!("scanné : {}, {pages} page(s)", to_db(&report.status));
        self.log(run, Severity::Info, Some(&path), None, &message)
            .await?;
        for issue in &report.issues {
            let message = match &issue.entry {
                Some(entry) => format!("{entry} : {}", issue.message),
                None => issue.message.clone(),
            };
            self.log(run, issue.severity, Some(&path), Some(issue.code), &message)
                .await?;
        }
        Ok(())
    }

    /// Passes du serveur restées `running` (arrêt brutal) : elles ne reprendront jamais. Les
    /// passes de la CLI ne sont pas touchées : une CLI peut scanner pendant que le serveur démarre.
    pub async fn interrupt_running_runs(&self) -> Result<u64, Error> {
        let sql = concat!(
            "UPDATE scan_runs SET status = 'interrupted', finished_at = ",
            now!(),
            " WHERE status = 'running' AND origin = 'api'"
        );
        Ok(sqlx::query(sql).execute(&self.pool).await?.rows_affected())
    }

    /// Supprime les passes terminées depuis plus de `days` jours, et leur journal. Une passe
    /// dont viennent les problèmes actuels d'un tome (`scan_issues`) est gardée : la supprimer
    /// effacerait ces problèmes en cascade.
    pub async fn purge_runs(&self, days: u32) -> Result<u64, Error> {
        let done = sqlx::query(
            "DELETE FROM scan_runs
             WHERE finished_at IS NOT NULL
               AND finished_at < strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-' || ? || ' days')
               AND id NOT IN (SELECT scan_run_id FROM scan_issues)",
        )
        .bind(days)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }
}

#[cfg(test)]
mod tests;
