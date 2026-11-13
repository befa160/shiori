//! Scan d'intégrité d'un fichier (principes 3 et 4).
//!
//! Pour une archive, on établit une fois pour toutes la liste ordonnée de ses pages
//! (identité, dimensions, type) et tous les problèmes rencontrés, chacun avec un code
//! stable. Une page illisible reste listée à sa place avec `status = broken` : le
//! lecteur affichera une erreur explicite à cet endroit, jamais une autre image.

use std::fs;
use std::path::{Path, PathBuf};

use imagesize::{Compression, ImageType};
use serde::{Deserialize, Serialize, Serializer};

use std::collections::{BTreeMap, HashMap};

use crate::archive::{self, ArchiveError, Entry, EntryKind};
use crate::id::{PageId, VolumeId};
use crate::metadata::{self, LocalMetadata, MAX_METADATA_SIZE, MetadataSource};
use crate::order::compare_entry_names;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Warning,
    Error,
}

/// Codes de diagnostic stables : l'interface et la base s'appuient dessus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IssueCode {
    ArchiveUnreadable,
    ArchiveTruncated,
    MultivolumeArchive,
    EncryptedEntry,
    UnsupportedCompression,
    CrcMismatch,
    SizeMismatch,
    EntryTooLarge,
    ImageUndecodable,
    DuplicateEntryConflict,
    NoPages,
    FileChangedDuringScan,
    DuplicateEntryIdentical,
    ExtensionMismatch,
    UnsupportedImage,
    NonImageEntry,
    Subdirectories,
    ArchiveOrderDiffers,
    IgnoredEntry,
    SolidArchive,
    MetadataInvalid,
}

impl IssueCode {
    pub fn severity(self) -> Severity {
        use IssueCode::*;
        match self {
            ArchiveUnreadable
            | ArchiveTruncated
            | MultivolumeArchive
            | EncryptedEntry
            | UnsupportedCompression
            | CrcMismatch
            | SizeMismatch
            | EntryTooLarge
            | ImageUndecodable
            | DuplicateEntryConflict
            | NoPages
            | FileChangedDuringScan => Severity::Error,
            DuplicateEntryIdentical
            | ExtensionMismatch
            | UnsupportedImage
            | NonImageEntry
            | Subdirectories
            | ArchiveOrderDiffers
            | MetadataInvalid => Severity::Warning,
            IgnoredEntry | SolidArchive => Severity::Info,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Issue {
    pub code: IssueCode,
    pub severity: Severity,
    /// Entrée concernée, si le problème porte sur une entrée précise.
    pub entry: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    Ok,
    Broken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Page {
    pub id: PageId,
    /// Position de lecture (0..n), selon l'ordre naturel.
    pub ordinal: usize,
    pub entry_name: String,
    #[serde(skip)]
    pub entry_name_raw: Vec<u8>,
    pub archive_index: usize,
    pub media_type: Option<&'static str>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub compressed_size: u64,
    pub size: u64,
    pub crc32: u32,
    pub status: PageStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileReport {
    #[serde(serialize_with = "lossy_path")]
    pub path: PathBuf,
    /// Absent si le fichier n'a même pas pu être lu.
    pub volume_id: Option<VolumeId>,
    /// `cbz` ou `cbr`, d'après le contenu du fichier.
    pub format: &'static str,
    pub size: u64,
    pub status: Status,
    pub pages: Vec<Page>,
    pub issues: Vec<Issue>,
    /// Métadonnées des fichiers `ComicInfo.xml` et `index.json` à la racine de l'archive.
    pub metadata: Vec<(MetadataSource, LocalMetadata)>,
}

fn lossy_path<S: Serializer>(path: &Path, s: S) -> Result<S::Ok, S::Error> {
    s.collect_str(&path.display())
}

/// Version des règles de scan. À incrémenter quand un changement de ces règles
/// (ordre, classification, codes) doit déclencher le rescan des tomes déjà connus.
/// 2 : lecture de `ComicInfo.xml` et `index.json` (phase 5).
pub const SCANNER_VERSION: i64 = 2;

#[derive(Debug, Clone, Copy, Default)]
pub struct ScanOptions {
    /// Décode entièrement chaque image (jpg, png, webp ; AVIF : en-tête seulement).
    pub deep: bool,
}

impl FileReport {
    fn empty(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            volume_id: None,
            format: "cbz",
            size: 0,
            status: Status::Ok,
            pages: Vec::new(),
            issues: Vec::new(),
            metadata: Vec::new(),
        }
    }

    fn issue(&mut self, code: IssueCode, entry: Option<&str>, message: impl Into<String>) {
        self.issues.push(Issue {
            code,
            severity: code.severity(),
            entry: entry.map(str::to_owned),
            message: message.into(),
        });
    }

    fn finish(mut self) -> Self {
        self.status = match self.issues.iter().map(|i| i.severity).max() {
            Some(Severity::Error) => Status::Error,
            Some(Severity::Warning) => Status::Warning,
            _ => Status::Ok,
        };
        self
    }
}

/// Scanne un fichier CBZ ou CBR. Ne modifie jamais le fichier et ne panique pas sur une
/// archive invalide : tout problème devient un [`Issue`].
pub fn scan_file(path: &Path, options: ScanOptions) -> FileReport {
    guarded(path, || scan_unguarded(path, options))
}

/// Une panique dans une bibliothèque tierce (archive malformée) devient un problème
/// du fichier concerné au lieu d'interrompre le scan de toute la bibliothèque.
fn guarded(path: &Path, scan: impl FnOnce() -> FileReport) -> FileReport {
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(scan)).unwrap_or_else(|panic| {
        let detail = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_default();
        let mut report = FileReport::empty(path);
        report.issue(
            IssueCode::ArchiveUnreadable,
            None,
            format!("erreur interne pendant l'analyse : {detail}"),
        );
        report.finish()
    })
}

fn scan_unguarded(path: &Path, options: ScanOptions) -> FileReport {
    let mut report = FileReport::empty(path);
    // shortcut: deux lectures du fichier (hash puis entrées) ; à fusionner si le scan de
    // grosses bibliothèques devient lent (le rescan, lui, évite le hash : lot 1.4).
    let mut stamp_before = None;
    let volume = match fs::metadata(path).and_then(|m| {
        report.size = m.len();
        stamp_before = Some(stamp(&m));
        VolumeId::from_file(path)
    }) {
        Ok(v) => v,
        Err(e) => {
            report.issue(
                IssueCode::ArchiveUnreadable,
                None,
                format!("fichier illisible : {e}"),
            );
            return report.finish();
        }
    };
    report.volume_id = Some(volume);
    if archive::is_rar(path) {
        report.format = "cbr";
    }

    let mut archive = match archive::open(path) {
        Ok(a) => a,
        Err(e) => {
            let code = if matches!(e, ArchiveError::Truncated(_)) || starts_like_zip(path) {
                IssueCode::ArchiveTruncated
            } else {
                IssueCode::ArchiveUnreadable
            };
            report.issue(code, None, e.to_string());
            return report.finish();
        }
    };
    if archive.is_multivolume() {
        report.issue(
            IssueCode::MultivolumeArchive,
            None,
            "archive en plusieurs volumes (.part1.rar…), non gérée",
        );
        return report.finish();
    }
    if archive.is_solid() {
        report.issue(
            IssueCode::SolidArchive,
            None,
            "archive RAR solid : pages servies depuis un cache d'extraction",
        );
    }

    let mut conflicts = Vec::new();
    match archive.duplicates() {
        Ok(dups) => {
            for d in dups {
                if d.identical {
                    report.issue(
                        IssueCode::DuplicateEntryIdentical,
                        Some(&d.name),
                        "entrée présente plusieurs fois avec le même contenu : une seule page conservée",
                    );
                } else {
                    report.issue(
                        IssueCode::DuplicateEntryConflict,
                        Some(&d.name),
                        "entrée présente plusieurs fois avec des contenus différents : page ambiguë",
                    );
                    conflicts.push(d.name_raw);
                }
            }
        }
        Err(e) => report.issue(IssueCode::ArchiveUnreadable, None, e.to_string()),
    }

    // Un nom présent plusieurs fois : seule sa dernière occurrence fait une page (le zip
    // fusionne déjà ses doublons, le RAR les liste tous).
    let entries = archive.entries().to_vec();
    let last: HashMap<&[u8], usize> = entries
        .iter()
        .map(|e| (e.name_raw.as_slice(), e.index))
        .collect();
    let mut metadata: BTreeMap<usize, (MetadataSource, String)> = BTreeMap::new();
    let mut candidates: BTreeMap<usize, (Page, bool)> = entries
        .iter()
        .filter(|e| last[e.name_raw.as_slice()] == e.index)
        .filter_map(|e| {
            let source = MetadataSource::of_entry(&e.name).filter(|_| e.kind == EntryKind::File);
            let Some(source) = source else {
                return Some((e.index, classify(&volume, e, &mut report)?));
            };
            if metadata.values().any(|(s, _)| *s == source) {
                // `ComicInfo.xml` et `comicinfo.xml` : le premier seul compte.
                let msg = "second fichier de métadonnées du même type, ignoré";
                report.issue(IssueCode::IgnoredEntry, Some(&e.name), msg);
            } else if e.size > MAX_METADATA_SIZE {
                let msg = "fichier de métadonnées trop gros, ignoré";
                report.issue(IssueCode::MetadataInvalid, Some(&e.name), msg);
            } else {
                metadata.insert(e.index, (source, e.name.clone()));
            }
            None
        })
        .collect();
    let mut indices: Vec<usize> = candidates.keys().chain(metadata.keys()).copied().collect();
    indices.sort_unstable();
    archive.read_each(&indices, &mut |index, bytes| {
        if let Some((source, name)) = metadata.remove(&index) {
            match bytes
                .map_err(|e| e.to_string())
                .and_then(|b| metadata::parse(source, &b))
            {
                Ok(meta) => report.metadata.push((source, meta)),
                Err(msg) => report.issue(IssueCode::MetadataInvalid, Some(&name), msg),
            }
            return;
        }
        let Some((page, looks_like_image)) = candidates.remove(&index) else {
            return;
        };
        let conflict = conflicts.contains(&page.entry_name_raw);
        if let Some(mut page) = check(page, looks_like_image, bytes, options, &mut report) {
            if conflict {
                page.status = PageStatus::Broken;
            }
            report.pages.push(page);
        }
    });

    // Le fichier a changé entre le hash et la lecture des entrées : les pages ne
    // correspondent pas forcément au `volume_id`. On ne les garde pas.
    if fs::metadata(path).map(|m| stamp(&m)).ok() != stamp_before {
        report.pages.clear();
        report.metadata.clear();
        report.issue(
            IssueCode::FileChangedDuringScan,
            None,
            "fichier modifié pendant le scan : à rescanner",
        );
        return report.finish();
    }

    finalize_pages(&mut report);
    report.finish()
}

/// Taille et date de modification : ce qui change quand un fichier est réécrit.
fn stamp(meta: &fs::Metadata) -> (u64, Option<std::time::SystemTime>) {
    (meta.len(), meta.modified().ok())
}

/// Un fichier qui commence comme un zip mais dont l'annuaire est introuvable a très
/// probablement été coupé (copie interrompue, téléchargement incomplet).
fn starts_like_zip(path: &Path) -> bool {
    use std::io::Read;
    let mut magic = [0u8; 4];
    fs::File::open(path)
        .and_then(|mut f| f.read_exact(&mut magic))
        .is_ok()
        && magic == *b"PK\x03\x04"
}

fn issue_for(err: &ArchiveError) -> IssueCode {
    match err {
        ArchiveError::Truncated(_) => IssueCode::ArchiveTruncated,
        ArchiveError::Encrypted => IssueCode::EncryptedEntry,
        ArchiveError::UnsupportedCompression(_) => IssueCode::UnsupportedCompression,
        ArchiveError::CrcMismatch => IssueCode::CrcMismatch,
        ArchiveError::SizeMismatch => IssueCode::SizeMismatch,
        ArchiveError::EntryTooLarge(_) => IssueCode::EntryTooLarge,
        ArchiveError::Unreadable(_) | ArchiveError::NoSuchEntry(_) | ArchiveError::Io(_) => {
            IssueCode::ArchiveUnreadable
        }
    }
}

const IMAGE_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "avif", "gif", "bmp", "tif", "tiff", "jxl", "heic", "heif",
];

fn extension(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
}

fn media_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "jpg" | "jpeg" => Some("image/jpeg"),
        "png" => Some("image/png"),
        "webp" => Some("image/webp"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

fn is_ignored(name: &str) -> bool {
    let mut parts = name.split(['/', '\\']).filter(|p| !p.is_empty());
    let base = parts.clone().next_back().unwrap_or_default();
    parts.any(|p| p == "__MACOSX")
        || ["comicinfo.xml", "thumbs.db", ".ds_store"]
            .iter()
            .any(|n| base.eq_ignore_ascii_case(n))
}

/// Première moitié de l'examen d'une entrée, sans la lire : la page à venir et si son nom
/// ressemble à une image, ou rien si l'entrée est écartée (problème enregistré).
fn classify(volume: &VolumeId, entry: &Entry, report: &mut FileReport) -> Option<(Page, bool)> {
    let name = entry.name.as_str();
    match entry.kind {
        EntryKind::Dir => return None,
        EntryKind::Symlink => {
            report.issue(
                IssueCode::NonImageEntry,
                Some(name),
                "lien symbolique ignoré",
            );
            return None;
        }
        EntryKind::File => {}
    }
    if is_ignored(name) {
        report.issue(
            IssueCode::IgnoredEntry,
            Some(name),
            "entrée connue, ignorée",
        );
        return None;
    }

    let ext = extension(name);
    let looks_like_image = ext
        .as_deref()
        .is_some_and(|e| IMAGE_EXTENSIONS.contains(&e));
    let page = Page {
        id: PageId::derive(volume, &entry.name_raw),
        ordinal: 0,
        entry_name: entry.name.clone(),
        entry_name_raw: entry.name_raw.clone(),
        archive_index: entry.index,
        media_type: None,
        width: None,
        height: None,
        compressed_size: entry.compressed_size,
        size: entry.size,
        crc32: entry.crc32,
        status: PageStatus::Broken,
    };
    Some((page, looks_like_image))
}

/// Seconde moitié : à partir du contenu lu, une page (éventuellement `broken`) ou rien, en
/// enregistrant les problèmes dans le rapport.
fn check(
    mut page: Page,
    looks_like_image: bool,
    bytes: Result<Vec<u8>, ArchiveError>,
    options: ScanOptions,
    report: &mut FileReport,
) -> Option<Page> {
    let name = page.entry_name.clone();
    let name = name.as_str();
    let ext = extension(name);
    let broken = |report: &mut FileReport, code: IssueCode, msg: String, page: Page| {
        report.issue(code, Some(name), msg);
        looks_like_image.then_some(page)
    };

    let bytes = match bytes {
        Ok(b) => b,
        Err(e) => return broken(report, issue_for(&e), e.to_string(), page),
    };

    let media = match imagesize::image_type(&bytes) {
        Ok(ImageType::Jpeg) => "image/jpeg",
        Ok(ImageType::Png) => "image/png",
        Ok(ImageType::Webp) => "image/webp",
        Ok(ImageType::Heif(Compression::Av1)) => "image/avif",
        Ok(other) => {
            report.issue(
                IssueCode::UnsupportedImage,
                Some(name),
                format!("format d'image non pris en charge ({other:?}), entrée ignorée"),
            );
            return None;
        }
        Err(_) if looks_like_image => {
            let msg = "contenu non reconnu comme image".to_string();
            return broken(report, IssueCode::ImageUndecodable, msg, page);
        }
        Err(_) => {
            report.issue(
                IssueCode::NonImageEntry,
                Some(name),
                "fichier parasite, ignoré",
            );
            return None;
        }
    };
    page.media_type = Some(media);

    let size = match imagesize::blob_size(&bytes) {
        Ok(s) if s.width > 0 && s.height > 0 => s,
        _ => {
            let msg = "dimensions illisibles dans l'en-tête".to_string();
            return broken(report, IssueCode::ImageUndecodable, msg, page);
        }
    };
    page.width = u32::try_from(size.width).ok();
    page.height = u32::try_from(size.height).ok();

    if options.deep
        && media != "image/avif"
        && let Err(e) = image::load_from_memory(&bytes)
    {
        let msg = format!("décodage : {e}");
        return broken(report, IssueCode::ImageUndecodable, msg, page);
    }

    if let Some(expected) = ext.as_deref().and_then(media_for_extension)
        && expected != media
    {
        report.issue(
            IssueCode::ExtensionMismatch,
            Some(name),
            format!("extension trompeuse : contenu {media}"),
        );
    }
    page.status = PageStatus::Ok;
    Some(page)
}

/// Ordre de lecture, ordinaux et diagnostics qui portent sur l'ensemble des pages.
fn finalize_pages(report: &mut FileReport) {
    let pages = &mut report.pages;
    pages.sort_by(|a, b| {
        compare_entry_names(
            &a.entry_name,
            &a.entry_name_raw,
            &b.entry_name,
            &b.entry_name_raw,
        )
    });
    let mut archive_order_differs = false;
    for i in 0..pages.len() {
        pages[i].ordinal = i;
        archive_order_differs |= i > 0 && pages[i - 1].archive_index > pages[i].archive_index;
    }
    let in_subdirectories = pages.iter().any(|p| p.entry_name.contains(['/', '\\']));

    if pages.is_empty() {
        report.issue(IssueCode::NoPages, None, "aucune image dans l'archive");
    }
    if archive_order_differs {
        report.issue(
            IssueCode::ArchiveOrderDiffers,
            None,
            "l'ordre de l'archive diffère de l'ordre naturel des noms ; l'ordre naturel est appliqué",
        );
    }
    if in_subdirectories {
        report.issue(
            IssueCode::Subdirectories,
            None,
            "pages réparties dans des sous-dossiers",
        );
    }
}

#[cfg(test)]
mod tests;
