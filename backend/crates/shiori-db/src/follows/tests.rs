use std::path::Path;
use std::time::Duration;

use shiori_core::{PageId, ScanOptions, VolumeId};
use shiori_testkit::fixtures;

use crate::{Db, Library, LibraryKind, Role};

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    library: Library,
    folder: std::path::PathBuf,
    series: i64,
    /// Tome déjà là avant le suivi : jamais nouveau.
    old: VolumeId,
}

/// Scanne et range un fichier ; rend son tome et sa première page.
async fn add(db: &Db, library: &Library, file: &Path) -> (VolumeId, PageId) {
    let run = db.start_run("/", "{}").await.unwrap();
    let scanned = db
        .scan(run, file, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(library, file).await.unwrap();
    db.refresh_series(library).await.unwrap();
    (
        scanned.report.volume_id.unwrap(),
        scanned.report.pages[0].id,
    )
}

/// Les dates sont à la milliseconde : un tome ajouté juste après un « vu » doit être plus récent.
async fn tick() {
    tokio::time::sleep(Duration::from_millis(3)).await;
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bibliotheque");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let library = db
        .create_library("Lib", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let (old, _) = add(&db, &library, &fixtures::manga(&folder)).await;
    let me = crate::tests::viewer(&db).await;
    let series = db.series_of(&me, library.id).await.unwrap().unwrap()[0].id;
    Fixture {
        db,
        library,
        folder,
        series,
        old,
        _dir: dir,
    }
}

fn ids(list: &[crate::NewVolume]) -> Vec<String> {
    list.iter().map(|n| n.volume.volume_id.clone()).collect()
}

#[tokio::test]
async fn volumes_added_to_a_followed_series_are_new_until_opened_or_seen() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    assert!(
        !f.db
            .series_detail(&me, f.series)
            .await
            .unwrap()
            .unwrap()
            .followed
    );
    assert!(f.db.follow(&me, f.series).await.unwrap());
    assert!(
        f.db.follow(&me, f.series).await.unwrap(),
        "suivre deux fois ne change rien"
    );
    assert!(
        f.db.series_detail(&me, f.series)
            .await
            .unwrap()
            .unwrap()
            .followed
    );
    assert!(
        f.db.new_volumes(&me).await.unwrap().is_empty(),
        "le tome d'avant n'est pas nouveau"
    );

    tick().await;
    let one = add(&f.db, &f.library, &fixtures::unpadded(&f.folder)).await;
    let two = add(&f.db, &f.library, &fixtures::webtoon(&f.folder)).await;
    let new = f.db.new_volumes(&me).await.unwrap();
    let mut got = ids(&new);
    got.sort();
    let mut want = vec![one.0.to_string(), two.0.to_string()];
    want.sort();
    assert_eq!(got, want);
    assert_eq!(new[0].series_id, f.series);
    assert_eq!(new[0].series_name, "Série");
    assert_eq!(new[0].library_id, f.library.id);
    assert!(!ids(&new).contains(&f.old.to_string()));

    // Ouvert : il n'est plus nouveau.
    f.db.save_progress(&me, &one.0, &one.1).await.unwrap();
    assert_eq!(
        ids(&f.db.new_volumes(&me).await.unwrap()),
        [two.0.to_string()]
    );

    // Progression effacée (tome, puis série « non lue ») : ouvert une fois, il ne redevient pas
    // nouveau ; l'autre, jamais ouvert, le reste.
    f.db.clear_progress(&me, &one.0).await.unwrap();
    assert_eq!(
        ids(&f.db.new_volumes(&me).await.unwrap()),
        [two.0.to_string()]
    );
    f.db.save_progress(&me, &one.0, &one.1).await.unwrap();
    f.db.mark_series(&me, f.series, false).await.unwrap();
    assert_eq!(
        ids(&f.db.new_volumes(&me).await.unwrap()),
        [two.0.to_string()]
    );

    // « Tout marquer comme vu » ; un tome ajouté ensuite redevient nouveau.
    f.db.mark_new_seen(&me).await.unwrap();
    assert!(f.db.new_volumes(&me).await.unwrap().is_empty());
    tick().await;
    let three = add(&f.db, &f.library, &fixtures::messy(&f.folder)).await;
    assert_eq!(
        ids(&f.db.new_volumes(&me).await.unwrap()),
        [three.0.to_string()]
    );

    // Plus suivie : plus de nouveautés.
    assert!(f.db.unfollow(&me, f.series).await.unwrap());
    assert!(
        !f.db
            .series_detail(&me, f.series)
            .await
            .unwrap()
            .unwrap()
            .followed
    );
    assert!(f.db.new_volumes(&me).await.unwrap().is_empty());
}

#[tokio::test]
async fn a_moved_file_is_not_new() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    f.db.follow(&me, f.series).await.unwrap();
    tick().await;
    // Même fichier sous un autre nom : même tome, déjà daté d'avant le suivi.
    let original = f.folder.read_dir().unwrap().next().unwrap().unwrap().path();
    let copy = f.folder.join("copie.cbz");
    std::fs::rename(&original, &copy).unwrap();
    assert_eq!(add(&f.db, &f.library, &copy).await.0, f.old);
    let detail = f.db.series_detail(&me, f.series).await.unwrap().unwrap();
    assert_eq!(
        detail.volumes.len(),
        1,
        "l'ancien emplacement est retiré par le scan"
    );
    assert!(f.db.new_volumes(&me).await.unwrap().is_empty());
}

#[tokio::test]
async fn follows_are_personal_and_need_the_series_visible() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    let bob =
        f.db.create_user("bob", "mot de passe", Role::Reader)
            .await
            .unwrap();
    // Le lecteur reçoit toutes les bibliothèques à sa création : on les lui retire.
    let none = crate::UserChange {
        libraries: Some(vec![]),
        ..Default::default()
    };
    f.db.update_user(bob.id, none).await.unwrap();
    let bob = f.db.viewer(bob.id).await.unwrap();
    // Bibliothèque fermée à bob : il ne peut ni suivre ni cesser de suivre.
    assert!(!f.db.follow(&bob, f.series).await.unwrap());
    assert!(!f.db.unfollow(&bob, f.series).await.unwrap());
    assert!(!f.db.follow(&me, 999).await.unwrap());

    f.db.follow(&me, f.series).await.unwrap();
    tick().await;
    add(&f.db, &f.library, &fixtures::unpadded(&f.folder)).await;
    assert_eq!(f.db.new_volumes(&me).await.unwrap().len(), 1);
    // « Vu » par bob ne touche pas aux nouveautés d'un autre compte.
    f.db.mark_new_seen(&bob).await.unwrap();
    assert_eq!(f.db.new_volumes(&me).await.unwrap().len(), 1);
    assert!(f.db.new_volumes(&bob).await.unwrap().is_empty());
}

#[tokio::test]
async fn the_series_completed_last_comes_first() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    let other = f.folder.with_file_name("Autre");
    std::fs::create_dir_all(&other).unwrap();
    add(&f.db, &f.library, &fixtures::manga(&other)).await;
    let series = f.db.series_of(&me, f.library.id).await.unwrap().unwrap();
    let autre = series.iter().find(|s| s.name == "Autre").unwrap().id;
    f.db.follow(&me, f.series).await.unwrap();
    f.db.follow(&me, autre).await.unwrap();
    tick().await;
    add(&f.db, &f.library, &fixtures::unpadded(&f.folder)).await;
    tick().await;
    // Un autre fichier : le même contenu serait le même tome, déjà daté.
    add(&f.db, &f.library, &fixtures::webtoon(&other)).await;
    let order: Vec<i64> =
        f.db.new_volumes(&me)
            .await
            .unwrap()
            .iter()
            .map(|n| n.series_id)
            .collect();
    assert_eq!(order, [autre, f.series]);
}
