//! Accès aux archives de BD (principes 5 et 6).
//!
//! Les fichiers sont ouverts en lecture seule et ne sont jamais modifiés. Le CBZ est lu
//! via l'annuaire central : chaque entrée est accessible directement par son index,
//! sans extraction complète, zip64 compris. Le CBR viendra derrière le même trait.

use std::fs::File;
use std::io::{self, BufReader, Read};
use std::path::Path;

use zip::result::ZipError;
use zip::{CompressionMethod, ZipArchive};

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
    /// Position dans l'annuaire, utilisée pour l'accès direct.
    pub index: usize,
    /// Nom décodé (affichage, tri, diagnostics). Remplacement avec `U+FFFD` si le nom
    /// n'est pas décodable.
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
    #[error("aucune entrée d'index {0}")]
    NoSuchEntry(usize),
    #[error(transparent)]
    Io(io::Error),
}

impl ArchiveError {
    /// Interprète une erreur survenue pendant la lecture du contenu d'une entrée.
    pub fn from_read_error(err: io::Error) -> Self {
        // `zip` ne signale un CRC faux que par ce message (crc32.rs, `invalid_checksum!`).
        if err.kind() == io::ErrorKind::InvalidData && err.to_string() == "Invalid checksum" {
            Self::CrcMismatch
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

    /// Vrai si l'annuaire déclare plusieurs entrées sous le même nom. Dans ce cas
    /// `entries()` n'en expose qu'une par nom : l'appelant doit le signaler.
    fn has_duplicate_names(&self) -> bool;

    /// Flux du contenu décompressé d'une entrée. Le CRC est vérifié à la fin de la
    /// lecture : une erreur se convertit avec [`ArchiveError::from_read_error`].
    fn open_entry(&mut self, index: usize) -> Result<Box<dyn Read + '_>, ArchiveError>;

    /// Contenu complet d'une entrée, CRC vérifié.
    fn read_entry(&mut self, index: usize) -> Result<Vec<u8>, ArchiveError> {
        let mut out = Vec::new();
        self.open_entry(index)?
            .read_to_end(&mut out)
            .map_err(ArchiveError::from_read_error)?;
        Ok(out)
    }
}

/// Archive CBZ (zip).
pub struct CbzArchive {
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
        Ok(Self { zip, entries })
    }
}

impl Archive for CbzArchive {
    fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn has_duplicate_names(&self) -> bool {
        self.zip.has_duplicate_names()
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
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use zip::write::SimpleFileOptions;

    fn write_zip(dir: &Path, name: &str, files: &[(&str, &[u8], CompressionMethod)]) -> PathBuf {
        let path = dir.join(name);
        let mut zip = zip::ZipWriter::new(File::create(&path).unwrap());
        for (entry, content, method) in files {
            if entry.ends_with('/') {
                zip.add_directory(*entry, SimpleFileOptions::default())
                    .unwrap();
            } else {
                zip.start_file(
                    *entry,
                    SimpleFileOptions::default().compression_method(*method),
                )
                .unwrap();
                zip.write_all(content).unwrap();
            }
        }
        zip.finish().unwrap();
        path
    }

    fn replace_all(path: &Path, from: &[u8], to: &[u8]) {
        assert_eq!(from.len(), to.len());
        let mut bytes = std::fs::read(path).unwrap();
        let mut found = 0;
        for i in 0..=bytes.len() - from.len() {
            if &bytes[i..i + from.len()] == from {
                bytes[i..i + from.len()].copy_from_slice(to);
                found += 1;
            }
        }
        assert!(found > 0, "motif introuvable");
        std::fs::write(path, bytes).unwrap();
    }

    const STORED: CompressionMethod = CompressionMethod::Stored;
    const DEFLATED: CompressionMethod = CompressionMethod::Deflated;

    #[test]
    fn lists_entries_in_directory_order() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(
            dir.path(),
            "a.cbz",
            &[
                ("10.jpg", b"dix", DEFLATED),
                ("ch1/", b"", STORED),
                ("2.jpg", b"deux", STORED),
            ],
        );
        let archive = CbzArchive::open(&path).unwrap();
        let e = archive.entries();
        assert_eq!(e.len(), 3);
        assert_eq!(
            e.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["10.jpg", "ch1/", "2.jpg"]
        );
        assert_eq!(e[0].index, 0);
        assert_eq!(e[0].name_raw, b"10.jpg");
        assert_eq!(e[0].size, 3);
        assert_eq!(e[0].crc32, crc32(b"dix"));
        assert_eq!(e[0].compression, "Deflated");
        assert!(e[0].compression_supported && !e[0].encrypted);
        assert_eq!(e[1].kind, EntryKind::Dir);
        assert_eq!(e[2].kind, EntryKind::File);
        assert!(!archive.has_duplicate_names());
    }

    fn crc32(data: &[u8]) -> u32 {
        // CRC-32/ISO-HDLC, calculé à part pour ne pas dépendre de `zip`.
        let mut crc = !0u32;
        for &b in data {
            crc ^= u32::from(b);
            for _ in 0..8 {
                crc = (crc >> 1) ^ (0xEDB8_8320 & (crc & 1).wrapping_neg());
            }
        }
        !crc
    }

    #[test]
    fn reads_entries_by_index() {
        let dir = tempfile::tempdir().unwrap();
        let big = vec![7u8; 100_000];
        let path = write_zip(
            dir.path(),
            "a.cbz",
            &[("a.jpg", &big, DEFLATED), ("b.jpg", b"bee", STORED)],
        );
        let mut archive = CbzArchive::open(&path).unwrap();
        assert_eq!(archive.read_entry(1).unwrap(), b"bee");
        assert_eq!(archive.read_entry(0).unwrap(), big);
        assert!(matches!(
            archive.read_entry(2),
            Err(ArchiveError::NoSuchEntry(2))
        ));
    }

    #[test]
    fn detects_crc_mismatch() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(
            dir.path(),
            "a.cbz",
            &[("a.jpg", b"contenu-original", STORED)],
        );
        replace_all(&path, b"contenu-original", b"contenu-altere!!");
        let mut archive = CbzArchive::open(&path).unwrap();
        assert!(matches!(
            archive.read_entry(0),
            Err(ArchiveError::CrcMismatch)
        ));
    }

    #[test]
    fn rejects_non_zip_and_truncated_files() {
        let dir = tempfile::tempdir().unwrap();
        let junk = dir.path().join("junk.cbz");
        std::fs::write(&junk, b"ceci n'est pas un zip").unwrap();
        assert!(matches!(
            CbzArchive::open(&junk),
            Err(ArchiveError::Unreadable(_))
        ));

        let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"aaaa", STORED)]);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::write(&path, &bytes[..bytes.len() - 10]).unwrap();
        assert!(matches!(
            CbzArchive::open(&path),
            Err(ArchiveError::Unreadable(_) | ArchiveError::Truncated(_))
        ));
    }

    #[test]
    fn duplicate_names_are_flagged() {
        // `zip` fusionne les entrées de même nom : on vérifie qu'il le signale,
        // l'analyse fine des doublons relève des contrôles d'intégrité.
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(
            dir.path(),
            "a.cbz",
            &[("a.jpg", b"un", STORED), ("b.jpg", b"deux", STORED)],
        );
        replace_all(&path, b"b.jpg", b"a.jpg");
        let archive = CbzArchive::open(&path).unwrap();
        assert!(archive.has_duplicate_names());
        assert_eq!(archive.entries().len(), 1);
    }

    #[test]
    fn opening_does_not_modify_the_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = write_zip(dir.path(), "a.cbz", &[("a.jpg", b"aaaa", DEFLATED)]);
        let before = std::fs::read(&path).unwrap();
        let mtime = std::fs::metadata(&path).unwrap().modified().unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();

        let mut archive = CbzArchive::open(&path).unwrap();
        archive.read_entry(0).unwrap();
        drop(archive);

        assert_eq!(std::fs::read(&path).unwrap(), before);
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);

        // Windows refuse de supprimer un fichier en lecture seule : on rétablit les droits
        // pour que le dossier temporaire soit nettoyé.
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        perms.set_readonly(false);
        std::fs::set_permissions(&path, perms).unwrap();
    }
}
