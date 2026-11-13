//! Cœur de Shiori : archives, identités stables et ordre des pages.
//!
//! Aucune dépendance à la base ni au runtime asynchrone : ce crate est partagé par
//! la CLI de scan, le futur job de scan de l'application et le serveur.

pub mod archive;
pub mod id;
pub mod integrity;
pub mod metadata;
pub mod name;
pub mod order;

pub use archive::{
    Archive, ArchiveError, CbzArchive, Duplicate, Entry, EntryKind, MAX_ENTRY_SIZE, RarArchive,
    is_rar, open as open_archive,
};
pub use id::{PageId, ParseIdError, VolumeId};
pub use integrity::{
    FileReport, Issue, IssueCode, Page, PageStatus, SCANNER_VERSION, ScanOptions, Severity, Status,
    scan_file,
};
pub use metadata::{Author, LocalMetadata, MAX_METADATA_SIZE, MetadataSource};
pub use name::{NameKind, PARSER_VERSION, ParsedName, compare_in_series, parse as parse_name};
pub use order::compare_entry_names;
