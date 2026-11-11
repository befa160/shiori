//! Shiori : CLI de scan d'intégrité et serveur HTTP, dans un seul binaire.

use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use shiori_db::Db;

pub mod scan;
pub mod server;

/// Données de l'utilisateur : `$SHIORI_DATA_DIR`, sinon le dossier de données de l'OS.
fn data_dir() -> Result<PathBuf> {
    if let Some(dir) = std::env::var_os("SHIORI_DATA_DIR").filter(|d| !d.is_empty()) {
        return Ok(PathBuf::from(dir));
    }
    directories::ProjectDirs::from("", "", "shiori")
        .map(|d| d.data_dir().to_path_buf())
        .context("dossier de données introuvable : définir SHIORI_DATA_DIR ou --db")
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
