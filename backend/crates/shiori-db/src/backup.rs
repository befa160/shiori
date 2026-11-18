//! Sauvegarde et restauration de la base, Shiori allumé.
//!
//! - Sauvegarde : `VACUUM INTO`, copie cohérente même pendant un scan, écrite sous un nom
//!   temporaire puis renommée (un outil de sauvegarde ne voit jamais un fichier à moitié écrit).
//! - Restauration : la sauvegarde est copiée, vérifiée et migrée à part, puis son contenu
//!   remplace celui de la base en une transaction ; aucun fichier n'est échangé sous SQLite.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use sqlx::{AssertSqlSafe, Connection, SqliteConnection};

use crate::{Db, Error};

/// Une restauration refusée : rien n'a été modifié.
#[derive(Debug, thiserror::Error)]
pub enum RestoreError {
    #[error("ce fichier n'est pas une sauvegarde de Shiori lisible")]
    NotABackup,
    #[error("sauvegarde d'une version plus récente de Shiori : mettez Shiori à jour d'abord")]
    TooNew,
    #[error(transparent)]
    Db(#[from] Error),
}

impl Db {
    /// Écrit une copie cohérente de la base dans `dir`, nommée `<prefix>-AAAAMMJJ-HHMMSS.db`
    /// (heure UTC), et renvoie son chemin. Lisible du seul propriétaire sous Unix : la base
    /// contient les hachés des mots de passe et les secrets de double authentification.
    pub async fn backup(&self, dir: &Path, prefix: &str) -> Result<PathBuf, Error> {
        std::fs::create_dir_all(dir)?;
        let dest = self.free_name(dir, prefix).await?;
        let part = temp_file(dir);
        // Reste d'un processus précédent au même numéro : `VACUUM INTO` refuse un fichier existant.
        let _ = std::fs::remove_file(&part);
        let written = sqlx::query("VACUUM INTO ?")
            .bind(utf8(&part)?)
            .execute(&self.pool)
            .await;
        if let Err(e) = written {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&part, std::fs::Permissions::from_mode(0o600))?;
        }
        std::fs::rename(&part, &dest)?;
        Ok(dest)
    }

    /// `dir/<prefix>-AAAAMMJJ-HHMMSS.db` (UTC, triable), suffixé `-2`, `-3`… si ce nom est
    /// pris : deux fichiers de la même seconde ne s'écrasent jamais.
    pub async fn free_name(&self, dir: &Path, prefix: &str) -> Result<PathBuf, Error> {
        let stamp: String = sqlx::query_scalar("SELECT strftime('%Y%m%d-%H%M%S', 'now')")
            .fetch_one(&self.pool)
            .await?;
        let mut path = dir.join(format!("{prefix}-{stamp}.db"));
        let mut n = 1;
        while path.exists() {
            n += 1;
            path = dir.join(format!("{prefix}-{stamp}-{n}.db"));
        }
        Ok(path)
    }

    /// Remplace tout le contenu de la base par celui de la sauvegarde `source`, mise d'abord au
    /// schéma actuel sur une copie de travail dans `work_dir` (`source` n'est jamais modifiée).
    /// Toutes les sessions sont fermées : on se reconnecte avec les comptes restaurés.
    pub async fn restore(&self, source: &Path, work_dir: &Path) -> Result<(), RestoreError> {
        let work = temp_file(work_dir);
        remove_db(&work);
        std::fs::copy(source, &work).map_err(Error::Io)?;
        let outcome = self.restore_from(&work).await;
        remove_db(&work);
        outcome
    }

    async fn restore_from(&self, work: &Path) -> Result<(), RestoreError> {
        prepare(work).await?;
        let mut conn = self.pool.acquire().await?;
        sqlx::query("ATTACH DATABASE ? AS src")
            .bind(utf8(work)?.to_owned())
            .execute(&mut *conn)
            .await?;
        let copied = copy_all(&mut conn).await;
        sqlx::query("DETACH DATABASE src")
            .execute(&mut *conn)
            .await?;
        copied
    }
}

/// Vérifie que `work` est une base Shiori intacte, puis la met au schéma actuel.
async fn prepare(work: &Path) -> Result<(), RestoreError> {
    let options = SqliteConnectOptions::new().filename(work);
    // Pool d'une connexion plutôt qu'une connexion seule : la migration d'une connexion
    // empruntée rend le futur non `Send` (bornes de durée de vie de sqlx).
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(options)
        .await
        .map_err(|_| RestoreError::NotABackup)?;
    let checked = check_and_migrate(&pool).await;
    pool.close().await;
    checked
}

async fn check_and_migrate(pool: &SqlitePool) -> Result<(), RestoreError> {
    // Une base SQLite quelconque serait migrée comme une base neuve, puis restaurée vide.
    let integrity: Result<String, Error> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(pool)
        .await;
    let first: Result<i64, Error> =
        sqlx::query_scalar("SELECT COUNT(*) FROM _sqlx_migrations WHERE version = 1")
            .fetch_one(pool)
            .await;
    if !matches!((integrity.as_deref(), first), (Ok("ok"), Ok(1))) {
        return Err(RestoreError::NotABackup);
    }
    sqlx::migrate!().run(pool).await.map_err(|e| match e {
        sqlx::migrate::MigrateError::VersionMissing(_) => RestoreError::TooNew,
        _ => RestoreError::NotABackup,
    })
}

/// Vide chaque table de la base et la remplit depuis `src`, en une transaction.
async fn copy_all(conn: &mut SqliteConnection) -> Result<(), RestoreError> {
    type Schema = Vec<(String, String, Option<String>)>;
    let ours: Schema = sqlx::query_as(
        "SELECT type, name, sql FROM main.sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(&mut *conn)
    .await?;
    let theirs: Schema = sqlx::query_as(
        "SELECT type, name, sql FROM src.sqlite_master
         WHERE name NOT LIKE 'sqlite_%' ORDER BY type, name",
    )
    .fetch_all(&mut *conn)
    .await?;
    // Mêmes migrations, même schéma ; sinon les colonnes de `SELECT *` ne correspondraient pas.
    if ours != theirs {
        return Err(RestoreError::NotABackup);
    }
    let tables: Vec<&str> = ours
        .iter()
        .filter(|(kind, name, _)| kind == "table" && name != "_sqlx_migrations")
        .map(|(_, name, _)| name.as_str())
        .collect();
    let mut tx = conn.begin().await?;
    // Tables vidées puis remplies dans n'importe quel ordre : clés étrangères vérifiées au commit.
    sqlx::query("PRAGMA defer_foreign_keys = ON")
        .execute(&mut *tx)
        .await?;
    for table in &tables {
        // Noms tirés du schéma de la base elle-même, pas d'une entrée extérieure.
        sqlx::query(AssertSqlSafe(format!("DELETE FROM main.\"{table}\"")))
            .execute(&mut *tx)
            .await?;
    }
    for table in tables.iter().filter(|t| **t != "sessions") {
        sqlx::query(AssertSqlSafe(format!(
            "INSERT INTO main.\"{table}\" SELECT * FROM src.\"{table}\""
        )))
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Fichier temporaire caché de `dir`, jamais le même deux fois dans ce processus : deux
/// sauvegardes, restaurations ou téléversements simultanés n'écrivent pas au même endroit.
pub fn temp_file(dir: &Path) -> PathBuf {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    dir.join(format!(
        ".tmp-{}.part",
        NEXT.fetch_add(1, Ordering::Relaxed)
    ))
}

/// Une base et ses fichiers WAL, sans erreur s'ils n'existent pas.
fn remove_db(path: &Path) {
    for suffix in ["", "-wal", "-shm"] {
        let mut file = path.as_os_str().to_owned();
        file.push(suffix);
        let _ = std::fs::remove_file(file);
    }
}

fn utf8(path: &Path) -> Result<&str, Error> {
    path.to_str().ok_or_else(|| {
        Error::Io(std::io::Error::other(format!(
            "chemin non UTF-8 : {}",
            path.display()
        )))
    })
}

#[cfg(test)]
mod tests;
