//! Dossier des sauvegardes : `shiori-<date>.db` (périodiques ou demandées, les `keep` plus
//! récentes gardées), `shiori-avant-…` (avant une migration ou une restauration) et
//! `shiori-importee-…` (téléversées), jamais effacées d'office. Un outil de sauvegarde
//! (Backrest, restic) n'a qu'à copier ce dossier.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Result;
use clap::Args;
use serde::Serialize;
use shiori_db::Db;

/// Préfixe des sauvegardes soumises à la rotation.
const ROUTINE: &str = "shiori";

#[derive(Clone, Debug)]
pub struct Backups {
    pub dir: PathBuf,
    /// `None` : pas de sauvegarde périodique.
    pub every: Option<Duration>,
    pub keep: usize,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct BackupFile {
    pub name: String,
    pub size: u64,
    /// Date de modification, en millisecondes depuis l'époque Unix.
    pub modified_ms: u64,
}

impl Backups {
    /// Sauvegarde du jour (périodique ou demandée), puis rotation.
    pub async fn create(&self, db: &Db) -> Result<BackupFile, shiori_db::Error> {
        let path = db.backup(&self.dir, ROUTINE).await?;
        self.prune();
        file(&path).ok_or_else(|| std::io::Error::other("sauvegarde disparue").into())
    }

    /// Les sauvegardes, de la plus récente à la plus ancienne (nom horodaté).
    pub fn list(&self) -> Vec<BackupFile> {
        let mut files: Vec<BackupFile> = std::fs::read_dir(&self.dir)
            .into_iter()
            .flatten()
            .filter_map(|e| file(&e.ok()?.path()))
            .collect();
        files.sort_by(|a, b| b.modified_ms.cmp(&a.modified_ms).then(b.name.cmp(&a.name)));
        files
    }

    /// Chemin d'une sauvegarde du dossier, `None` pour tout autre nom (`..`, chemin, fichier caché).
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        let path = self.dir.join(name);
        (valid_name(name) && path.is_file()).then_some(path)
    }

    /// Efface les sauvegardes de routine au-delà des `keep` plus récentes.
    fn prune(&self) {
        let mut routine: Vec<String> = self
            .list()
            .into_iter()
            .map(|f| f.name)
            .filter(|n| is_routine(n))
            .collect();
        routine.sort_unstable_by(|a, b| b.cmp(a));
        for name in routine.iter().skip(self.keep.max(1)) {
            if let Err(e) = std::fs::remove_file(self.dir.join(name)) {
                tracing::warn!("rotation des sauvegardes, {name} : {e}");
            }
        }
    }

    /// Temps avant la prochaine sauvegarde périodique : `every` après la plus récente.
    fn due_in(&self, every: Duration) -> Duration {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let last = self
            .list()
            .into_iter()
            .filter(|f| is_routine(&f.name))
            .map(|f| Duration::from_millis(f.modified_ms))
            .max();
        last.map_or(Duration::ZERO, |last| (last + every).saturating_sub(now))
    }

    /// Sauvegarde périodique, pour toute la vie du serveur. Un redémarrage n'en refait pas une
    /// si la dernière est assez récente.
    pub fn spawn(self, db: Arc<Db>) {
        let Some(every) = self.every else { return };
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(self.due_in(every)).await;
                match self.create(&db).await {
                    Ok(file) => tracing::info!("sauvegarde {}", file.name),
                    Err(e) => {
                        tracing::error!("sauvegarde périodique : {e}");
                        // Disque plein, dossier retiré : on réessaie au prochain intervalle.
                        tokio::time::sleep(every).await;
                    }
                }
            }
        });
    }
}

#[derive(Args)]
pub struct BackupArgs {
    /// Base SQLite (défaut : <dossier de données>/shiori.db, ou $SHIORI_DATA_DIR/shiori.db).
    #[arg(long, value_name = "FICHIER")]
    db: Option<PathBuf>,
    /// Dossier des sauvegardes (défaut : `backups` à côté de la base).
    #[arg(long, value_name = "DOSSIER", env = "SHIORI_BACKUP_DIR")]
    dir: Option<PathBuf>,
}

/// `shiori backup` : une sauvegarde tout de suite, Shiori allumé ou non (avant une
/// sauvegarde Backrest, par exemple) ; affiche son chemin.
pub async fn run(args: BackupArgs) -> Result<()> {
    let (db, dir) = crate::open_db(args.db, args.dir).await?;
    println!("{}", db.backup(&dir, ROUTINE).await?.display());
    Ok(())
}

/// `shiori-AAAAMMJJ-HHMMSS.db`.
fn is_routine(name: &str) -> bool {
    name.strip_prefix(ROUTINE)
        .and_then(|rest| rest.strip_prefix('-'))
        .is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

fn valid_name(name: &str) -> bool {
    name.ends_with(".db")
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
}

fn file(path: &Path) -> Option<BackupFile> {
    let name = path.file_name()?.to_str()?;
    let meta = std::fs::metadata(path).ok().filter(|m| m.is_file())?;
    let modified_ms = meta
        .modified()
        .ok()?
        .duration_since(UNIX_EPOCH)
        .ok()?
        .as_millis() as u64;
    valid_name(name).then(|| BackupFile {
        name: name.to_owned(),
        size: meta.len(),
        modified_ms,
    })
}

#[cfg(test)]
mod tests;
