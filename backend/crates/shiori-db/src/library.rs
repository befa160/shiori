//! Bibliothèques et séries : rattachement des emplacements scannés à leur série.
//!
//! Une série = un dossier sous la racine (Q9). Les fichiers posés à la racine sont
//! regroupés par nom affiché et rejoignent la série-dossier qui porte le même nom. Le nom
//! stocké d'une série est celui de son premier fichier dans l'ordre de la série : il ne
//! dépend pas de l'ordre du scan.

use std::collections::HashMap;
use std::path::Path;

use shiori_core::{ParsedName, compare_in_series, parse_name};
use sqlx::Row;

use crate::{Db, Error, location_key, to_db};

/// Bibliothèque : un nom et un dossier racine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Library {
    pub id: i64,
    pub name: String,
    /// Chemin absolu, séparateurs `/`, sans `/` final (comme les clés d'emplacement).
    pub root: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("le nom de la bibliothèque est vide")]
    EmptyName,
    #[error("une bibliothèque s'appelle déjà « {0} »")]
    NameTaken(String),
    #[error("{0} n'est pas un dossier accessible")]
    NotADirectory(String),
    #[error("{0} chevauche la bibliothèque « {1} »")]
    Overlaps(String, String),
    #[error(transparent)]
    Db(#[from] Error),
}

/// Chemin relatif à la racine, ou `None` si `key` n'est pas sous `root`.
fn relative<'a>(root: &str, key: &'a str) -> Option<&'a str> {
    key.strip_prefix(root)?.strip_prefix('/')
}

/// Deux racines se chevauchent si l'une est l'autre ou la contient : un fichier ne doit
/// appartenir qu'à une bibliothèque.
fn overlaps(a: &str, b: &str) -> bool {
    a == b || relative(a, b).is_some() || relative(b, a).is_some()
}

fn root_key(root: &Path) -> String {
    location_key(root).trim_end_matches('/').to_owned()
}

impl Db {
    pub async fn create_library(&self, name: &str, root: &Path) -> Result<Library, LibraryError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(LibraryError::EmptyName);
        }
        let key = root_key(root);
        if !std::fs::metadata(root).is_ok_and(|m| m.is_dir()) {
            return Err(LibraryError::NotADirectory(key));
        }
        for other in self.libraries().await? {
            if other.name == name {
                return Err(LibraryError::NameTaken(other.name));
            }
            if overlaps(&other.root, &key) {
                return Err(LibraryError::Overlaps(key, other.name));
            }
        }
        let sql = concat!(
            "INSERT INTO libraries (name, root, created_at) VALUES (?, ?, ",
            now!(),
            ") RETURNING id"
        );
        let id = sqlx::query(sql)
            .bind(name)
            .bind(&key)
            .fetch_one(&self.pool)
            .await?
            .get("id");
        Ok(Library {
            id,
            name: name.to_owned(),
            root: key,
        })
    }

    pub async fn libraries(&self) -> Result<Vec<Library>, Error> {
        let rows = sqlx::query("SELECT id, name, root FROM libraries ORDER BY name, id")
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .iter()
            .map(|r| Library {
                id: r.get("id"),
                name: r.get("name"),
                root: r.get("root"),
            })
            .collect())
    }

    /// Supprime une bibliothèque, ses séries et ses emplacements. Les tomes et les pages
    /// restent (progression future préservée) ; aucun fichier n'est touché.
    pub async fn delete_library(&self, id: i64) -> Result<bool, Error> {
        let done = sqlx::query("DELETE FROM libraries WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(done.rows_affected() > 0)
    }

    /// Rattache l'emplacement d'un fichier déjà scanné à sa série. `false` si le fichier
    /// n'est pas sous la racine ou n'a pas d'emplacement (jamais scanné, illisible avant
    /// le hash, modifié pendant le scan). Un tome en erreur est rattaché : l'erreur doit se voir.
    pub async fn attach(&self, library: &Library, path: &Path) -> Result<bool, Error> {
        let key = location_key(path);
        let Some(rel) = relative(&library.root, &key) else {
            return Ok(false);
        };
        let parsed = parse_name(rel);
        // IMMEDIATE : verrou d'écriture pris d'emblée. Une transaction qui lit avant d'écrire
        // échoue aussitôt (SQLITE_BUSY, sans attente) si un autre fichier a écrit entre-temps.
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let located = sqlx::query("SELECT 1 FROM file_locations WHERE path = ?")
            .bind(&key)
            .fetch_optional(&mut *tx)
            .await?
            .is_some();
        if !located {
            return Ok(false);
        }
        let series: i64 = match &parsed.folder {
            Some(folder) => sqlx::query(
                "INSERT INTO series (library_id, folder, name) VALUES (?, ?, ?)
                 ON CONFLICT (library_id, folder) WHERE folder IS NOT NULL
                 DO UPDATE SET folder = excluded.folder RETURNING id",
            )
            .bind(library.id)
            .bind(folder)
            .bind(&parsed.series)
            .fetch_one(&mut *tx)
            .await?
            .get("id"),
            None => {
                // shortcut: NOCASE ne replie que l'ASCII (`ÉTÉ` ≠ `été`) ; replier en Rust si un cas réel apparaît.
                let existing = sqlx::query(
                    "SELECT id FROM series WHERE library_id = ? AND name = ? COLLATE NOCASE
                     ORDER BY folder IS NULL, id LIMIT 1",
                )
                .bind(library.id)
                .bind(&parsed.series)
                .fetch_optional(&mut *tx)
                .await?;
                match existing {
                    Some(row) => row.get("id"),
                    None => sqlx::query(
                        "INSERT INTO series (library_id, folder, name) VALUES (?, NULL, ?)
                         RETURNING id",
                    )
                    .bind(library.id)
                    .bind(&parsed.series)
                    .fetch_one(&mut *tx)
                    .await?
                    .get("id"),
                }
            }
        };
        sqlx::query(
            "UPDATE file_locations SET library_id = ?, series_id = ?, kind = ?, number = ?,
               number_end = ? WHERE path = ?",
        )
        .bind(library.id)
        .bind(series)
        .bind(to_db(&parsed.kind))
        .bind(parsed.number)
        .bind(parsed.number_end)
        .bind(&key)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(true)
    }

    /// Supprime les emplacements de la bibliothèque situés sous `under` que la passe `run`
    /// n'a pas vus. Renvoie leur nombre. On compare des numéros de passe et non des
    /// horodatages : deux passes peuvent tomber dans la même milliseconde.
    pub async fn remove_missing(
        &self,
        library: &Library,
        under: &Path,
        run: i64,
    ) -> Result<u64, Error> {
        let prefix = root_key(under) + "/";
        let done = sqlx::query(
            "DELETE FROM file_locations
             WHERE library_id = ? AND substr(path, 1, length(?)) = ?
               AND last_seen_run IS NOT ?",
        )
        .bind(library.id)
        .bind(&prefix)
        .bind(&prefix)
        .bind(run)
        .execute(&self.pool)
        .await?;
        Ok(done.rows_affected())
    }

    /// Recalcule le nom de chaque série (celui de son premier fichier), fusionne les
    /// fichiers de la racine dans la série-dossier de même nom et supprime les séries vides.
    pub async fn refresh_series(&self, library: &Library) -> Result<(), Error> {
        let mut tx = self.pool.begin_with("BEGIN IMMEDIATE").await?;
        let rows = sqlx::query(
            "SELECT l.path, l.series_id, s.folder FROM file_locations l
             JOIN series s ON s.id = l.series_id WHERE l.library_id = ?",
        )
        .bind(library.id)
        .fetch_all(&mut *tx)
        .await?;
        let mut first: HashMap<i64, (bool, ParsedName, String)> = HashMap::new();
        for row in &rows {
            let path: String = row.get("path");
            let Some(rel) = relative(&library.root, &path) else {
                continue;
            };
            let parsed = parse_name(rel);
            let is_folder = row.get::<Option<String>, _>("folder").is_some();
            let entry = first.entry(row.get("series_id"));
            let current = entry.or_insert_with(|| (is_folder, parsed.clone(), rel.to_owned()));
            if compare_in_series(&parsed, rel, &current.1, &current.2).is_lt() {
                *current = (is_folder, parsed, rel.to_owned());
            }
        }
        let mut ids: Vec<i64> = first.keys().copied().collect();
        ids.sort_unstable();
        let folder_by_name: HashMap<String, i64> = ids
            .iter()
            .rev()
            .filter(|id| first[id].0)
            .map(|id| (first[id].1.series.to_lowercase(), *id))
            .collect();
        for id in &ids {
            let (is_folder, parsed, _) = &first[id];
            let target = folder_by_name.get(&parsed.series.to_lowercase());
            match target {
                Some(target) if !is_folder => {
                    sqlx::query("UPDATE file_locations SET series_id = ? WHERE series_id = ?")
                        .bind(target)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
                _ => {
                    sqlx::query("UPDATE series SET name = ? WHERE id = ?")
                        .bind(&parsed.series)
                        .bind(id)
                        .execute(&mut *tx)
                        .await?;
                }
            }
        }
        sqlx::query(
            "DELETE FROM series WHERE library_id = ? AND id NOT IN
               (SELECT series_id FROM file_locations WHERE series_id IS NOT NULL)",
        )
        .bind(library.id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await
    }
}

#[cfg(test)]
mod tests;
