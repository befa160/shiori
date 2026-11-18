use std::path::PathBuf;

use shiori_core::ScanOptions;
use shiori_testkit::fixtures;

use super::*;
use crate::{Library, LibraryKind};

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    library: Library,
    run: i64,
}

/// Un tome sain, un en avertissement, un en erreur, et un fichier illisible (sans tome),
/// construit à la main : un fichier qu'on ne peut pas lire du tout ne se fabrique pas
/// pareil sous Linux et Windows.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let root = dir.path().join("manga");
    std::fs::create_dir_all(root.join("A")).unwrap();
    let files = [
        fixtures::manga(&root.join("A")),
        fixtures::messy(&root.join("A")),
        fixtures::bad_crc(&root.join("A")),
    ];
    let library = db
        .create_library("Manga", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let run = db.start_run(&library.root, "{}").await.unwrap();
    for file in &files {
        db.scan(run, file, ScanOptions::default(), false)
            .await
            .unwrap();
        db.attach(&library, file).await.unwrap();
    }
    db.refresh_series(&library).await.unwrap();
    for path in [
        format!("{}/A/zz illisible.cbz", library.root),
        "/ailleurs/x.cbz".to_owned(),
    ] {
        sqlx::query(
            "INSERT INTO scan_issues (scan_run_id, volume_id, path, severity, code, message)
             VALUES (?, NULL, ?, 'error', 'archive_unreadable', 'fichier illisible')",
        )
        .bind(run)
        .bind(path)
        .execute(db.pool())
        .await
        .unwrap();
    }
    Fixture {
        _dir: dir,
        db,
        library,
        run,
    }
}

fn names(files: &[FileHealth]) -> Vec<String> {
    files
        .iter()
        .map(|f| {
            PathBuf::from(&f.path)
                .file_name()
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect()
}

#[tokio::test]
async fn health_counts_volumes_by_state_and_unreadable_files() {
    let f = fixture().await;
    let health = f.db.health().await.unwrap();
    let [library] = health.libraries.as_slice() else {
        panic!("{health:?}")
    };
    assert_eq!(
        (
            library.ok,
            library.warning,
            library.error,
            library.unreadable
        ),
        (1, 1, 1, 1),
        "{library:?}"
    );
    assert_eq!(library.deep_checked_at, None);
    // Erreurs d'abord ; le fichier hors de toute bibliothèque compte aussi (son code existe).
    assert_eq!(health.codes[0].severity, Severity::Error);
    assert!(health.codes.iter().all(|c| c.severity != Severity::Info));
    let unreadable = health
        .codes
        .iter()
        .find(|c| c.code == IssueCode::ArchiveUnreadable)
        .unwrap();
    assert_eq!(unreadable.files, 2);

    f.db.set_deep_checked(f.library.id).await.unwrap();
    let health = f.db.health().await.unwrap();
    assert!(health.libraries[0].deep_checked_at.is_some());
}

#[tokio::test]
async fn files_to_repair_are_sorted_filtered_and_paged() {
    let f = fixture().await;
    let all = HealthFilter {
        limit: 50,
        ..Default::default()
    };
    let files = f.db.files_to_repair(&all).await.unwrap();
    assert_eq!(
        names(&files),
        ["bad-crc.cbz", "messy.cbz", "zz illisible.cbz"]
    );
    let [bad, messy, unreadable] = files.as_slice() else {
        unreachable!()
    };
    assert_eq!(
        (bad.status, messy.status),
        (Severity::Error, Severity::Warning)
    );
    assert!(bad.volume_id.is_some() && bad.series_id.is_some());
    assert_eq!(bad.issues[0].severity, Severity::Error, "erreurs en tête");
    assert_eq!(
        (unreadable.volume_id.as_deref(), unreadable.status),
        (None, Severity::Error)
    );
    assert_eq!(unreadable.issues[0].code, IssueCode::ArchiveUnreadable);

    let page = HealthFilter {
        after: Some(bad.path.clone()),
        limit: 1,
        ..Default::default()
    };
    assert_eq!(
        names(&f.db.files_to_repair(&page).await.unwrap()),
        ["messy.cbz"]
    );
    let by_code = HealthFilter {
        code: Some(IssueCode::ArchiveUnreadable),
        ..all
    };
    assert_eq!(
        names(&f.db.files_to_repair(&by_code).await.unwrap()),
        ["zz illisible.cbz"]
    );
    let other = HealthFilter {
        library_id: Some(f.library.id + 1),
        limit: 50,
        ..Default::default()
    };
    assert!(f.db.files_to_repair(&other).await.unwrap().is_empty());
}

#[tokio::test]
async fn an_unreadable_file_that_disappears_is_forgotten() {
    let f = fixture().await;
    // Passe suivante du dossier : le fichier illisible n'a pas été revu.
    let next = f.db.start_run(&f.library.root, "{}").await.unwrap();
    assert!(next > f.run);
    let root = PathBuf::from(&f.library.root);
    f.db.remove_missing(&f.library, &root, next).await.unwrap();
    assert_eq!(f.db.health().await.unwrap().libraries[0].unreadable, 0);
}
