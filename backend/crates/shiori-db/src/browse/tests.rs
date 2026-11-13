use std::path::Path;

use shiori_core::{PageId, ScanOptions, Status};
use shiori_testkit::{STORED, fixtures, png, write_zip};

use super::*;
use crate::Library;

/// Tome de `pages` pages, de contenu propre à `seed` (donc une identité à lui).
fn volume(dir: &Path, name: &str, seed: u8, pages: usize) -> std::path::PathBuf {
    let images: Vec<(String, Vec<u8>)> = (0..pages)
        .map(|i| (format!("{i:02}.png"), png(4, 6, [seed, i as u8, 0])))
        .collect();
    let files: Vec<_> = images
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
        .collect();
    write_zip(dir, name, &files)
}

struct Fixture {
    _dir: tempfile::TempDir,
    db: Db,
    library: Library,
    other: Library,
    /// Pages des tomes 1, 2 et 4 de « Série » (le tome 3 est en erreur).
    pages: HashMap<u8, Vec<PageId>>,
    volumes: HashMap<u8, String>,
}

impl Fixture {
    async fn read(&self, tome: u8, page: usize) {
        let volume = self.volumes[&tome].parse().unwrap();
        self.db
            .save_progress(&volume, &self.pages[&tome][page])
            .await
            .unwrap();
    }

    async fn series(&self, name: &str) -> SeriesSummary {
        self.db
            .all_series()
            .await
            .unwrap()
            .into_iter()
            .find(|s| s.name == name)
            .unwrap()
    }
}

/// « Série » : tomes 1, 2, 3 (sans image : en erreur) et 4, de 3 pages ; « Autre » : un tome,
/// dans une seconde bibliothèque.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("lib");
    let folder = root.join("Série");
    let other_root = dir.path().join("autre");
    let other_folder = other_root.join("Autre");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::create_dir_all(&other_folder).unwrap();
    let broken = fixtures::no_images(&folder);
    std::fs::rename(&broken, folder.join("Série T03.cbz")).unwrap();
    let files = [
        (1, volume(&folder, "Série T01.cbz", 1, 3)),
        (2, volume(&folder, "Série T02.cbz", 2, 3)),
        (3, folder.join("Série T03.cbz")),
        (4, volume(&folder, "Série T04.cbz", 4, 3)),
        (9, volume(&other_folder, "Autre T01.cbz", 9, 3)),
    ];
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let library = db
        .create_library("Lib", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let other = db
        .create_library("Autre", &other_root, LibraryKind::Comics)
        .await
        .unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let (mut pages, mut volumes) = (HashMap::new(), HashMap::new());
    for (tome, path) in &files {
        let scanned = db
            .scan(run, path, ScanOptions::default(), false)
            .await
            .unwrap();
        assert_eq!(scanned.report.status == Status::Error, *tome == 3);
        let lib = if *tome == 9 { &other } else { &library };
        assert!(db.attach(lib, path).await.unwrap());
        volumes.insert(*tome, scanned.report.volume_id.unwrap().to_string());
        pages.insert(
            *tome,
            scanned
                .report
                .pages
                .iter()
                .map(|p| p.id)
                .collect::<Vec<_>>(),
        );
    }
    db.refresh_series(&library).await.unwrap();
    db.refresh_series(&other).await.unwrap();
    Fixture {
        _dir: dir,
        db,
        library,
        other,
        pages,
        volumes,
    }
}

#[tokio::test]
async fn summaries_follow_the_reading() {
    let f = fixture().await;
    let all = f.db.all_series().await.unwrap();
    assert_eq!(
        all.iter().map(|s| s.name.as_str()).collect::<Vec<_>>(),
        ["Autre", "Série"]
    );
    assert_eq!(f.db.series_of(f.other.id).await.unwrap().unwrap().len(), 1);
    assert_eq!(f.db.series_of(999).await.unwrap().map(|s| s.len()), None);

    // Jamais ouverte : premier tome, date de création écrite par le scan.
    let s = f.series("Série").await;
    assert_eq!((s.library_id, s.volume_count), (f.library.id, 4));
    assert_eq!((s.read_count, s.reading_status), (0, ReadingStatus::Unread));
    assert_eq!((s.last_read_at, s.publication_status), (None, None));
    assert!(s.created_at.ends_with('Z'), "{}", s.created_at);
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&1]));

    // Tome 1 commencé : on le reprend.
    f.read(1, 1).await;
    let s = f.series("Série").await;
    assert_eq!(
        (s.read_count, s.reading_status),
        (0, ReadingStatus::InProgress)
    );
    assert!(s.last_read_at.is_some());
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&1]));

    // Tomes 1 et 2 finis : le tome 3 est en erreur, on passe au 4.
    f.read(1, 2).await;
    f.read(2, 2).await;
    let s = f.series("Série").await;
    assert_eq!(
        (s.read_count, s.reading_status),
        (2, ReadingStatus::InProgress)
    );
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&4]));
    // Le détail de la série suit la même règle.
    let detail = f.db.series_detail(s.id).await.unwrap().unwrap();
    assert_eq!(detail.resume_volume_id, s.resume_volume_id);

    // Tout ce qui se lit est lu, sauf le tome en erreur : la série n'est pas « lue ».
    f.read(4, 2).await;
    let s = f.series("Série").await;
    assert_eq!(
        (s.read_count, s.reading_status),
        (3, ReadingStatus::InProgress)
    );

    // Une série entièrement lue se relit depuis le début.
    f.read(9, 2).await;
    let s = f.series("Autre").await;
    assert_eq!((s.read_count, s.reading_status), (1, ReadingStatus::Read));
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&9]));
}

#[tokio::test]
async fn resume_falls_back_to_the_first_unread_volume() {
    let f = fixture().await;
    // Le dernier tome lu (4) est fini et n'a pas de suite ; le tome 1 n'a jamais été lu.
    f.read(4, 2).await;
    let s = f.series("Série").await;
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&1]));
    assert!(f.db.continue_reading(20).await.unwrap().is_empty());
}

#[tokio::test]
async fn continue_reading_lists_the_next_volume_of_each_series() {
    let f = fixture().await;
    assert!(f.db.continue_reading(20).await.unwrap().is_empty());

    f.read(9, 1).await; // « Autre » : en cours
    f.read(1, 2).await; // « Série » : tome 1 fini, puis tome 2 à lire
    let items = f.db.continue_reading(20).await.unwrap();
    let got: Vec<_> = items
        .iter()
        .map(|i| (i.series_name.as_str(), i.volume.volume_id.as_str()))
        .collect();
    assert_eq!(
        got,
        [
            ("Série", f.volumes[&2].as_str()),
            ("Autre", f.volumes[&9].as_str())
        ]
    );
    // Le tome suivant n'a pas encore de progression ; le tome en cours a la sienne.
    assert_eq!(items[0].volume.progress, None);
    assert_eq!(items[1].volume.progress.as_ref().unwrap().ordinal, 1);
    assert_eq!(items[0].library_id, f.library.id);
    assert_eq!(f.db.continue_reading(1).await.unwrap().len(), 1);

    // Tome 2 fini : le tome 3, en erreur, est sauté.
    f.read(2, 2).await;
    let items = f.db.continue_reading(20).await.unwrap();
    assert_eq!(items[0].volume.volume_id, f.volumes[&4]);

    // Relire le tome 1, déjà fini : c'est lui qu'on reprend, pas la suite.
    f.read(1, 1).await;
    let items = f.db.continue_reading(20).await.unwrap();
    assert_eq!(items[0].volume.volume_id, f.volumes[&1]);
    let s = f.series("Série").await;
    assert_eq!(s.resume_volume_id.as_ref(), Some(&f.volumes[&1]));
    assert_eq!(s.read_count, 2);

    // « Autre » fini, sans suite : elle disparaît.
    f.read(9, 2).await;
    let items = f.db.continue_reading(20).await.unwrap();
    assert_eq!(items.len(), 1);
}

#[tokio::test]
async fn publication_status_is_set_and_cleared() {
    let f = fixture().await;
    let id = f.series("Série").await.id;
    assert!(
        f.db.set_publication_status(id, Some(PublicationStatus::Hiatus))
            .await
            .unwrap()
    );
    assert_eq!(
        f.series("Série").await.publication_status,
        Some(PublicationStatus::Hiatus)
    );
    let detail = f.db.series_detail(id).await.unwrap().unwrap();
    assert_eq!(detail.publication_status, Some(PublicationStatus::Hiatus));
    assert!(f.db.set_publication_status(id, None).await.unwrap());
    assert_eq!(f.series("Série").await.publication_status, None);
    assert!(!f.db.set_publication_status(999, None).await.unwrap());
}
