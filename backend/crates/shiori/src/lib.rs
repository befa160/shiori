//! Shiori : CLI de scan d'intégrité et serveur HTTP, dans un seul binaire.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use shiori_db::Db;

/// Numéro de la release installée (`0.5.0-rc.2`), donné par la compilation d'un tag
/// (`SHIORI_RELEASE`, workflow de release) ; sinon la version du code.
pub const RELEASE: &str = match option_env!("SHIORI_RELEASE") {
    Some(release) => release,
    None => env!("CARGO_PKG_VERSION"),
};

pub mod backup;
pub mod cache;
pub mod job;
pub mod meta;
pub mod render;
pub mod scan;
pub mod server;
pub mod user;
pub mod watch;
pub mod web;

/// Données de l'utilisateur : `$SHIORI_DATA_DIR`, sinon le dossier de données de l'OS.
fn data_dir() -> Result<PathBuf> {
    user_dir(
        "SHIORI_DATA_DIR",
        std::env::var_os("SHIORI_DATA_DIR"),
        |d| d.data_dir(),
    )
}

/// Cache : `$SHIORI_CACHE_DIR`, sinon le dossier de cache de l'OS.
fn cache_dir() -> Result<PathBuf> {
    user_dir(
        "SHIORI_CACHE_DIR",
        std::env::var_os("SHIORI_CACHE_DIR"),
        |d| d.cache_dir(),
    )
}

/// La variable `var` si elle est définie et non vide, sinon le dossier de l'OS choisi par `pick`.
fn user_dir(
    var: &str,
    value: Option<OsString>,
    pick: fn(&ProjectDirs) -> &Path,
) -> Result<PathBuf> {
    if let Some(dir) = value.filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    ProjectDirs::from("", "", "shiori")
        .map(|d| pick(&d).to_path_buf())
        .with_context(|| format!("dossier de l'utilisateur introuvable : définir {var}"))
}

/// Sauvegardes : `dir`, sinon `$SHIORI_BACKUP_DIR`, sinon `backups` à côté de la base.
fn backup_dir(db: &Path, dir: Option<PathBuf>) -> PathBuf {
    dir.or_else(|| {
        std::env::var_os("SHIORI_BACKUP_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
    })
    .unwrap_or_else(|| db.parent().unwrap_or(Path::new("")).join("backups"))
}

/// Ouvre la base `path`, ou `<dossier de données>/shiori.db`, en créant son dossier ; une base
/// à migrer est d'abord sauvegardée dans le dossier des sauvegardes (`backups`, voir
/// [`backup_dir`]), renvoyé avec elle.
async fn open_db(path: Option<PathBuf>, backups: Option<PathBuf>) -> Result<(Db, PathBuf)> {
    let path = match path {
        Some(path) => path,
        None => data_dir()?.join("shiori.db"),
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| format!("création de {}", parent.display()))?;
    }
    let backups = backup_dir(&path, backups);
    let db = Db::open_backing_up(&path, Some(&backups))
        .await
        .with_context(|| format!("ouverture de la base {}", path.display()))?;
    Ok((db, backups))
}

#[cfg(test)]
mod tests;
