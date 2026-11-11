//! Cache disque des pages redimensionnées, indexé par (page, largeur, format, version du
//! rendu). Une page ne change jamais sous un même ID : rien n'est jamais invalidé, on ne
//! fait que libérer de la place (les fichiers les moins récemment lus partent en premier).

use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use shiori_core::PageId;

use crate::render::{Format, RENDER_VERSION};

pub struct Images {
    dir: PathBuf,
    max_bytes: u64,
    /// Taille approximative du cache : recalculée exactement à chaque éviction.
    used: AtomicU64,
    /// Une seule éviction à la fois ; les autres requêtes ne l'attendent pas.
    sweeping: Mutex<()>,
    next_tmp: AtomicU64,
}

impl Images {
    /// Ouvre (ou crée) le cache dans `dir`, limité à `max_bytes` octets.
    pub fn open(dir: PathBuf, max_bytes: u64) -> io::Result<Self> {
        fs::create_dir_all(dir.join("pages"))?;
        let used = files(&dir.join("pages")).iter().map(|f| f.1).sum();
        Ok(Self {
            dir,
            max_bytes,
            used: AtomicU64::new(used),
            sweeping: Mutex::new(()),
            next_tmp: AtomicU64::new(0),
        })
    }

    pub fn used(&self) -> u64 {
        self.used.load(Ordering::Relaxed)
    }

    /// `<dir>/pages/<2 premiers caractères>/<page>-w<largeur>-v<version>.<ext>` : l'ID est
    /// en hexadécimal validé, aucun chemin ne peut sortir du cache.
    fn path(&self, page: &PageId, width: u32, format: Format) -> PathBuf {
        let id = page.to_string();
        self.dir.join("pages").join(&id[..2]).join(format!(
            "{id}-w{width}-v{RENDER_VERSION}.{}",
            format.extension()
        ))
    }

    /// Contenu en cache, et date de lecture rafraîchie (base de l'éviction).
    pub fn get(&self, page: &PageId, width: u32, format: Format) -> Option<Vec<u8>> {
        let path = self.path(page, width, format);
        let bytes = fs::read(&path).ok()?;
        // Échec sans gravité : le fichier partira simplement plus tôt.
        let _ = File::options()
            .write(true)
            .open(&path)
            .and_then(|f| f.set_times(FileTimes::new().set_modified(SystemTime::now())));
        Some(bytes)
    }

    /// Écrit un rendu (fichier temporaire puis renommage : jamais de fichier partiel
    /// visible), puis libère de la place si le plafond est dépassé.
    pub fn put(&self, page: &PageId, width: u32, format: Format, bytes: &[u8]) -> io::Result<()> {
        let path = self.path(page, width, format);
        let dir = path.parent().expect("chemin du cache dans un sous-dossier");
        fs::create_dir_all(dir)?;
        let n = self.next_tmp.fetch_add(1, Ordering::Relaxed);
        let tmp = dir.join(format!(".{}.{n}.tmp", std::process::id()));
        fs::write(&tmp, bytes)?;
        if let Err(e) = fs::rename(&tmp, &path) {
            let _ = fs::remove_file(&tmp);
            return Err(e);
        }
        let used = self.used.fetch_add(bytes.len() as u64, Ordering::Relaxed) + bytes.len() as u64;
        if used > self.max_bytes {
            self.evict();
        }
        Ok(())
    }

    /// Supprime les fichiers les moins récemment lus jusqu'à 90 % du plafond.
    fn evict(&self) {
        let Ok(_guard) = self.sweeping.try_lock() else {
            return;
        };
        let mut all = files(&self.dir.join("pages"));
        all.sort_by_key(|f| f.2);
        let mut total: u64 = all.iter().map(|f| f.1).sum();
        let target = self.max_bytes / 10 * 9;
        for (path, size, _) in &all {
            if total <= target {
                break;
            }
            // Un fichier ouvert ailleurs (Windows) est retenté à la prochaine éviction.
            if fs::remove_file(path).is_ok() {
                total -= size;
            }
        }
        self.used.store(total, Ordering::Relaxed);
    }
}

/// (chemin, taille, date de modification) de chaque fichier du cache.
fn files(pages: &Path) -> Vec<(PathBuf, u64, SystemTime)> {
    let mut out = Vec::new();
    for shard in fs::read_dir(pages).into_iter().flatten().flatten() {
        for file in fs::read_dir(shard.path()).into_iter().flatten().flatten() {
            if let Ok(meta) = file.metadata()
                && meta.is_file()
            {
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                out.push((file.path(), meta.len(), mtime));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests;
