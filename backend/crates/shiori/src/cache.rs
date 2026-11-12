//! Cache disque des pages redimensionnées, indexé par (page, largeur, format, version du
//! rendu), et des tomes RAR solid extraits (`extract/<tome>/<index>`, docs/cbr.md). Une page
//! ne change jamais sous un même ID : rien n'est jamais invalidé, on ne fait que libérer de
//! la place, sous un seul plafond (le moins récemment lu part en premier).

use std::fs::{self, File, FileTimes};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use shiori_core::{PageId, VolumeId};

use crate::render::{Format, RENDER_VERSION};

pub struct Images {
    dir: PathBuf,
    max_bytes: u64,
    /// Taille approximative du cache : recalculée exactement à chaque éviction.
    used: AtomicU64,
    /// Une seule éviction à la fois ; les autres requêtes ne l'attendent pas.
    sweeping: Mutex<()>,
    next_tmp: AtomicU64,
    /// Une extraction à la fois : les requêtes simultanées sur un même tome l'attendent au
    /// lieu d'en lancer une autre.
    extracting: Mutex<()>,
    /// Dernier tome extrait lu : jamais évincé, sinon un tome plus gros que le plafond
    /// serait réextrait à chaque page.
    last_extracted: Mutex<Option<String>>,
}

impl Images {
    /// Ouvre (ou crée) le cache dans `dir`, limité à `max_bytes` octets.
    pub fn open(dir: PathBuf, max_bytes: u64) -> io::Result<Self> {
        fs::create_dir_all(dir.join("pages"))?;
        fs::create_dir_all(dir.join("extract"))?;
        // Extractions interrompues ou suppressions inachevées d'un lancement précédent.
        for leftover in fs::read_dir(dir.join("extract"))?.flatten() {
            if leftover.file_name().to_string_lossy().contains('.') {
                let _ = fs::remove_dir_all(leftover.path());
            }
        }
        let used = units(&dir).iter().map(|u| u.size).sum();
        Ok(Self {
            dir,
            max_bytes,
            used: AtomicU64::new(used),
            sweeping: Mutex::new(()),
            next_tmp: AtomicU64::new(0),
            extracting: Mutex::new(()),
            last_extracted: Mutex::new(None),
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

    /// Entrée `index` d'un tome RAR solid, de taille `size` (celle du scan). À la première
    /// demande du tome, `extract` y écrit toutes ses pages (`<dossier>/<index>`) dans un
    /// dossier temporaire, renommé à la fin : jamais d'extraction partielle visible.
    pub fn extracted(
        &self,
        volume: &VolumeId,
        index: usize,
        size: u64,
        extract: impl FnOnce(&Path) -> io::Result<()>,
    ) -> io::Result<Vec<u8>> {
        let volume = volume.to_string();
        *lock(&self.last_extracted) = Some(volume.clone());
        let dir = self.dir.join("extract").join(&volume);
        let file = dir.join(index.to_string());
        match read_sized(&file, size) {
            // Absent parce que le tome n'est pas (ou plus) extrait : on l'extrait.
            Err(e) if e.kind() == io::ErrorKind::NotFound && !dir.exists() => {}
            read => return read,
        }
        {
            let _one = lock(&self.extracting);
            if !dir.exists() {
                let tmp = dir.with_extension("tmp");
                let _ = fs::remove_dir_all(&tmp);
                fs::create_dir_all(&tmp)?;
                let done = extract(&tmp).and_then(|()| fs::rename(&tmp, &dir));
                if let Err(e) = done {
                    let _ = fs::remove_dir_all(&tmp);
                    return Err(e);
                }
                let added = dir_size(&dir).0;
                if self.used.fetch_add(added, Ordering::Relaxed) + added > self.max_bytes {
                    self.evict();
                }
            }
        }
        read_sized(&file, size)
    }

    /// Supprime ce qui a été le moins récemment lu jusqu'à 90 % du plafond.
    fn evict(&self) {
        let Ok(_guard) = self.sweeping.try_lock() else {
            return;
        };
        let keep = lock(&self.last_extracted).clone();
        let mut all = units(&self.dir);
        all.sort_by_key(|u| u.read);
        let mut total: u64 = all.iter().map(|u| u.size).sum();
        let target = self.max_bytes / 10 * 9;
        for unit in &all {
            if total <= target {
                break;
            }
            if unit.is_dir && keep.as_deref().is_some_and(|k| unit.path.ends_with(k)) {
                continue;
            }
            // Un fichier ouvert ailleurs (Windows) est retenté à la prochaine éviction.
            if remove(unit).is_ok() {
                total -= unit.size;
            }
        }
        self.used.store(total, Ordering::Relaxed);
    }
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Lit un fichier du cache, rejeté s'il n'a pas la taille attendue, et rafraîchit sa date
/// de lecture.
fn read_sized(path: &Path, size: u64) -> io::Result<Vec<u8>> {
    let bytes = fs::read(path)?;
    if bytes.len() as u64 != size {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{} : taille différente de celle du scan", path.display()),
        ));
    }
    let _ = File::options()
        .write(true)
        .open(path)
        .and_then(|f| f.set_times(FileTimes::new().set_modified(SystemTime::now())));
    Ok(bytes)
}

/// Ce que l'éviction peut supprimer : un rendu, ou un tome extrait en entier.
struct Unit {
    path: PathBuf,
    size: u64,
    read: SystemTime,
    is_dir: bool,
}

/// Un tome extrait est d'abord renommé (impossible sous Windows si une page est ouverte :
/// on n'efface alors rien), puis effacé : jamais de tome à moitié supprimé encore visible.
fn remove(unit: &Unit) -> io::Result<()> {
    if !unit.is_dir {
        return fs::remove_file(&unit.path);
    }
    let gone = unit.path.with_extension("del");
    let _ = fs::remove_dir_all(&gone); // reste d'une suppression inachevée
    fs::rename(&unit.path, &gone)?;
    let _ = fs::remove_dir_all(&gone);
    Ok(())
}

/// (taille totale, dernière lecture) des fichiers d'un dossier.
fn dir_size(dir: &Path) -> (u64, SystemTime) {
    let mut out = (0, SystemTime::UNIX_EPOCH);
    for file in fs::read_dir(dir).into_iter().flatten().flatten() {
        if let Ok(meta) = file.metadata()
            && meta.is_file()
        {
            out.0 += meta.len();
            out.1 = out.1.max(meta.modified().unwrap_or(SystemTime::UNIX_EPOCH));
        }
    }
    out
}

/// Tous les rendus (`pages/<shard>/<fichier>`) et tous les tomes extraits terminés.
fn units(dir: &Path) -> Vec<Unit> {
    let mut out = Vec::new();
    for shard in fs::read_dir(dir.join("pages"))
        .into_iter()
        .flatten()
        .flatten()
    {
        for file in fs::read_dir(shard.path()).into_iter().flatten().flatten() {
            if let Ok(meta) = file.metadata()
                && meta.is_file()
            {
                out.push(Unit {
                    path: file.path(),
                    size: meta.len(),
                    read: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    is_dir: false,
                });
            }
        }
    }
    for volume in fs::read_dir(dir.join("extract"))
        .into_iter()
        .flatten()
        .flatten()
    {
        // `<tome>.tmp` (extraction en cours) et `<tome>.del` ne comptent pas.
        if volume.file_name().to_string_lossy().contains('.') {
            continue;
        }
        let (size, read) = dir_size(&volume.path());
        out.push(Unit {
            path: volume.path(),
            size,
            read,
            is_dir: true,
        });
    }
    out
}

#[cfg(test)]
mod tests;
