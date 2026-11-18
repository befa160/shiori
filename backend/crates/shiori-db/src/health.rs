//! Santé des bibliothèques (lot 9.3) : état de chaque tome d'après son dernier scan, fichiers
//! illisibles, dernière vérification complète, liste des fichiers à réparer.

use serde::Serialize;
use shiori_core::{IssueCode, Severity};
use sqlx::Row;

use crate::{Db, Error, from_db};

/// Une bibliothèque vue par la santé.
#[derive(Debug, Serialize)]
pub struct LibraryHealth {
    pub id: i64,
    pub name: String,
    /// Tomes (distincts) par état de leur dernier scan.
    pub ok: i64,
    pub warning: i64,
    pub error: i64,
    /// Fichiers sans tome fiable (illisibles, tronqués avant le hash) : rien à afficher.
    pub unreadable: i64,
    /// Fin de la dernière passe `deep` terminée sur toute la bibliothèque ; `None` : jamais.
    pub deep_checked_at: Option<String>,
}

/// Un code de diagnostic présent, et le nombre de fichiers qui l'ont.
#[derive(Debug, Serialize)]
pub struct CodeCount {
    pub code: IssueCode,
    pub severity: Severity,
    pub files: i64,
}

#[derive(Debug, Serialize)]
pub struct Health {
    pub libraries: Vec<LibraryHealth>,
    /// Avertissements et erreurs seulement, des plus graves aux plus fréquents.
    pub codes: Vec<CodeCount>,
}

#[derive(Debug, Serialize)]
pub struct FileIssue {
    pub severity: Severity,
    pub code: IssueCode,
    pub entry_name: Option<String>,
    pub message: String,
}

/// Un fichier à réparer : un tome en avertissement ou en erreur, ou un fichier illisible.
#[derive(Debug, Serialize)]
pub struct FileHealth {
    pub path: String,
    pub library_id: i64,
    /// `None` pour un fichier illisible (aucun tome, donc aucune page à montrer).
    pub volume_id: Option<String>,
    pub series_id: Option<i64>,
    pub status: Severity,
    pub issues: Vec<FileIssue>,
}

/// Filtre de la liste à réparer, triée par chemin.
#[derive(Debug, Default)]
pub struct HealthFilter {
    pub library_id: Option<i64>,
    pub code: Option<IssueCode>,
    /// Chemins après celui-ci (pagination).
    pub after: Option<String>,
    pub limit: u32,
}

impl Db {
    pub async fn health(&self) -> Result<Health, Error> {
        let rows = sqlx::query(
            "SELECT b.id, b.name, b.deep_checked_at,
                    COUNT(DISTINCT CASE v.status WHEN 'ok' THEN v.id END) AS ok,
                    COUNT(DISTINCT CASE v.status WHEN 'warning' THEN v.id END) AS warning,
                    COUNT(DISTINCT CASE v.status WHEN 'error' THEN v.id END) AS error,
                    (SELECT COUNT(DISTINCT i.path) FROM scan_issues i
                     WHERE i.volume_id IS NULL AND substr(i.path, 1, length(b.root) + 1) = b.root || '/')
                      AS unreadable
             FROM libraries b
             LEFT JOIN file_locations l ON l.library_id = b.id
             LEFT JOIN volumes v ON v.id = l.volume_id
             GROUP BY b.id ORDER BY b.name, b.id",
        )
        .fetch_all(&self.pool)
        .await?;
        let libraries = rows
            .iter()
            .map(|r| LibraryHealth {
                id: r.get("id"),
                name: r.get("name"),
                ok: r.get("ok"),
                warning: r.get("warning"),
                error: r.get("error"),
                unreadable: r.get("unreadable"),
                deep_checked_at: r.get("deep_checked_at"),
            })
            .collect();
        // Problèmes encore d'actualité : ceux d'un tome toujours dans une bibliothèque, ou d'un
        // fichier illisible (nettoyés quand il disparaît, voir `remove_missing`).
        let rows = sqlx::query(
            "SELECT i.code, i.severity, COUNT(DISTINCT coalesce(i.volume_id, i.path)) AS files
             FROM scan_issues i
             WHERE i.severity != 'info' AND (i.volume_id IS NULL OR EXISTS
               (SELECT 1 FROM file_locations l
                WHERE l.volume_id = i.volume_id AND l.library_id IS NOT NULL))
             GROUP BY i.code, i.severity
             ORDER BY i.severity = 'warning', files DESC, i.code",
        )
        .fetch_all(&self.pool)
        .await?;
        let codes = rows
            .iter()
            .map(|r| {
                Ok(CodeCount {
                    code: from_db(r.get::<&str, _>("code"))?,
                    severity: from_db(r.get::<&str, _>("severity"))?,
                    files: r.get("files"),
                })
            })
            .collect::<Result<_, Error>>()?;
        Ok(Health { libraries, codes })
    }

    /// Fichiers à réparer, triés par chemin, avec leurs problèmes.
    pub async fn files_to_repair(&self, filter: &HealthFilter) -> Result<Vec<FileHealth>, Error> {
        let code = filter.code.as_ref().map(crate::to_db);
        let rows = sqlx::query(
            "SELECT * FROM (
               SELECT l.path, l.library_id, v.id AS volume_id, l.series_id, v.status
               FROM file_locations l JOIN volumes v ON v.id = l.volume_id
               WHERE v.status != 'ok' AND l.library_id IS NOT NULL
                 AND (?1 IS NULL OR EXISTS
                   (SELECT 1 FROM scan_issues i WHERE i.volume_id = v.id AND i.code = ?1))
               UNION
               SELECT i.path, b.id, NULL, NULL, 'error'
               FROM scan_issues i JOIN libraries b
                 ON substr(i.path, 1, length(b.root) + 1) = b.root || '/'
               WHERE i.volume_id IS NULL AND (?1 IS NULL OR i.code = ?1)
             )
             WHERE (?2 IS NULL OR library_id = ?2) AND (?3 IS NULL OR path > ?3)
             ORDER BY path LIMIT ?4",
        )
        .bind(code)
        .bind(filter.library_id)
        .bind(&filter.after)
        .bind(filter.limit)
        .fetch_all(&self.pool)
        .await?;
        let mut files = Vec::with_capacity(rows.len());
        for r in &rows {
            let path: String = r.get("path");
            let volume_id: Option<String> = r.get("volume_id");
            let issues = sqlx::query(
                "SELECT severity, code, entry_name, message FROM scan_issues
                 WHERE CASE WHEN ?1 IS NULL THEN volume_id IS NULL AND path = ?2
                            ELSE volume_id = ?1 END
                 ORDER BY CASE severity WHEN 'error' THEN 0 WHEN 'warning' THEN 1 ELSE 2 END, id",
            )
            .bind(&volume_id)
            .bind(&path)
            .fetch_all(&self.pool)
            .await?
            .iter()
            .map(|i| {
                Ok(FileIssue {
                    severity: from_db(i.get::<&str, _>("severity"))?,
                    code: from_db(i.get::<&str, _>("code"))?,
                    entry_name: i.get("entry_name"),
                    message: i.get("message"),
                })
            })
            .collect::<Result<_, Error>>()?;
            files.push(FileHealth {
                path,
                library_id: r.get("library_id"),
                volume_id,
                series_id: r.get("series_id"),
                status: from_db(r.get::<&str, _>("status"))?,
                issues,
            });
        }
        Ok(files)
    }

    /// Note la fin d'une vérification complète (passe `deep` terminée sur toute la bibliothèque).
    pub async fn set_deep_checked(&self, library_id: i64) -> Result<(), Error> {
        let sql = concat!(
            "UPDATE libraries SET deep_checked_at = ",
            now!(),
            " WHERE id = ?"
        );
        sqlx::query(sql)
            .bind(library_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
