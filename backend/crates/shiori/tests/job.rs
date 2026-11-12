//! Job de scan : une passe à la fois, cible confinée à la bibliothèque, compteurs, journal,
//! annulation et arrêt propre.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use shiori::job::{ScanRequest, Scanner, StartError};
use shiori_db::{Db, Library, LibraryKind};
use shiori_testkit::{STORED, fixtures, page_color, png, write_zip};
use sqlx::Row;

struct Fixture {
    dir: tempfile::TempDir,
    db: Arc<Db>,
    library: Library,
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
        let root = dir.path().join("manga");
        std::fs::create_dir_all(root.join("Série A")).unwrap();
        fixtures::unpadded(&root.join("Série A"));
        fixtures::manga(&root.join("Série A"));
        fixtures::bad_crc(&root);
        let library = db
            .create_library("Manga", &root, LibraryKind::Manga)
            .await
            .unwrap();
        Self { dir, db, library }
    }

    fn root(&self) -> PathBuf {
        PathBuf::from(&self.library.root)
    }

    fn request(&self, path: Option<PathBuf>) -> ScanRequest {
        ScanRequest {
            library_id: self.library.id,
            path,
            deep: false,
            rehash: false,
        }
    }

    /// Lance une passe, attend sa fin, renvoie sa ligne `scan_runs`.
    async fn pass(&self, scanner: &Scanner, request: ScanRequest) -> sqlx::sqlite::SqliteRow {
        let run = scanner.start(self.db.clone(), request).await.unwrap();
        scanner.wait().await;
        self.run(run).await
    }

    async fn run(&self, run: i64) -> sqlx::sqlite::SqliteRow {
        sqlx::query("SELECT * FROM scan_runs WHERE id = ?")
            .bind(run)
            .fetch_one(self.db.pool())
            .await
            .unwrap()
    }

    async fn series(&self) -> Vec<String> {
        let mut names: Vec<String> = self
            .db
            .series_of(self.library.id)
            .await
            .unwrap()
            .unwrap()
            .into_iter()
            .map(|s| format!("{} ({})", s.name, s.volume_count))
            .collect();
        names.sort();
        names
    }
}

fn counts(row: &sqlx::sqlite::SqliteRow) -> [i64; 6] {
    [
        "files_total",
        "files_ok",
        "files_warning",
        "files_error",
        "files_unchanged",
        "files_removed",
    ]
    .map(|c| row.get::<Option<i64>, _>(c).unwrap_or(-1))
}

/// Une archive par fichier `n` : assez de travail pour qu'une passe dure un peu.
fn many(dir: &Path, n: usize) {
    for i in 0..n {
        let page = png(32, 2400, page_color(i));
        let files: Vec<(String, Vec<u8>)> = (0..30)
            .map(|p| (format!("{p:02}.png"), page.clone()))
            .collect();
        let files: Vec<_> = files
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
            .collect();
        write_zip(dir, &format!("Tome {i:02}.cbz"), &files);
    }
}

#[tokio::test]
async fn library_pass_scans_attaches_and_counts() {
    let f = Fixture::new().await;
    let scanner = Scanner::new(2, 30);
    let row = f.pass(&scanner, f.request(None)).await;
    assert_eq!(row.get::<String, _>("status"), "completed");
    assert_eq!(row.get::<String, _>("origin"), "api");
    assert_eq!(row.get::<i64, _>("library_id"), f.library.id);
    // 3 fichiers : manga ok, unpadded avertissement, bad-crc erreur.
    assert_eq!(counts(&row), [3, 1, 1, 1, 0, 0]);
    assert_eq!(f.series().await, ["Série A (2)", "bad-crc (1)"]);
    let logs: i64 = sqlx::query("SELECT COUNT(*) AS n FROM scan_run_logs WHERE scan_run_id = ?")
        .bind(row.get::<i64, _>("id"))
        .fetch_one(f.db.pool())
        .await
        .unwrap()
        .get("n");
    assert!(logs >= 3);
    assert!(
        scanner.progress().await.is_none(),
        "rien ne tourne après la fin"
    );

    // Deuxième passe : tout est inchangé ; un fichier supprimé est retiré.
    std::fs::remove_file(f.root().join("bad-crc.cbz")).unwrap();
    let row = f.pass(&scanner, f.request(None)).await;
    assert_eq!(counts(&row), [2, 1, 1, 0, 2, 1]);
    assert_eq!(f.series().await, ["Série A (2)"]);
}

#[tokio::test]
async fn folder_and_file_targets() {
    let f = Fixture::new().await;
    let scanner = Scanner::new(1, 30);
    let folder = f.root().join("Série A");
    let row = f.pass(&scanner, f.request(Some(folder.clone()))).await;
    assert_eq!(counts(&row)[0], 2);
    assert_eq!(row.get::<String, _>("root"), folder.display().to_string());

    let file = f.root().join("bad-crc.cbz");
    let row = f.pass(&scanner, f.request(Some(file))).await;
    assert_eq!(counts(&row), [1, 0, 0, 1, 0, 0]);
    assert_eq!(f.series().await, ["Série A (2)", "bad-crc (1)"]);
}

#[tokio::test]
async fn targets_outside_the_library_are_refused() {
    let f = Fixture::new().await;
    let scanner = Scanner::new(1, 30);
    let outside = f.dir.path().to_path_buf();
    let sneaky = f.root().join("..").join("manga");
    let cases = [
        (f.request(Some(outside)), "OutsideLibrary"),
        (f.request(Some(sneaky)), "OutsideLibrary"),
        (f.request(Some(f.root().join("absent"))), "NotFound"),
        (
            ScanRequest {
                library_id: 999,
                ..f.request(None)
            },
            "UnknownLibrary",
        ),
    ];
    for (request, expected) in cases {
        let err = scanner.start(f.db.clone(), request).await.unwrap_err();
        assert!(format!("{err:?}").starts_with(expected), "{err:?}");
        assert!(!err.to_string().is_empty());
    }
    #[cfg(unix)]
    {
        let link = f.root().join("lien");
        std::os::unix::fs::symlink(f.dir.path(), &link).unwrap();
        let err = scanner
            .start(f.db.clone(), f.request(Some(link)))
            .await
            .unwrap_err();
        assert!(matches!(err, StartError::OutsideLibrary(_)), "{err:?}");
    }
}

#[tokio::test]
async fn one_pass_at_a_time_and_cancellation() {
    let f = Fixture::new().await;
    many(&f.root(), 12);
    let scanner = Scanner::new(1, 30);
    let run = scanner.start(f.db.clone(), f.request(None)).await.unwrap();
    let again = scanner.start(f.db.clone(), f.request(None)).await;
    assert!(matches!(again, Err(StartError::AlreadyRunning)));
    let progress = scanner.progress().await.unwrap();
    assert_eq!((progress.run, progress.library_id), (run, f.library.id));

    assert!(scanner.cancel().await);
    scanner.wait().await;
    let row = f.run(run).await;
    assert_eq!(row.get::<String, _>("status"), "cancelled");
    let [total, ok, warning, error, ..] = counts(&row);
    assert_eq!(total, 15);
    assert!(ok + warning + error < total, "{:?}", counts(&row));
    assert!(!scanner.cancel().await, "plus rien à annuler");
    // Une nouvelle passe peut démarrer.
    assert_eq!(
        f.pass(&scanner, f.request(None))
            .await
            .get::<String, _>("status"),
        "completed"
    );
}

/// Arrêt du serveur pendant un scan : la passe est annulée et close, jamais `running`.
#[tokio::test]
async fn shutdown_closes_the_running_pass() {
    let f = Fixture::new().await;
    many(&f.root(), 12);
    let scanner = Scanner::new(1, 30);
    let run = scanner.start(f.db.clone(), f.request(None)).await.unwrap();
    scanner.shutdown().await;
    assert_eq!(f.run(run).await.get::<String, _>("status"), "cancelled");
    scanner.shutdown().await; // rien à faire, sans erreur
}

/// Erreur de base en cours de passe : la passe finit `failed` avec son erreur.
#[tokio::test]
async fn database_failure_marks_the_pass_failed() {
    let f = Fixture::new().await;
    sqlx::query("DROP TABLE series")
        .execute(f.db.pool())
        .await
        .unwrap();
    let scanner = Scanner::new(1, 30);
    let row = f.pass(&scanner, f.request(None)).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert!(
        row.get::<String, _>("error").contains("series"),
        "{}",
        row.get::<String, _>("error")
    );
}
