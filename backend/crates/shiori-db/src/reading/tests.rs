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
    let series = db.series_of(library.id).await.unwrap().unwrap()[0].id;
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
    assert_eq!(f.db.progress(&f.volume).await.unwrap(), None);

    let p = saved(f.db.save_progress(&f.volume, &f.pages[3]).await.unwrap());
    assert_eq!(
        (p.page_id.as_str(), p.ordinal, p.page_count),
        (&*f.pages[3].to_string(), 3, 12)
    );
    assert!(!p.finished);
    assert_eq!(f.db.progress(&f.volume).await.unwrap(), Some(p));

    // La dernière page marque le tome comme lu ; revenir en arrière ne l'efface pas.
    assert!(saved(f.db.save_progress(&f.volume, &f.pages[last]).await.unwrap()).finished);
    let back = saved(f.db.save_progress(&f.volume, &f.pages[1]).await.unwrap());
    assert_eq!((back.ordinal, back.finished), (1, true));

    assert!(f.db.clear_progress(&f.volume).await.unwrap());
    assert_eq!(f.db.progress(&f.volume).await.unwrap(), None);
}

#[tokio::test]
async fn progress_refuses_foreign_pages_and_unknown_volumes() {
    let f = fixture(LibraryKind::Manga).await;
    assert_eq!(
        f.db.save_progress(&f.volume, &f.other_page).await.unwrap(),
        SaveProgress::PageNotInVolume
    );
    let unknown = VolumeId::from_reader(&b"inconnu"[..]).unwrap();
    assert_eq!(
        f.db.save_progress(&unknown, &f.pages[0]).await.unwrap(),
        SaveProgress::UnknownVolume
    );
    assert!(!f.db.clear_progress(&unknown).await.unwrap());
    assert_eq!(
        f.db.progress(&f.volume).await.unwrap(),
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
        let settings = f.db.series_settings(f.series).await.unwrap().unwrap();
        assert_eq!(
            settings,
            ReadingSettings {
                reading_mode: mode,
                page_layout: PageLayout::Auto,
                reading_mode_override: None,
                page_layout_override: None,
            }
        );
        assert_eq!(f.db.libraries().await.unwrap()[0].kind, kind);
        assert_eq!(f.db.library_summaries().await.unwrap()[0].kind, kind);
        let volume = f.db.volume_detail(&f.volume).await.unwrap().unwrap();
        assert_eq!(volume.settings, settings);
    }
}

#[tokio::test]
async fn series_settings_change_one_field_at_a_time_and_reset() {
    let f = fixture(LibraryKind::Manga).await;
    let change = |reading_mode, page_layout| SettingsChange {
        reading_mode,
        page_layout,
    };
    let s =
        f.db.change_series_settings(f.series, change(Some(Some(ReadingMode::Vertical)), None))
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
        f.db.change_series_settings(f.series, change(None, Some(Some(PageLayout::Double))))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(
        (s.reading_mode, s.page_layout),
        (ReadingMode::Vertical, PageLayout::Double),
        "le sens choisi avant reste"
    );
    // Le tome suit les réglages de sa série.
    let volume = f.db.volume_detail(&f.volume).await.unwrap().unwrap();
    assert_eq!(volume.settings, s);
    // `Some(None)` : retour au défaut de la bibliothèque.
    let s =
        f.db.change_series_settings(f.series, change(Some(None), Some(None)))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(s, ReadingSettings::default());
    assert_eq!(
        f.db.change_series_settings(999, change(None, None))
            .await
            .unwrap(),
        None
    );
    assert_eq!(f.db.series_settings(999).await.unwrap(), None);
}

#[tokio::test]
async fn series_and_volume_details_carry_the_progress() {
    let f = fixture(LibraryKind::Comics).await;
    let detail = f.db.series_detail(f.series).await.unwrap().unwrap();
    assert_eq!(detail.settings.reading_mode, ReadingMode::Ltr);
    assert_eq!(detail.volumes[0].progress, None);
    let p = saved(f.db.save_progress(&f.volume, &f.pages[5]).await.unwrap());
    let detail = f.db.series_detail(f.series).await.unwrap().unwrap();
    assert_eq!(detail.volumes[0].progress.as_ref(), Some(&p));
    let volume = f.db.volume_detail(&f.volume).await.unwrap().unwrap();
    assert_eq!(volume.progress, Some(p));
}

/// Un tome rattaché à aucune série prend le défaut d'une bibliothèque de manga.
#[tokio::test]
async fn a_volume_outside_any_series_gets_the_manga_default() {
    let f = fixture(LibraryKind::Webtoon).await;
    let page = f.db.page_source(&f.other_page).await.unwrap().unwrap();
    let detail = f.db.volume_detail(&page.volume_id).await.unwrap().unwrap();
    assert_eq!(detail.settings, ReadingSettings::default());
    assert_eq!(detail.settings.reading_mode, ReadingMode::Rtl);
}
