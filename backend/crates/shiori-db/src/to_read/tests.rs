use shiori_core::{PageId, ScanOptions, VolumeId};
use shiori_testkit::fixtures;

use crate::{Db, LibraryKind, Role};

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    series: i64,
    other: i64,
    volume: VolumeId,
    page: PageId,
}

/// Deux séries d'un tome chacune.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bibliotheque");
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let library = db
        .create_library("Lib", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let mut first = None;
    for (folder, make) in [
        (
            "A",
            fixtures::manga as fn(&std::path::Path) -> std::path::PathBuf,
        ),
        ("B", fixtures::unpadded),
    ] {
        let dir = root.join(folder);
        std::fs::create_dir_all(&dir).unwrap();
        let file = make(&dir);
        let scanned = db
            .scan(run, &file, ScanOptions::default(), false)
            .await
            .unwrap();
        db.attach(&library, &file).await.unwrap();
        first.get_or_insert((
            scanned.report.volume_id.unwrap(),
            scanned.report.pages[0].id,
        ));
    }
    db.refresh_series(&library).await.unwrap();
    let me = crate::tests::viewer(&db).await;
    let list = db.series_of(&me, library.id).await.unwrap().unwrap();
    let id = |name: &str| list.iter().find(|s| s.name == name).unwrap().id;
    let (volume, page) = first.unwrap();
    Fixture {
        series: id("A"),
        other: id("B"),
        db,
        volume,
        page,
        _dir: dir,
    }
}

async fn to_read(f: &Fixture, viewer: &crate::Viewer) -> Vec<(i64, bool)> {
    let mut list: Vec<_> =
        f.db.all_series(viewer)
            .await
            .unwrap()
            .into_iter()
            .map(|s| (s.id, s.to_read_at.is_some()))
            .collect();
    list.sort();
    list
}

#[tokio::test]
async fn a_series_waits_in_to_read_until_it_is_started() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    assert!(f.db.add_to_read(&me, f.series).await.unwrap());
    assert!(f.db.add_to_read(&me, f.other).await.unwrap());
    let first = f.db.all_series(&me).await.unwrap();
    let at = first
        .iter()
        .find(|s| s.id == f.series)
        .unwrap()
        .to_read_at
        .clone();
    assert!(
        f.db.add_to_read(&me, f.series).await.unwrap(),
        "deux fois : sans effet"
    );
    let again = f.db.all_series(&me).await.unwrap();
    assert_eq!(
        again.iter().find(|s| s.id == f.series).unwrap().to_read_at,
        at
    );
    assert!(
        f.db.series_detail(&me, f.series)
            .await
            .unwrap()
            .unwrap()
            .to_read
    );

    // Une page lue : la série sort de « À lire », l'autre reste.
    f.db.save_progress(&me, &f.volume, &f.page).await.unwrap();
    assert_eq!(to_read(&f, &me).await, [(f.series, false), (f.other, true)]);
    assert!(
        !f.db
            .series_detail(&me, f.series)
            .await
            .unwrap()
            .unwrap()
            .to_read
    );

    // Marquée lue : elle sort aussi ; retirée à la main : de même.
    f.db.mark_series(&me, f.other, true).await.unwrap();
    assert_eq!(
        to_read(&f, &me).await,
        [(f.series, false), (f.other, false)]
    );
    f.db.add_to_read(&me, f.series).await.unwrap();
    assert!(f.db.remove_to_read(&me, f.series).await.unwrap());
    assert_eq!(
        to_read(&f, &me).await,
        [(f.series, false), (f.other, false)]
    );
}

#[tokio::test]
async fn to_read_is_personal_and_needs_the_series_visible() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    let bob =
        f.db.create_user("bob", "mot de passe", Role::Reader)
            .await
            .unwrap();
    let none = crate::UserChange {
        libraries: Some(vec![]),
        ..Default::default()
    };
    f.db.update_user(bob.id, none).await.unwrap();
    let bob = f.db.viewer(bob.id).await.unwrap();
    assert!(!f.db.add_to_read(&bob, f.series).await.unwrap());
    assert!(!f.db.remove_to_read(&bob, f.series).await.unwrap());
    assert!(!f.db.add_to_read(&me, 999).await.unwrap());

    f.db.add_to_read(&me, f.series).await.unwrap();
    // La lecture d'un autre compte ne retire rien de la liste de celui-ci.
    let alice =
        f.db.create_user("alice", "mot de passe", Role::Admin)
            .await
            .unwrap();
    let alice = f.db.viewer(alice.id).await.unwrap();
    f.db.save_progress(&alice, &f.volume, &f.page)
        .await
        .unwrap();
    assert_eq!(to_read(&f, &me).await, [(f.series, true), (f.other, false)]);
    assert_eq!(
        to_read(&f, &alice).await,
        [(f.series, false), (f.other, false)]
    );
}
