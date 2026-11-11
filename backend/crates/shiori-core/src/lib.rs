//! Cœur de Shiori : archives, identités stables et ordre des pages.
//!
//! Aucune dépendance à la base ni au runtime asynchrone : ce crate est partagé par
//! la CLI de scan, le futur job de scan de l'application et le serveur.

pub mod archive;
pub mod id;
pub mod integrity;
pub mod order;

pub use archive::{Archive, ArchiveError, CbzArchive, Duplicate, Entry, EntryKind, MAX_ENTRY_SIZE};
pub use id::{PageId, ParseIdError, VolumeId};
pub use integrity::{
    FileReport, Issue, IssueCode, Page, PageStatus, ScanOptions, Severity, Status, scan_file,
};
pub use order::compare_entry_names;
