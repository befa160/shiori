use shiori_core::{PageId, ScanOptions, VolumeId};
use shiori_testkit::fixtures;

use super::*;
use crate::{LibraryKind, Role};

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    series: i64,
    volume: VolumeId,
    pages: Vec<PageId>,
    /// Tome hors de toute bibliothèque : invisible d'un compte lecteur.
    outside: VolumeId,
    outside_page: PageId,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bibliotheque");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let manga = fixtures::manga(&folder);
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let library = db
        .create_library("Lib", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let scanned = db
        .scan(run, &manga, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(&library, &manga).await.unwrap();
    db.refresh_series(&library).await.unwrap();
    let other = fixtures::unpadded(dir.path());
    let other = db
        .scan(run, &other, ScanOptions::default(), false)
        .await
        .unwrap();
    let series = db
        .series_of(&crate::tests::viewer(&db).await, library.id)
        .await
        .unwrap()
        .unwrap()[0]
        .id;
    Fixture {
        db,
        series,
        volume: scanned.report.volume_id.unwrap(),
        pages: scanned.report.pages.iter().map(|p| p.id).collect(),
        outside: other.report.volume_id.unwrap(),
        outside_page: other.report.pages[0].id,
        _dir: dir,
    }
}

fn added(result: AddBookmark) -> Bookmark {
    match result {
        AddBookmark::Added(b) => b,
        other => panic!("marque-page refusé : {other:?}"),
    }
}

#[test]
fn a_note_is_trimmed_and_bounded() {
    assert_eq!(clean_note("  à relire \n").as_deref(), Some("à relire"));
    assert_eq!(
        clean_note(&"é".repeat(MAX_NOTE)).map(|n| n.len()),
        Some(2 * MAX_NOTE)
    );
    assert_eq!(clean_note(&"é".repeat(MAX_NOTE + 1)), None);
}

#[tokio::test]
async fn bookmarks_are_added_listed_noted_and_removed() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    assert_eq!(
        f.db.bookmarks_of(&me, &f.volume).await.unwrap(),
        Some(vec![])
    );

    let late = added(
        f.db.add_bookmark(&me, &f.volume, &f.pages[7], "combat")
            .await
            .unwrap(),
    );
    assert_eq!(
        (
            late.volume_id.as_str(),
            late.page_id.as_str(),
            late.ordinal,
            late.note.as_str()
        ),
        (
            &*f.volume.to_string(),
            &*f.pages[7].to_string(),
            7,
            "combat"
        )
    );
    let early = added(
        f.db.add_bookmark(&me, &f.volume, &f.pages[2], "")
            .await
            .unwrap(),
    );
    // Une page déjà marquée garde son marque-page et sa note.
    assert_eq!(
        f.db.add_bookmark(&me, &f.volume, &f.pages[7], "autre")
            .await
            .unwrap(),
        AddBookmark::Existing(late.clone())
    );
    let listed = f.db.bookmarks_of(&me, &f.volume).await.unwrap().unwrap();
    assert_eq!(listed, [early.clone(), late.clone()]);
    assert_eq!(
        f.db.series_bookmarks(&me, f.series).await.unwrap(),
        [early.clone(), late.clone()]
    );

    let noted =
        f.db.set_bookmark_note(&me, early.id, "début")
            .await
            .unwrap()
            .unwrap();
    assert_eq!((noted.id, noted.note.as_str()), (early.id, "début"));
    assert!(f.db.delete_bookmark(&me, late.id).await.unwrap());
    assert!(!f.db.delete_bookmark(&me, late.id).await.unwrap());
    assert_eq!(
        f.db.set_bookmark_note(&me, late.id, "x").await.unwrap(),
        None
    );
    assert_eq!(
        f.db.bookmarks_of(&me, &f.volume).await.unwrap().unwrap(),
        [noted]
    );
}

#[tokio::test]
async fn bookmarks_are_personal_and_checked() {
    let f = fixture().await;
    let me = crate::tests::viewer(&f.db).await;
    let bob =
        f.db.create_user("bob", "mot de passe", Role::Reader)
            .await
            .unwrap();
    let bob = f.db.viewer(bob.id).await.unwrap();
    let mine = added(
        f.db.add_bookmark(&me, &f.volume, &f.pages[1], "")
            .await
            .unwrap(),
    );

    // Un autre compte ne voit ni ne touche les marque-pages d'autrui.
    assert_eq!(
        f.db.bookmarks_of(&bob, &f.volume).await.unwrap(),
        Some(vec![])
    );
    assert!(
        f.db.series_bookmarks(&bob, f.series)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        f.db.set_bookmark_note(&bob, mine.id, "à moi")
            .await
            .unwrap(),
        None
    );
    assert!(!f.db.delete_bookmark(&bob, mine.id).await.unwrap());
    // La même page marquée par bob : son propre marque-page.
    let his = added(
        f.db.add_bookmark(&bob, &f.volume, &f.pages[1], "")
            .await
            .unwrap(),
    );
    assert_ne!(his.id, mine.id);

    // Tome invisible du compte, page d'un autre tome.
    assert_eq!(f.db.bookmarks_of(&bob, &f.outside).await.unwrap(), None);
    assert_eq!(
        f.db.add_bookmark(&bob, &f.outside, &f.outside_page, "")
            .await
            .unwrap(),
        AddBookmark::UnknownVolume
    );
    assert_eq!(
        f.db.add_bookmark(&me, &f.volume, &f.outside_page, "")
            .await
            .unwrap(),
        AddBookmark::PageNotInVolume
    );

    // Le compte supprimé emporte ses marque-pages.
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(bob.user)
        .execute(&f.db.pool)
        .await
        .unwrap();
    let left: i64 = sqlx::query_scalar("SELECT count(*) FROM bookmarks")
        .fetch_one(&f.db.pool)
        .await
        .unwrap();
    assert_eq!(left, 1);
}
