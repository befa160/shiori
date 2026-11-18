//! Surveillance des dossiers (lot 9.1) : rattrapage au démarrage, scan ciblé du dossier de
//! série touché, retrait d'une série supprimée, bibliothèque non surveillée, scan périodique.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use shiori::cache::Images;
use shiori::server::App;
use shiori::watch::{Timing, spawn};
use shiori_db::{Db, LibraryKind};
use shiori_testkit::fixtures;
use sqlx::Row;

const FAST: Timing = Timing {
    calm: Duration::from_millis(300),
    tick: Duration::from_millis(50),
    retry: Duration::from_millis(500),
    hour: Duration::from_secs(3600),
};

struct Fixture {
    _dir: tempfile::TempDir,
    app: Arc<App>,
    root: std::path::PathBuf,
    library: i64,
}

async fn fixture(watch: bool) -> Fixture {
    // Événements reçus, montrés si un test échoue (Windows et macOS diffèrent de Linux).
    let _ = tracing_subscriber::fmt()
        .with_env_filter("shiori::watch=debug")
        .with_test_writer()
        .try_init();
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("manga");
    std::fs::create_dir_all(root.join("Série A")).unwrap();
    fixtures::manga(&root.join("Série A"));
    // Une seconde série : une racine vidée passerait pour démontée, rien n'en serait retiré.
    std::fs::create_dir_all(root.join("Série B")).unwrap();
    fixtures::bad_crc(&root.join("Série B"));
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let library = db
        .create_library("Manga", &root, LibraryKind::Manga)
        .await
        .unwrap();
    db.set_library_watch(library.id, watch).await.unwrap();
    let app = Arc::new(App::new(db, images));
    Fixture {
        _dir: dir,
        app,
        root: std::path::PathBuf::from(&library.root),
        library: library.id,
    }
}

impl Fixture {
    /// `(dossier, déclencheur)` des passes terminées, dans l'ordre.
    async fn runs(&self) -> Vec<(String, String)> {
        sqlx::query(
            "SELECT root, options ->> 'trigger' AS t FROM scan_runs
             WHERE status = 'completed' ORDER BY id",
        )
        .fetch_all(self.app.db.pool())
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get("root"), r.get("t")))
        .collect()
    }

    async fn locations(&self) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM file_locations WHERE library_id = ?")
            .bind(self.library)
            .fetch_one(self.app.db.pool())
            .await
            .unwrap()
    }

    /// Attend (20 s au plus) qu'une passe terminée vérifie `done`.
    async fn until(&self, what: &str, done: impl Fn(&[(String, String)]) -> bool) {
        for _ in 0..400 {
            if done(&self.runs().await) && self.app.scanner.progress().await.is_none() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        panic!("{what} : {:?}", self.runs().await);
    }
}

fn shown(path: &Path) -> String {
    path.display().to_string()
}

#[tokio::test]
async fn changes_are_scanned_where_they_happen() {
    let f = fixture(true).await;
    let task = spawn(f.app.clone(), FAST);
    // Rattrapage : ce qui a changé serveur éteint.
    let root = shown(&f.root);
    f.until("rattrapage", |runs| runs.iter().any(|(r, _)| *r == root))
        .await;
    assert_eq!(f.runs().await[0], (root.clone(), "watch".to_owned()));
    assert_eq!(f.locations().await, 2);

    // Nouveau tome dans une série existante : seul son dossier est scanné.
    let series = f.root.join("Série A");
    fixtures::unpadded(&series);
    let folder = shown(&series);
    f.until("scan ciblé", |runs| runs.iter().any(|(r, _)| *r == folder))
        .await;
    assert_eq!(f.locations().await, 3);

    // Série supprimée : la racine est rescannée et ses tomes retirés.
    std::fs::remove_dir_all(&series).unwrap();
    let before = f.runs().await.len();
    f.until("retrait", |runs| {
        runs.len() > before && runs[before..].iter().any(|(r, _)| *r == root)
    })
    .await;
    assert_eq!(f.locations().await, 1);
    task.abort();
}

#[tokio::test]
async fn an_unwatched_library_waits_for_the_periodic_scan() {
    let f = fixture(false).await;
    f.app.db.set_full_scan_hours(1).await.unwrap();
    let timing = Timing {
        hour: Duration::from_millis(600),
        ..FAST
    };
    let task = spawn(f.app.clone(), timing);
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        f.runs().await.is_empty(),
        "aucun rattrapage sans surveillance"
    );
    let root = shown(&f.root);
    f.until("scan périodique", |runs| !runs.is_empty()).await;
    assert_eq!(f.runs().await[0], (root, "schedule".to_owned()));
    task.abort();
}

#[tokio::test]
async fn stopping_the_watch_and_deleting_the_library_are_followed() {
    let f = fixture(true).await;
    let task = spawn(f.app.clone(), FAST);
    f.until("rattrapage", |runs| !runs.is_empty()).await;
    f.app.db.set_library_watch(f.library, false).await.unwrap();
    tokio::time::sleep(Duration::from_millis(200)).await; // le tick lâche la racine
    fixtures::unpadded(&f.root.join("Série A"));
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(f.runs().await.len(), 1, "plus de scan une fois arrêtée");
    // Surveillance rétablie : rattrapage de ce qui a changé entre-temps.
    f.app.db.set_library_watch(f.library, true).await.unwrap();
    f.until("reprise", |runs| runs.len() == 2).await;
    assert_eq!(f.locations().await, 3);
    task.abort();
}

#[tokio::test]
async fn a_root_mounted_late_is_watched_once_it_appears() {
    let f = fixture(true).await;
    let hidden = f.root.with_extension("démonté");
    std::fs::rename(&f.root, &hidden).unwrap();
    let task = spawn(f.app.clone(), FAST);
    tokio::time::sleep(Duration::from_millis(300)).await;
    std::fs::rename(&hidden, &f.root).unwrap();
    let root = shown(&f.root);
    f.until("racine revenue", |runs| {
        runs.iter().any(|(r, _)| *r == root)
    })
    .await;
    assert_eq!(f.locations().await, 2);
    task.abort();
}

#[tokio::test]
async fn what_shiori_writes_itself_is_not_a_change() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("manga");
    std::fs::create_dir_all(root.join("Série A")).unwrap();
    fixtures::manga(&root.join("Série A"));
    // Base et cache rangés dans la bibliothèque elle-même.
    let db = Arc::new(Db::open(&root.join("shiori.db")).await.unwrap());
    let images = Images::open(root.join("cache"), u64::MAX).unwrap();
    db.create_library("Manga", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let app = Arc::new(App::new(db.clone(), images));
    let task = spawn(app.clone(), FAST);
    let runs = || async {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM scan_runs")
            .fetch_one(db.pool())
            .await
            .unwrap()
    };
    for _ in 0..200 {
        if runs().await == 1 && app.scanner.progress().await.is_none() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    std::fs::write(root.join("cache").join("x"), b"rendu").unwrap();
    tokio::time::sleep(Duration::from_millis(1000)).await;
    assert_eq!(
        runs().await,
        1,
        "le rattrapage seul, pas de scan relancé par la base"
    );
    task.abort();
}

#[tokio::test]
async fn a_library_created_while_running_is_left_to_its_own_scan() {
    let f = fixture(true).await;
    let task = spawn(f.app.clone(), FAST);
    f.until("rattrapage", |runs| runs.len() == 1).await;
    let bd = f.root.with_file_name("bd");
    std::fs::create_dir_all(bd.join("Série")).unwrap();
    fixtures::manga(&bd.join("Série"));
    f.app
        .db
        .create_library("BD", &bd, LibraryKind::Comics)
        .await
        .unwrap();
    tokio::time::sleep(Duration::from_millis(800)).await;
    assert_eq!(
        f.runs().await.len(),
        1,
        "le scan de l'interface ne doit pas être doublé"
    );
    // Mais elle est bien surveillée : un ajout est scanné.
    fixtures::unpadded(&bd.join("Série"));
    let folder = shown(&bd.join("Série"));
    f.until("scan ciblé", |runs| runs.iter().any(|(r, _)| *r == folder))
        .await;
    task.abort();
}
