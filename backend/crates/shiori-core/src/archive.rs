//! Accès aux archives de BD (principes 5 et 6).
//!
//! Les fichiers sont ouverts en lecture seule et ne sont jamais modifiés. Le CBZ est lu
//! via l'annuaire central : chaque entrée est accessible directement par son index,
//! sans extraction complète, zip64 compris. Le CBR (RAR) passe par le même trait : voir
//! [`rar`].

use std::collections::BTreeMap;
use std::fs::File;
use std::io::{self, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use zip::result::ZipError;
use zip::{CompressionMethod, ZipArchive};

pub mod rar;
pub use rar::RarArchive;

/// Nature d'une entrée d'archive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    File,
    Dir,
    Symlink,
}

/// Métadonnées d'une entrée, telles que déclarées par l'annuaire central.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Index d'accès direct dans `zip`. Les doublons de nom y sont fusionnés : ce n'est
    /// pas la position brute dans l'annuaire central.
    pub index: usize,
    /// Nom décodé (affichage, tri, diagnostics) : UTF-8, sinon CP437.
    pub name: String,
    /// Octets bruts du nom : base de l'identité des pages.
    pub name_raw: Vec<u8>,
    pub kind: EntryKind,
    pub compressed_size: u64,
    pub size: u64,
    pub crc32: u32,
    /// Nom lisible de la méthode de compression.
    pub compression: String,
    pub compression_supported: bool,
    pub encrypted: bool,
}

/// Taille maximale d'une entrée lue en mémoire : une page, même de webtoon, reste
/// très en dessous. Protège contre les bombes zip.
pub const MAX_ENTRY_SIZE: u64 = 512 * 1024 * 1024;

/// Un nom présent plusieurs fois dans l'annuaire central.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplicate {
    pub name: String,
    pub name_raw: Vec<u8>,
    /// Vrai si toutes les occurrences annoncent le même contenu (CRC et tailles).
    pub identical: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("archive illisible : {0}")]
    Unreadable(String),
    #[error("archive tronquée : {0}")]
    Truncated(String),
    #[error("entrée chiffrée")]
    Encrypted,
    #[error("méthode de compression non gérée : {0}")]
    UnsupportedCompression(String),
    #[error("CRC32 incorrect : le contenu ne correspond pas à l'annuaire")]
    CrcMismatch,
    #[error("entrée trop grosse : {0} octets déclarés (maximum {MAX_ENTRY_SIZE})")]
    EntryTooLarge(u64),
    #[error("l'entrée contient plus d'octets que sa taille déclarée")]
    SizeMismatch,
    #[error("aucune entrée d'index {0}")]
    NoSuchEntry(usize),
    #[error(transparent)]
    Io(io::Error),
}

impl ArchiveError {
    /// Interprète une erreur survenue pendant la lecture du contenu d'une entrée.
    pub fn from_read_error(err: io::Error) -> Self {
        // `zip` ne signale ces deux cas que par leur message (crc32.rs).
        let invalid_data = err.kind() == io::ErrorKind::InvalidData;
        if invalid_data && err.to_string() == "Invalid checksum" {
            Self::CrcMismatch
        } else if invalid_data
            && err.to_string() == "File is larger than its declared uncompressed size"
        {
            Self::SizeMismatch
        } else if err.kind() == io::ErrorKind::UnexpectedEof {
            Self::Truncated(err.to_string())
        } else {
            Self::Io(err)
        }
    }
}

impl From<ZipError> for ArchiveError {
    fn from(err: ZipError) -> Self {
        match err {
            ZipError::Io(e) => Self::from_read_error(e),
            ZipError::InvalidArchive(msg) => Self::Unreadable(msg.into_owned()),
            ZipError::UnsupportedArchive(msg) => Self::Unreadable(msg.to_owned()),
            ZipError::CompressionMethodNotSupported(m) => {
                Self::UnsupportedCompression(format!("méthode {m}"))
            }
            ZipError::InvalidPassword => Self::Encrypted,
            other => Self::Unreadable(other.to_string()),
        }
    }
}

/// Archive dont on peut lister et lire les entrées.
pub trait Archive {
    /// Entrées dans l'ordre de l'annuaire central.
    fn entries(&self) -> &[Entry];

    /// Noms déclarés plusieurs fois dans l'annuaire. `entries()` n'expose qu'une entrée
    /// par nom : l'appelant doit signaler ces doublons.
    fn duplicates(&self) -> Result<Vec<Duplicate>, ArchiveError>;

    /// Flux du contenu décompressé d'une entrée. Le CRC est vérifié à la fin de la
    /// lecture : une erreur se convertit avec [`ArchiveError::from_read_error`].
    fn open_entry(&mut self, index: usize) -> Result<Box<dyn Read + '_>, ArchiveError>;

    /// Contenu complet d'une entrée, CRC vérifié, jamais plus que sa taille déclarée
    /// ni plus de [`MAX_ENTRY_SIZE`].
    fn read_entry(&mut self, index: usize) -> Result<Vec<u8>, ArchiveError> {
        let size = self
            .entries()
            .get(index)
            .ok_or(ArchiveError::NoSuchEntry(index))?
            .size;
        if size > MAX_ENTRY_SIZE {
            return Err(ArchiveError::EntryTooLarge(size));
        }
        // `zip` arrête la lecture dès qu'elle dépasse la taille déclarée (`SizeMismatch`) :
        // la mémoire reste bornée par `size`.
        let mut out = Vec::with_capacity(size as usize);
        self.open_entry(index)?
            .read_to_end(&mut out)
            .map_err(ArchiveError::from_read_error)?;
        Ok(out)
    }

    /// Lire une entrée oblige à décoder toutes celles qui la précèdent (RAR solid).
    fn is_solid(&self) -> bool {
        false
    }

    /// Archive découpée en plusieurs fichiers (`.part2.rar`…), non gérée.
    fn is_multivolume(&self) -> bool {
        false
    }

    /// Lit chacune des entrées `indices` et remet son contenu (ou son erreur) à `visit`,
    /// dans l'ordre de l'archive, une seule fois chacune. Une archive solid est décodée en
    /// une seule passe au lieu d'une par entrée.
    fn read_each(
        &mut self,
        indices: &[usize],
        visit: &mut dyn FnMut(usize, Result<Vec<u8>, ArchiveError>),
    ) {
        let mut sorted = indices.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        for i in sorted {
            let read = self.read_entry(i);
            visit(i, read);
        }
    }
}

/// Le format se reconnaît à son contenu, pas à l'extension : bien des `.cbr` sont des zip.
pub fn is_rar(path: &Path) -> bool {
    let mut magic = [0u8; 6];
    File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == *b"Rar!\x1a\x07"
}

/// Ouvre un CBZ ou un CBR selon son contenu.
pub fn open(path: &Path) -> Result<Box<dyn Archive + Send>, ArchiveError> {
    Ok(if is_rar(path) {
        Box::new(RarArchive::open(path)?)
    } else {
        Box::new(CbzArchive::open(path)?)
    })
}

/// Archive CBZ (zip).
pub struct CbzArchive {
    path: PathBuf,
    zip: ZipArchive<BufReader<File>>,
    entries: Vec<Entry>,
}

fn is_supported(method: CompressionMethod) -> bool {
    matches!(
        method,
        CompressionMethod::Stored
            | CompressionMethod::Deflated
            | CompressionMethod::Deflate64
            | CompressionMethod::Lzma
            | CompressionMethod::Xz
            | CompressionMethod::Ppmd
    )
}

impl CbzArchive {
    /// Ouvre une archive en lecture seule et lit son annuaire central.
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let file = File::open(path).map_err(ArchiveError::Io)?;
        let zip = ZipArchive::new(BufReader::new(file))?;
        let entries = (0..zip.len())
            .map(|index| {
                let e = zip.by_index_data(index)?;
                let name_raw = e.name_raw().to_vec();
                let kind = if e.is_symlink() {
                    EntryKind::Symlink
                } else if e.is_dir() {
                    EntryKind::Dir
                } else {
                    EntryKind::File
                };
                Ok(Entry {
                    index,
                    name: e
                        .name()
                        .map(|n| n.into_owned())
                        .unwrap_or_else(|_| String::from_utf8_lossy(&name_raw).into_owned()),
                    name_raw,
                    kind,
                    compressed_size: e.compressed_size(),
                    size: e.size(),
                    crc32: e.crc32(),
                    compression: format!("{:?}", e.compression()),
                    compression_supported: is_supported(e.compression()),
                    encrypted: e.encrypted(),
                })
            })
            .collect::<Result<_, ZipError>>()?;
        Ok(Self {
            path: path.to_path_buf(),
            zip,
            entries,
        })
    }
}

impl Archive for CbzArchive {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn duplicates(&self) -> Result<Vec<Duplicate>, ArchiveError> {
        if !self.zip.has_duplicate_names() {
            return Ok(Vec::new());
        }
        // `zip` a fusionné les doublons : on relit l'annuaire central pour les retrouver.
        let mut cd = BufReader::new(File::open(&self.path).map_err(ArchiveError::Io)?);
        cd.seek(SeekFrom::Start(self.zip.central_directory_start()))
            .map_err(ArchiveError::Io)?;
        let mut by_name: BTreeMap<Vec<u8>, Vec<[u8; 12]>> = BTreeMap::new();
        let mut header = [0u8; 46];
        while cd.read_exact(&mut header).is_ok() && header[..4] == *b"PK\x01\x02" {
            let len = |at: usize| usize::from(u16::from_le_bytes([header[at], header[at + 1]]));
            let mut name = vec![0; len(28)];
            cd.read_exact(&mut name)
                .map_err(ArchiveError::from_read_error)?;
            cd.seek_relative((len(30) + len(32)) as i64)
                .map_err(ArchiveError::Io)?;
            // shortcut: compare CRC + tailles 32 bits ; les tailles zip64 (> 4 Gio) ne sont
            // pas lues, inutile tant qu'une entrée est plafonnée à MAX_ENTRY_SIZE.
            let mut fingerprint = [0u8; 12];
            fingerprint.copy_from_slice(&header[16..28]);
            by_name.entry(name).or_default().push(fingerprint);
        }
        Ok(by_name
            .into_iter()
            .filter(|(_, prints)| prints.len() > 1)
            .map(|(name_raw, prints)| Duplicate {
                name: self
                    .entries
                    .iter()
                    .find(|e| e.name_raw == name_raw)
                    .map_or_else(
                        || String::from_utf8_lossy(&name_raw).into_owned(),
                        |e| e.name.clone(),
                    ),
                identical: prints.windows(2).all(|w| w[0] == w[1]),
                name_raw,
            })
            .collect())
    }

    fn open_entry(&mut self, index: usize) -> Result<Box<dyn Read + '_>, ArchiveError> {
        let entry = self
            .entries
            .get(index)
            .ok_or(ArchiveError::NoSuchEntry(index))?;
        if entry.encrypted {
            return Err(ArchiveError::Encrypted);
        }
        if !entry.compression_supported {
            return Err(ArchiveError::UnsupportedCompression(
                entry.compression.clone(),
            ));
        }
        Ok(Box::new(self.zip.by_index(index)?))
    }
}

#[cfg(test)]
mod tests;
