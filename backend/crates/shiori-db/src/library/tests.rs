use std::path::{Path, PathBuf};

use shiori_core::ScanOptions;
use shiori_testkit::{STORED, page_color, png, write_zip};

use super::*;

/// Écrit un CBZ d'une page (couleur propre à `n`) à `rel` sous `root`.
fn cbz(root: &Path, rel: &str, n: usize) -> PathBuf {
    let path = root.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let page = png(4, 6, page_color(n));
    write_zip(
        path.parent().unwrap(),
        path.file_name().unwrap().to_str().unwrap(),
        &[("01.png", &page, STORED)],
    )
}

async fn scan_all(db: &Db, library: &Library, files: &[PathBuf]) -> i64 {
    let run = db.start_run(&library.root, "{}").await.unwrap();
    for file in files {
        db.scan(run, file, ScanOptions::default(), false)
            .await
            .unwrap();
        assert!(
            db.attach(library, file).await.unwrap(),
            "{}",
            file.display()
        );
    }
    run
}

/// (dossier, nom, nombre de fichiers) par série, triés par nom.
async fn series(db: &Db, library: &Library) -> Vec<(Option<String>, String, i64)> {
    sqlx::query(
        "SELECT s.folder, s.name, COUNT(l.path) AS n FROM series s
         LEFT JOIN file_locations l ON l.series_id = s.id
         WHERE s.library_id = ? GROUP BY s.id ORDER BY s.name, s.id",
    )
    .bind(library.id)
    .fetch_all(db.pool())
    .await
    .unwrap()
    .iter()
    .map(|r| (r.get("folder"), r.get("name"), r.get("n")))
    .collect()
}

async fn count(db: &Db, table: &str) -> i64 {
    let sql = match table {
        "volumes" => "SELECT COUNT(*) AS n FROM volumes",
        "pages" => "SELECT COUNT(*) AS n FROM pages",
        _ => "SELECT COUNT(*) AS n FROM file_locations",
    };
    sqlx::query(sql)
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get("n")
}

async fn setup() -> (tempfile::TempDir, Db, Library) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    let root = dir.path().join("manga");
    std::fs::create_dir(&root).unwrap();
    let library = db
        .create_library(" Manga ", &root, LibraryKind::Manga)
        .await
        .unwrap();
    (dir, db, library)
}

#[test]
fn roots_and_relative_paths() {
    assert_eq!(relative("/m", "/m/a/b.cbz"), Some("a/b.cbz"));
    assert_eq!(relative("/m", "/manga/b.cbz"), None);
    assert_eq!(relative("/m", "/m"), None);
    // Racine du système de fichiers : clé vide.
    assert_eq!(relative("", "/a.cbz"), Some("a.cbz"));
    assert!(overlaps("/m", "/m"));
    assert!(overlaps("/m", "/m/sub"));
    assert!(overlaps("/m/sub", "/m"));
    assert!(overlaps("", "/m"));
    assert!(!overlaps("/m", "/manga"));
}

#[tokio::test]
async fn libraries_are_validated() {
    let (dir, db, library) = setup().await;
    assert_eq!(library.name, "Manga");
    assert!(library.root.ends_with("/manga"), "{}", library.root);
    let other = dir.path().join("bd");
    std::fs::create_dir(&other).unwrap();
    let file = cbz(dir.path(), "seul.cbz", 0);

    let err = |r: Result<Library, LibraryError>| r.unwrap_err().to_string();
    assert!(matches!(
        db.create_library("  ", &other, LibraryKind::Manga).await,
        Err(LibraryError::EmptyName)
    ));
    assert!(matches!(
        db.create_library("Manga", &other, LibraryKind::Manga).await,
        Err(LibraryError::NameTaken(_))
    ));
    assert!(matches!(
        db.create_library("Fichier", &file, LibraryKind::Manga)
            .await,
        Err(LibraryError::NotADirectory(_))
    ));
    assert!(matches!(
        db.create_library("Absent", &dir.path().join("absent"), LibraryKind::Manga)
            .await,
        Err(LibraryError::NotADirectory(_))
    ));
    // Un chemin relatif dépendrait du dossier de lancement du serveur.
    assert!(matches!(
        db.create_library("Relatif", Path::new("manga"), LibraryKind::Manga)
            .await,
        Err(LibraryError::NotADirectory(_))
    ));
    let sub = dir.path().join("manga/sous");
    std::fs::create_dir(&sub).unwrap();
    assert!(err(db.create_library("Sous", &sub, LibraryKind::Manga).await).contains("« Manga »"));
    assert!(matches!(
        db.create_library("Parent", dir.path(), LibraryKind::Manga)
            .await,
        Err(LibraryError::Overlaps(..))
    ));
    // Un `/` final ne crée pas une seconde racine.
    let mut slash = dir.path().join("manga").into_os_string();
    slash.push("/");
    assert!(matches!(
        db.create_library("Bis", Path::new(&slash), LibraryKind::Manga)
            .await,
        Err(LibraryError::Overlaps(..))
    ));

    let bd = db
        .create_library("BD", &other, LibraryKind::Manga)
        .await
        .unwrap();
    let names: Vec<String> = db
        .libraries()
        .await
        .unwrap()
        .into_iter()
        .map(|l| l.name)
        .collect();
    assert_eq!(names, ["BD", "Manga"]);
    assert!(db.delete_library(bd.id).await.unwrap());
    assert!(!db.delete_library(bd.id).await.unwrap());
}

#[tokio::test]
async fn files_are_grouped_by_folder_and_root_files_join_by_name() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let files = [
        cbz(&root, "Wind.Breaker.T02.cbz", 1),
        cbz(&root, "Berserk (01-41+) [Digital-1920]/Berserk T02.cbz", 2),
        cbz(&root, "Berserk T03.cbz", 3),
        cbz(
            &root,
            "Berserk (01-41+) [Digital-1920]/Berserk T01 (2004).cbz",
            4,
        ),
        cbz(&root, "wind breaker t01.cbz", 5),
        cbz(&root, "Noritaka/Volume 1 VF_00067d.cbz", 6),
        cbz(&root, "Noritaka/Chapitre 2.cbz", 7),
    ];
    scan_all(&db, &library, &files).await;
    db.refresh_series(&library).await.unwrap();

    let folder = Some("Berserk (01-41+) [Digital-1920]".to_owned());
    assert_eq!(
        series(&db, &library).await,
        [
            // Le fichier de la racine a rejoint la série-dossier de même nom.
            (folder, "Berserk".to_owned(), 3),
            (Some("Noritaka".to_owned()), "Noritaka".to_owned(), 2),
            // Nom du premier tome de la série (`wind breaker t01`), pas du premier scanné.
            (None, "wind breaker".to_owned(), 2),
        ]
    );
    let row = sqlx::query("SELECT kind, number, number_end FROM file_locations WHERE path = ?")
        .bind(location_key(&files[6]))
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("kind"), "chapter");
    assert_eq!(row.get::<Option<f64>, _>("number"), Some(2.0));
    assert_eq!(row.get::<Option<f64>, _>("number_end"), None);

    // Idempotent : un second passage ne change rien.
    scan_all(&db, &library, &files).await;
    db.refresh_series(&library).await.unwrap();
    assert_eq!(series(&db, &library).await.len(), 3);
}

#[tokio::test]
async fn missing_files_leave_their_series_but_volumes_stay() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let kept = cbz(&root, "Gigant - 01-10/Gigant 01.cbz", 1);
    let gone = cbz(&root, "Blue Lock/Blue Lock T01.cbz", 2);
    let elsewhere = cbz(&root, "Gigant - 01-10/Gigant 02.cbz", 3);
    scan_all(
        &db,
        &library,
        &[kept.clone(), gone.clone(), elsewhere.clone()],
    )
    .await;
    db.refresh_series(&library).await.unwrap();
    assert_eq!(series(&db, &library).await.len(), 2);

    std::fs::remove_file(&gone).unwrap();
    // Passe limitée à un dossier : seul ce qu'il contient peut être retiré.
    let run = scan_all(&db, &library, std::slice::from_ref(&kept)).await;
    let gigant = root.join("Gigant - 01-10");
    assert_eq!(db.remove_missing(&library, &gigant, run).await.unwrap(), 1);
    assert_eq!(count(&db, "file_locations").await, 2);
    // Passe complète.
    assert_eq!(db.remove_missing(&library, &root, run).await.unwrap(), 1);
    db.refresh_series(&library).await.unwrap();
    assert_eq!(
        series(&db, &library).await,
        [(Some("Gigant - 01-10".to_owned()), "Gigant".to_owned(), 1)]
    );
    assert_eq!(count(&db, "volumes").await, 3);
    assert_eq!(count(&db, "pages").await, 3);
}

#[tokio::test]
async fn deleting_a_library_keeps_volumes_and_files() {
    let (_dir, db, library) = setup().await;
    let file = cbz(Path::new(&library.root), "Blue Lock/Blue Lock T01.cbz", 1);
    scan_all(&db, &library, std::slice::from_ref(&file)).await;
    db.refresh_series(&library).await.unwrap();

    assert!(db.delete_library(library.id).await.unwrap());
    assert_eq!(count(&db, "file_locations").await, 0);
    assert!(series(&db, &library).await.is_empty());
    assert_eq!(count(&db, "volumes").await, 1);
    assert_eq!(count(&db, "pages").await, 1);
    assert!(file.exists());
}

#[tokio::test]
async fn attach_ignores_files_outside_the_root_or_never_located_but_keeps_broken_ones() {
    let (dir, db, library) = setup().await;
    let outside = cbz(dir.path(), "dehors.cbz", 1);
    let run = db.start_run("/", "{}").await.unwrap();
    db.scan(run, &outside, ScanOptions::default(), false)
        .await
        .unwrap();
    assert!(!db.attach(&library, &outside).await.unwrap());

    let unscanned = cbz(Path::new(&library.root), "Neuf/Neuf T01.cbz", 2);
    assert!(!db.attach(&library, &unscanned).await.unwrap());
    let broken = Path::new(&library.root).join("Neuf/casse.cbz");
    std::fs::write(&broken, b"pas un zip").unwrap();
    db.scan(run, &broken, ScanOptions::default(), false)
        .await
        .unwrap();
    // Un fichier illisible a un tome en erreur : il est rattaché pour que l'erreur se voie.
    assert!(db.attach(&library, &broken).await.unwrap());
    assert_eq!(series(&db, &library).await.len(), 1);
}

/// Le job rattache les fichiers en parallèle : aucun échec, une seule série par nom.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn parallel_attaches_share_one_series() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let files: Vec<PathBuf> = (0..16)
        .map(|i| cbz(&root, &format!("Kaiju No 8 T{i:02}.cbz"), i))
        .collect();
    let run = db.start_run(&library.root, "{}").await.unwrap();
    for file in &files {
        db.scan(run, file, ScanOptions::default(), false)
            .await
            .unwrap();
    }
    let db = std::sync::Arc::new(db);
    let mut tasks = tokio::task::JoinSet::new();
    for file in files {
        let (db, library) = (db.clone(), library.clone());
        tasks.spawn(async move { db.attach(&library, &file).await });
    }
    while let Some(done) = tasks.join_next().await {
        assert!(done.unwrap().unwrap());
    }
    assert_eq!(
        series(&db, &library).await,
        [(None, "Kaiju No 8".to_owned(), 16)]
    );
}

/// Une base de la phase 1 garde ses données : les nouvelles colonnes sont vides.
#[tokio::test]
async fn phase_one_database_is_migrated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    sqlx::migrate!().run_to(1, &pool).await.unwrap();
    sqlx::query(
        "INSERT INTO volumes VALUES ('v', 1, 'cbz', 'ok', 0, 1, '2026-11-11T00:00:00.000Z');
         INSERT INTO file_locations VALUES ('/m/a.cbz', 'v', 1, 1, '2026-11-11T00:00:00.000Z');",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let db = Db::open(&path).await.unwrap();
    let row = sqlx::query("SELECT volume_id, library_id, series_id, kind FROM file_locations")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>("volume_id"), "v");
    assert_eq!(row.get::<Option<i64>, _>("library_id"), None);
    assert_eq!(row.get::<Option<i64>, _>("series_id"), None);
    assert_eq!(row.get::<Option<String>, _>("kind"), None);
}

/// Les séries d'avant la migration 0006 prennent la date de création de leur bibliothèque.
#[tokio::test]
async fn existing_series_get_their_library_date() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("t.db");
    let options = sqlx::sqlite::SqliteConnectOptions::new()
        .filename(&path)
        .create_if_missing(true);
    let pool = sqlx::SqlitePool::connect_with(options).await.unwrap();
    sqlx::migrate!().run_to(5, &pool).await.unwrap();
    sqlx::query(
        "INSERT INTO libraries (id, name, root, created_at) VALUES (1, 'L', '/l', '2026-01-02T00:00:00.000Z');
         INSERT INTO series (library_id, folder, name) VALUES (1, 'A', 'A');",
    )
    .execute(&pool)
    .await
    .unwrap();
    pool.close().await;

    let db = Db::open(&path).await.unwrap();
    let row = sqlx::query("SELECT created_at, publication_status FROM series")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(
        row.get::<String, _>("created_at"),
        "2026-01-02T00:00:00.000Z"
    );
    assert_eq!(row.get::<Option<String>, _>("publication_status"), None);
}

/// Passe complète sur `files` (déjà sur le disque), retrait des disparus et noms recalculés.
async fn full_pass(db: &Db, library: &Library, files: &[PathBuf]) {
    let run = scan_all(db, library, files).await;
    db.remove_missing(library, Path::new(&library.root), run)
        .await
        .unwrap();
    db.refresh_series(library).await.unwrap();
}

/// (id, dossier, statut de publication) par série, triés par dossier.
async fn identities(db: &Db, library: &Library) -> Vec<(i64, Option<String>, Option<String>)> {
    sqlx::query(
        "SELECT id, folder, publication_status FROM series WHERE library_id = ? ORDER BY folder",
    )
    .bind(library.id)
    .fetch_all(db.pool())
    .await
    .unwrap()
    .iter()
    .map(|r| (r.get("id"), r.get("folder"), r.get("publication_status")))
    .collect()
}

fn status(value: &str) -> crate::Edited {
    let patch = serde_json::json!({ "publication_status": value });
    patch.as_object().unwrap().clone()
}

fn rename(root: &Path, from: &str, to: &str) -> PathBuf {
    let to = root.join(to);
    std::fs::create_dir_all(to.parent().unwrap()).unwrap();
    std::fs::rename(root.join(from), &to).unwrap();
    to
}

#[tokio::test]
async fn a_renamed_folder_keeps_its_series_and_settings() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let files = [
        cbz(&root, "Blue Lock/Blue Lock T01.cbz", 1),
        cbz(&root, "Blue Lock/Blue Lock T02.cbz", 2),
    ];
    full_pass(&db, &library, &files).await;
    let [(id, ..)] = identities(&db, &library).await[..] else {
        panic!("une série attendue");
    };
    db.edit_series(id, status("completed")).await.unwrap();

    let moved = [
        rename(
            &root,
            "Blue Lock/Blue Lock T01.cbz",
            "Blue Lock (2018)/Blue Lock T01.cbz",
        ),
        rename(
            &root,
            "Blue Lock/Blue Lock T02.cbz",
            "Blue Lock (2018)/Blue Lock T02.cbz",
        ),
    ];
    full_pass(&db, &library, &moved).await;
    assert_eq!(
        identities(&db, &library).await,
        [(
            id,
            Some("Blue Lock (2018)".to_owned()),
            Some("completed".to_owned())
        )]
    );
    assert_eq!(
        series(&db, &library).await,
        [(
            Some("Blue Lock (2018)".to_owned()),
            "Blue Lock".to_owned(),
            2
        )]
    );
}

#[tokio::test]
async fn a_rename_with_a_volume_deleted_is_still_a_rename() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let files = [
        cbz(&root, "Gigant/Gigant 01.cbz", 1),
        cbz(&root, "Gigant/Gigant 02.cbz", 2),
    ];
    full_pass(&db, &library, &files).await;
    let id = identities(&db, &library).await[0].0;
    std::fs::remove_file(&files[1]).unwrap();
    let moved = rename(&root, "Gigant/Gigant 01.cbz", "GIGANT/Gigant 01.cbz");
    full_pass(&db, &library, &[moved]).await;
    assert_eq!(
        identities(&db, &library).await,
        [(id, Some("GIGANT".to_owned()), None)]
    );
}

#[tokio::test]
async fn partial_moves_splits_and_merges_are_not_renames() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let a = [
        cbz(&root, "A/A 01.cbz", 1),
        cbz(&root, "A/A 02.cbz", 2),
        cbz(&root, "A/A 03.cbz", 3),
        cbz(&root, "A/A 04.cbz", 4),
    ];
    let b = cbz(&root, "B/B 01.cbz", 5);
    let c = cbz(&root, "C/C 01.cbz", 6);
    let d = cbz(&root, "D/D 01.cbz", 7);
    full_pass(&db, &library, &[a.to_vec(), vec![b, c, d]].concat()).await;
    let before = identities(&db, &library).await;
    let id = |folder: &str| {
        before
            .iter()
            .find(|s| s.1.as_deref() == Some(folder))
            .unwrap()
            .0
    };
    let (id_a, id_b) = (id("A"), id("B"));

    // A : un tome reste, un autre part ailleurs (déplacement partiel).
    // B reçoit un tome de A (B existait : sa série reste la sienne).
    // C et D se réunissent dans un nouveau dossier E (fusion de deux séries).
    let files = [
        a[0].clone(),
        rename(&root, "A/A 02.cbz", "B/A 02.cbz"),
        root.join("B/B 01.cbz"),
        rename(&root, "C/C 01.cbz", "E/C 01.cbz"),
        rename(&root, "D/D 01.cbz", "E/D 01.cbz"),
        // Le reste de A éclaté dans deux nouveaux dossiers.
        rename(&root, "A/A 03.cbz", "F/A 03.cbz"),
        rename(&root, "A/A 04.cbz", "G/A 04.cbz"),
    ];
    full_pass(&db, &library, &files).await;
    let after = identities(&db, &library).await;
    let folders: Vec<_> = after.iter().map(|s| s.1.clone().unwrap()).collect();
    assert_eq!(folders, ["A", "B", "E", "F", "G"]);
    assert_eq!((after[0].0, after[1].0), (id_a, id_b));
    let old: Vec<i64> = before.iter().map(|s| s.0).collect();
    assert!(after[2..].iter().all(|s| !old.contains(&s.0)), "{after:?}");

    // Éclatement complet : H part vers I et J, aucune des deux ne prend sa place.
    let h = [cbz(&root, "H/H 01.cbz", 8), cbz(&root, "H/H 02.cbz", 9)];
    full_pass(&db, &library, &[files.to_vec(), h.to_vec()].concat()).await;
    let id_h = identities(&db, &library)
        .await
        .iter()
        .find(|s| s.1.as_deref() == Some("H"))
        .unwrap()
        .0;
    let split = [
        rename(&root, "H/H 01.cbz", "I/H 01.cbz"),
        rename(&root, "H/H 02.cbz", "J/H 02.cbz"),
    ];
    full_pass(&db, &library, &[files.to_vec(), split.to_vec()].concat()).await;
    assert!(identities(&db, &library).await.iter().all(|s| s.0 != id_h));

    // Fusion complète : tout K part dans B, qui existait ; B garde son id, K disparaît.
    let kept = [files.to_vec(), split.to_vec()].concat();
    let k = cbz(&root, "K/K 01.cbz", 10);
    full_pass(&db, &library, &[kept.clone(), vec![k]].concat()).await;
    let merged = rename(&root, "K/K 01.cbz", "B/K 01.cbz");
    full_pass(&db, &library, &[kept, vec![merged]].concat()).await;
    let after = identities(&db, &library).await;
    assert!(after.iter().all(|s| s.1.as_deref() != Some("K")));
    assert_eq!(
        after
            .iter()
            .find(|s| s.1.as_deref() == Some("B"))
            .unwrap()
            .0,
        id_b
    );
}

#[tokio::test]
async fn a_known_copy_keeps_its_identity_when_the_original_goes() {
    let (_dir, db, library) = setup().await;
    let root = PathBuf::from(&library.root);
    let original = cbz(&root, "One Piece/One Piece T01.cbz", 1);
    let copy = root.join("One Piece (copie)/One Piece T01.cbz");
    std::fs::create_dir_all(copy.parent().unwrap()).unwrap();
    std::fs::copy(&original, &copy).unwrap();
    full_pass(&db, &library, &[original.clone(), copy.clone()]).await;
    let id_copy = identities(&db, &library).await[1].0;
    db.edit_series(id_copy, status("hiatus")).await.unwrap();

    std::fs::remove_file(&original).unwrap();
    full_pass(&db, &library, &[copy]).await;
    assert_eq!(
        identities(&db, &library).await,
        [(
            id_copy,
            Some("One Piece (copie)".to_owned()),
            Some("hiatus".to_owned())
        )]
    );
}
