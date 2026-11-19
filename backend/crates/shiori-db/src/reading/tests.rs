use shiori_core::{PageId, ScanOptions, VolumeId};
use shiori_testkit::fixtures;

use super::*;

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    series: i64,
    volume: VolumeId,
    pages: Vec<PageId>,
    other_page: PageId,
}

/// Bibliothèque du type demandé : une série d'un tome (le faux manga de 12 pages), et un
/// second tome posé hors de toute bibliothèque.
async fn fixture(kind: LibraryKind) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bibliotheque");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let manga = fixtures::manga(&folder);
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let library = db.create_library("Lib", &root, kind).await.unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let scanned = db
        .scan(run, &manga, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(&library, &manga).await.unwrap();
    db.refresh_series(&library).await.unwrap();
    let outside = fixtures::unpadded(dir.path());
    let other = db
        .scan(run, &outside, ScanOptions::default(), false)
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
        other_page: other.report.pages[0].id,
        _dir: dir,
    }
}

fn saved(result: SaveProgress) -> Progress {
    match result {
        SaveProgress::Saved(p) => p,
        other => panic!("progression refusée : {other:?}"),
    }
}

#[tokio::test]
async fn progress_is_saved_finished_once_and_cleared() {
    let f = fixture(LibraryKind::Manga).await;
    let last = f.pages.len() - 1;
    assert_eq!(
        f.db.progress(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap(),
        None
    );

    let p = saved(
        f.db.save_progress(&crate::tests::viewer(&f.db).await, &f.volume, &f.pages[3])
            .await
            .unwrap(),
    );
    assert_eq!(
        (p.page_id.as_str(), p.ordinal, p.page_count),
        (&*f.pages[3].to_string(), 3, 12)
    );
    assert!(!p.finished);
    assert_eq!(
        f.db.progress(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap(),
        Some(p)
    );

    // La dernière page marque le tome comme lu ; revenir en arrière ne l'efface pas.
    assert!(
        saved(
            f.db.save_progress(
                &crate::tests::viewer(&f.db).await,
                &f.volume,
                &f.pages[last]
            )
            .await
            .unwrap()
        )
        .finished
    );
    let back = saved(
        f.db.save_progress(&crate::tests::viewer(&f.db).await, &f.volume, &f.pages[1])
            .await
            .unwrap(),
    );
    assert_eq!((back.ordinal, back.finished), (1, true));

    assert!(
        f.db.clear_progress(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap()
    );
    assert_eq!(
        f.db.progress(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn a_page_read_offline_does_not_replace_a_later_one() {
    let f = fixture(LibraryKind::Manga).await;
    let me = crate::tests::viewer(&f.db).await;
    let last = f.pages.len() - 1;
    // Tablette, en ligne : page 8.
    saved(
        f.db.save_progress(&me, &f.volume, &f.pages[8])
            .await
            .unwrap(),
    );
    // Téléphone, hors ligne plus tôt : page 2, envoyée après. La page 8 reste.
    let early = Some("2020-01-01T10:00:00.000Z");
    let p = saved(
        f.db.save_progress_at(&me, &f.volume, &f.pages[2], early)
            .await
            .unwrap(),
    );
    assert_eq!((p.ordinal, p.finished), (8, false));
    // Mais la dernière page lue hors ligne marque quand même le tome comme lu.
    let p = saved(
        f.db.save_progress_at(&me, &f.volume, &f.pages[last], early)
            .await
            .unwrap(),
    );
    assert_eq!((p.ordinal, p.finished), (8, true));
    // Une date future compte comme maintenant : elle ne fige pas la progression.
    let future = Some("2999-01-01T00:00:00.000Z");
    saved(
        f.db.save_progress_at(&me, &f.volume, &f.pages[5], future)
            .await
            .unwrap(),
    );
    let p = saved(
        f.db.save_progress(&me, &f.volume, &f.pages[6])
            .await
            .unwrap(),
    );
    assert_eq!(p.ordinal, 6);
    // Premier enregistrement d'un tome, daté : pris tel quel.
    f.db.clear_progress(&me, &f.volume).await.unwrap();
    let p = saved(
        f.db.save_progress_at(&me, &f.volume, &f.pages[4], early)
            .await
            .unwrap(),
    );
    assert_eq!(p.ordinal, 4);
    assert_eq!(
        f.db.save_progress_at(&me, &f.volume, &f.pages[4], Some("hier"))
            .await
            .unwrap(),
        SaveProgress::InvalidTime
    );
}

#[tokio::test]
async fn progress_refuses_foreign_pages_and_unknown_volumes() {
    let f = fixture(LibraryKind::Manga).await;
    assert_eq!(
        f.db.save_progress(&crate::tests::viewer(&f.db).await, &f.volume, &f.other_page)
            .await
            .unwrap(),
        SaveProgress::PageNotInVolume
    );
    let unknown = VolumeId::from_reader(&b"inconnu"[..]).unwrap();
    assert_eq!(
        f.db.save_progress(&crate::tests::viewer(&f.db).await, &unknown, &f.pages[0])
            .await
            .unwrap(),
        SaveProgress::UnknownVolume
    );
    assert!(
        !f.db
            .clear_progress(&crate::tests::viewer(&f.db).await, &unknown)
            .await
            .unwrap()
    );
    assert_eq!(
        f.db.progress(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap(),
        None,
        "rien d'écrit"
    );
}

#[tokio::test]
async fn the_library_kind_gives_the_default_reading_mode() {
    for (kind, mode) in [
        (LibraryKind::Manga, ReadingMode::Rtl),
        (LibraryKind::Comics, ReadingMode::Ltr),
        (LibraryKind::Webtoon, ReadingMode::Vertical),
    ] {
        let f = fixture(kind).await;
        let settings =
            f.db.series_settings(&crate::tests::viewer(&f.db).await, f.series)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(
            settings,
            ReadingSettings {
                reading_mode: mode,
                page_layout: PageLayout::Auto,
                page_turn: PageTurn::Reading,
                reading_mode_override: None,
                page_layout_override: None,
                page_turn_override: None,
            }
        );
        assert_eq!(f.db.libraries().await.unwrap()[0].kind, kind);
        assert_eq!(
            f.db.library_summaries(&crate::tests::viewer(&f.db).await)
                .await
                .unwrap()[0]
                .kind,
            kind
        );
        let volume =
            f.db.volume_detail(&crate::tests::viewer(&f.db).await, &f.volume)
                .await
                .unwrap()
                .unwrap();
        assert_eq!(volume.settings, settings);
    }
}

#[tokio::test]
async fn series_settings_change_one_field_at_a_time_and_reset() {
    let f = fixture(LibraryKind::Manga).await;
    let change = |reading_mode, page_layout| SettingsChange {
        reading_mode,
        page_layout,
        page_turn: None,
    };
    let s =
        f.db.change_series_settings(
            &crate::tests::viewer(&f.db).await,
            f.series,
            change(Some(Some(ReadingMode::Vertical)), None),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (
            s.reading_mode,
            s.reading_mode_override,
            s.page_layout_override
        ),
        (ReadingMode::Vertical, Some(ReadingMode::Vertical), None)
    );
    let s =
        f.db.change_series_settings(
            &crate::tests::viewer(&f.db).await,
            f.series,
            change(None, Some(Some(PageLayout::Double))),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        (s.reading_mode, s.page_layout),
        (ReadingMode::Vertical, PageLayout::Double),
        "le sens choisi avant reste"
    );
    // Le tome suit les réglages de sa série.
    let volume =
        f.db.volume_detail(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(volume.settings, s);
    // `Some(None)` : retour au défaut de la bibliothèque.
    let s =
        f.db.change_series_settings(
            &crate::tests::viewer(&f.db).await,
            f.series,
            change(Some(None), Some(None)),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(s, ReadingSettings::default());
    assert_eq!(
        f.db.change_series_settings(&crate::tests::viewer(&f.db).await, 999, change(None, None))
            .await
            .unwrap(),
        None
    );
    assert_eq!(
        f.db.series_settings(&crate::tests::viewer(&f.db).await, 999)
            .await
            .unwrap(),
        None
    );
}

#[tokio::test]
async fn series_and_volume_details_carry_the_progress() {
    let f = fixture(LibraryKind::Comics).await;
    let detail =
        f.db.series_detail(&crate::tests::viewer(&f.db).await, f.series)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(detail.settings.reading_mode, ReadingMode::Ltr);
    assert_eq!(detail.volumes[0].progress, None);
    let p = saved(
        f.db.save_progress(&crate::tests::viewer(&f.db).await, &f.volume, &f.pages[5])
            .await
            .unwrap(),
    );
    let detail =
        f.db.series_detail(&crate::tests::viewer(&f.db).await, f.series)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(detail.volumes[0].progress.as_ref(), Some(&p));
    let volume =
        f.db.volume_detail(&crate::tests::viewer(&f.db).await, &f.volume)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(volume.progress, Some(p));
}

/// Un tome rattaché à aucune série prend le défaut d'une bibliothèque de manga.
#[tokio::test]
async fn a_volume_outside_any_series_gets_the_manga_default() {
    let f = fixture(LibraryKind::Webtoon).await;
    let page =
        f.db.page_source(&crate::tests::viewer(&f.db).await, &f.other_page)
            .await
            .unwrap()
            .unwrap();
    let detail =
        f.db.volume_detail(&crate::tests::viewer(&f.db).await, &page.volume_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(detail.settings, ReadingSettings::default());
    assert_eq!(detail.settings.reading_mode, ReadingMode::Rtl);
}

/// Chaque compte a sa progression et ses réglages de série (Q48) ; le sens pour tourner les
/// pages par défaut est celui du compte, sinon celui de l'instance.
#[tokio::test]
async fn each_account_has_its_own_progress_and_settings() {
    let f = fixture(LibraryKind::Manga).await;
    let alice = crate::tests::viewer(&f.db).await;
    let bob =
        f.db.create_user("bob", "mot de passe", crate::Role::Reader)
            .await
            .unwrap();
    let bob = f.db.viewer(bob.id).await.unwrap();

    saved(
        f.db.save_progress(&alice, &f.volume, &f.pages[5])
            .await
            .unwrap(),
    );
    assert_eq!(f.db.progress(&bob, &f.volume).await.unwrap(), None);
    saved(
        f.db.save_progress(&bob, &f.volume, &f.pages[2])
            .await
            .unwrap(),
    );
    assert_eq!(
        f.db.progress(&alice, &f.volume)
            .await
            .unwrap()
            .unwrap()
            .ordinal,
        5
    );
    assert!(f.db.clear_progress(&bob, &f.volume).await.unwrap());
    assert!(f.db.progress(&alice, &f.volume).await.unwrap().is_some());

    let change = SettingsChange {
        reading_mode: Some(Some(ReadingMode::Ltr)),
        ..SettingsChange::default()
    };
    f.db.change_series_settings(&bob, f.series, change)
        .await
        .unwrap();
    let mine =
        f.db.series_settings(&alice, f.series)
            .await
            .unwrap()
            .unwrap();
    let his = f.db.series_settings(&bob, f.series).await.unwrap().unwrap();
    assert_eq!(
        (mine.reading_mode, his.reading_mode),
        (ReadingMode::Rtl, ReadingMode::Ltr)
    );

    f.db.set_page_turn(PageTurn::Rtl).await.unwrap();
    sqlx::query("INSERT INTO user_settings (user_id, page_turn) VALUES (?, 'ltr')")
        .bind(bob.user)
        .execute(f.db.pool())
        .await
        .unwrap();
    let mine =
        f.db.series_settings(&alice, f.series)
            .await
            .unwrap()
            .unwrap();
    let his = f.db.series_settings(&bob, f.series).await.unwrap().unwrap();
    assert_eq!(
        (mine.page_turn, his.page_turn),
        (PageTurn::Rtl, PageTurn::Ltr)
    );
    let detail = f.db.volume_detail(&bob, &f.volume).await.unwrap().unwrap();
    assert_eq!(detail.settings.reading_mode, ReadingMode::Ltr);
}

#[tokio::test]
async fn the_viewer_language_is_the_account_one_then_the_instance_one() {
    let f = fixture(LibraryKind::Manga).await;
    let alice = crate::tests::viewer(&f.db).await;
    assert_eq!(alice.language, Language::Fr);
    f.db.set_language(Some(Language::En)).await.unwrap();
    assert_eq!(
        f.db.viewer(alice.user).await.unwrap().language,
        Language::En
    );
    sqlx::query("INSERT INTO user_settings (user_id, language) VALUES (?, 'fr')")
        .bind(alice.user)
        .execute(f.db.pool())
        .await
        .unwrap();
    assert_eq!(
        f.db.viewer(alice.user).await.unwrap().language,
        Language::Fr
    );
}
