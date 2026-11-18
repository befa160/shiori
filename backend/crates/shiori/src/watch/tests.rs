use notify::event::{
    AccessKind, AccessMode, CreateKind, MetadataKind, ModifyKind, RemoveKind, RenameMode,
};

use super::*;

#[test]
fn reading_a_file_is_not_a_change() {
    assert!(!relevant(&EventKind::Access(AccessKind::Open(
        AccessMode::Read
    ))));
    assert!(!relevant(&EventKind::Access(AccessKind::Close(
        AccessMode::Read
    ))));
    assert!(!relevant(&EventKind::Modify(ModifyKind::Metadata(
        MetadataKind::AccessTime
    ))));
    assert!(relevant(&EventKind::Access(AccessKind::Close(
        AccessMode::Write
    ))));
    assert!(relevant(&EventKind::Create(CreateKind::File)));
    assert!(relevant(&EventKind::Modify(ModifyKind::Name(
        RenameMode::Both
    ))));
    assert!(relevant(&EventKind::Remove(RemoveKind::Folder)));
}

#[test]
fn a_change_targets_its_series_folder_or_the_root() {
    let roots = [(1, PathBuf::from("/m")), (2, PathBuf::from("/bd"))];
    let series = Some((1, PathBuf::from("/m/Série")));
    assert_eq!(target(&roots, Path::new("/m/Série/t01.cbz")), series);
    assert_eq!(target(&roots, Path::new("/m/Série/Bonus/t.cbz")), series);
    assert_eq!(
        target(&roots, Path::new("/m/t01.cbz")),
        Some((1, PathBuf::from("/m")))
    );
    assert_eq!(
        target(&roots, Path::new("/m")),
        Some((1, PathBuf::from("/m")))
    );
    assert_eq!(
        target(&roots, Path::new("/bd/X/a.cbz")),
        Some((2, PathBuf::from("/bd/X")))
    );
    assert_eq!(target(&roots, Path::new("/manga/a.cbz")), None);
}
