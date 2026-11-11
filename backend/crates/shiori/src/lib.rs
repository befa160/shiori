//! Shiori : CLI de scan d'intégrité et serveur HTTP, dans un seul binaire.

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;
use shiori_db::Db;

pub mod cache;
pub mod job;
pub mod render;
pub mod scan;
pub mod server;

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

/// Ouvre la base `path`, ou `<dossier de données>/shiori.db`, en créant son dossier.
async fn open_db(path: Option<PathBuf>) -> Result<Db> {
    let path = match path {
        Some(path) => path,
        None => data_dir()?.join("shiori.db"),
    };
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).with_context(|| format!("création de {}", parent.display()))?;
    }
    Db::open(&path)
        .await
        .with_context(|| format!("ouverture de la base {}", path.display()))
}

#[cfg(test)]
mod tests;
