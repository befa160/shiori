use shiori_core::ScanOptions;
use shiori_testkit::fixtures;

use super::*;
use crate::{LibraryKind, Role, UserChange};

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    /// Séries « A » (bibliothèque « Un ») et « B » (bibliothèque « Deux »).
    a: i64,
    b: i64,
    two: i64,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let me = crate::tests::viewer(&db).await;
    let mut ids = Vec::new();
    for (library, folder, make) in [
        (
            "Un",
            "A",
            fixtures::manga as fn(&std::path::Path) -> std::path::PathBuf,
        ),
        ("Deux", "B", fixtures::unpadded),
    ] {
        let root = dir.path().join(library);
        std::fs::create_dir_all(root.join(folder)).unwrap();
        let library = db
            .create_library(library, &root, LibraryKind::Manga)
            .await
            .unwrap();
        let file = make(&root.join(folder));
        db.scan(run, &file, ScanOptions::default(), false)
            .await
            .unwrap();
        db.attach(&library, &file).await.unwrap();
        db.refresh_series(&library).await.unwrap();
        let series = db.series_of(&me, library.id).await.unwrap().unwrap()[0].id;
        ids.push((library.id, series));
    }
    Fixture {
        db,
        a: ids[0].1,
        b: ids[1].1,
        two: ids[1].0,
        _dir: dir,
    }
}

#[test]
fn a_name_is_trimmed_and_bounded() {
    assert_eq!(clean_name("  Shōnen ").unwrap(), "Shōnen");
    assert!(clean_name(&"é".repeat(MAX_COLLECTION_NAME)).is_ok());
    assert!(matches!(clean_name(" "), Err(CollectionError::InvalidName)));
    assert!(matches!(
        clean_name(&"é".repeat(MAX_COLLECTION_NAME + 1)),
        Err(CollectionError::InvalidName)
    ));
}

#[tokio::test]
async fn collections_are_created_filled_renamed_and_deleted() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    let id = f.db.create_collection(" Shōnen ").await.unwrap();
    let other = f.db.create_collection("Aventure").await.unwrap();
    assert!(matches!(
        f.db.create_collection("aventure").await,
        Err(CollectionError::NameTaken(n)) if n == "aventure"
    ));
    assert!(matches!(
        f.db.create_collection("").await,
        Err(CollectionError::InvalidName)
    ));

    assert!(f.db.add_to_collection(id, f.b).await.unwrap());
    assert!(f.db.add_to_collection(id, f.a).await.unwrap());
    assert!(
        f.db.add_to_collection(id, f.a).await.unwrap(),
        "déjà dedans : sans effet"
    );
    assert!(!f.db.add_to_collection(id, 999).await.unwrap());
    assert!(!f.db.add_to_collection(999, f.a).await.unwrap());

    // Par nom ; l'administrateur voit aussi la collection vide.
    let list = f.db.collections(&me).await.unwrap();
    let names: Vec<_> = list.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["Aventure", "Shōnen"]);
    let shonen = list.iter().find(|c| c.id == id).unwrap();
    let mut series = shonen.series_ids.clone();
    series.sort();
    let mut want = vec![f.a, f.b];
    want.sort();
    assert_eq!(series, want);
    let detail = f.db.series_detail(&me, f.a).await.unwrap().unwrap();
    assert_eq!(
        detail.collections,
        [CollectionRef {
            id,
            name: "Shōnen".into()
        }]
    );

    assert!(f.db.rename_collection(id, "Nekketsu").await.unwrap());
    assert!(matches!(
        f.db.rename_collection(id, "AVENTURE").await,
        Err(CollectionError::NameTaken(_))
    ));
    assert!(!f.db.rename_collection(999, "X").await.unwrap());
    assert!(f.db.remove_from_collection(id, f.a).await.unwrap());
    assert!(!f.db.remove_from_collection(999, f.a).await.unwrap());
    assert!(
        f.db.series_detail(&me, f.a)
            .await
            .unwrap()
            .unwrap()
            .collections
            .is_empty()
    );

    assert!(f.db.delete_collection(other).await.unwrap());
    assert!(!f.db.delete_collection(other).await.unwrap());
    let left = f.db.collections(&me).await.unwrap();
    assert!(left.iter().all(|c| c.id != other));
    // La série reste, elle n'est que retirée.
    assert!(f.db.series_detail(&me, f.b).await.unwrap().is_some());
}

#[tokio::test]
async fn a_reader_sees_only_series_of_its_libraries() {
    let f = fixture().await;
    let id = f.db.create_collection("Shōnen").await.unwrap();
    let empty = f.db.create_collection("Vide").await.unwrap();
    f.db.add_to_collection(id, f.a).await.unwrap();
    f.db.add_to_collection(id, f.b).await.unwrap();
    let bob =
        f.db.create_user("bob", "mot de passe", Role::Reader)
            .await
            .unwrap();
    let only_two = UserChange {
        libraries: Some(vec![f.two]),
        ..Default::default()
    };
    f.db.update_user(bob.id, only_two).await.unwrap();
    let bob = f.db.viewer(bob.id).await.unwrap();
    let list = f.db.collections(&bob).await.unwrap();
    assert_eq!(
        list,
        [Collection {
            id,
            name: "Shōnen".into(),
            series_ids: vec![f.b]
        }],
        "série d'une autre bibliothèque et collection vide masquées"
    );
    assert!(list.iter().all(|c| c.id != empty));

    // Plus aucune série visible : la collection disparaît pour lui.
    f.db.remove_from_collection(id, f.b).await.unwrap();
    assert!(f.db.collections(&bob).await.unwrap().is_empty());
}
