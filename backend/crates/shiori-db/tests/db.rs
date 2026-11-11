//! Persistance des scans : détection de changement, rescan idempotent, identité stable.

use std::path::Path;

use shiori_core::{IssueCode, PageStatus, ScanOptions, Status};
use shiori_db::{Db, Scanned};
use shiori_testkit::{DEFLATED, fixtures, png, write_zip};
use sqlx::Row;

const NORMAL: ScanOptions = ScanOptions { deep: false };

async fn open(dir: &Path) -> (Db, i64) {
    let db = Db::open(&dir.join("shiori.db")).await.unwrap();
    let run = db.start_run("/bibliotheque", "{}").await.unwrap();
    (db, run)
}

async fn count(db: &Db, sql: &'static str) -> i64 {
    sqlx::query(sql).fetch_one(db.pool()).await.unwrap().get(0)
}

/// Contenu complet de la table des pages, pour vérifier qu'un rescan n'y touche pas.
async fn pages_dump(db: &Db) -> Vec<(i64, String, i64, String)> {
    sqlx::query("SELECT rowid, id, ordinal, status FROM pages ORDER BY rowid")
        .fetch_all(db.pool())
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1), r.get(2), r.get(3)))
        .collect()
}

async fn scan(db: &Db, run: i64, path: &Path) -> Scanned {
    db.scan(run, path, NORMAL, false).await.unwrap()
}

#[tokio::test]
async fn first_scan_stores_volume_pages_location_and_issues() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::unpadded(dir.path());

    let s = scan(&db, run, &path).await;
    assert!(!s.skipped);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM volumes").await, 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM pages").await, 11);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM file_locations").await, 1);
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM scan_issues WHERE code = 'archive_order_differs'"
        )
        .await,
        1
    );
    let row = sqlx::query("SELECT status, page_count, scanner_version FROM volumes")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert_eq!(row.get::<String, _>(0), "warning");
    assert_eq!(row.get::<i64, _>(1), 11);
    assert_eq!(row.get::<i64, _>(2), shiori_core::SCANNER_VERSION);
    db.finish_run(run).await.unwrap();
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM scan_runs WHERE finished_at IS NOT NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn unchanged_file_is_not_rescanned_and_reports_the_same() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::manga(dir.path());

    let first = scan(&db, run, &path).await;
    let before = pages_dump(&db).await;
    let second = scan(&db, run, &path).await;
    assert!(
        second.skipped,
        "un fichier inchangé ne doit être ni rehashé ni rescanné"
    );
    assert_eq!(second.report, first.report);
    assert_eq!(pages_dump(&db).await, before);
}

#[tokio::test]
async fn rehash_rescans_but_keeps_rows_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::manga(dir.path());

    scan(&db, run, &path).await;
    let before = pages_dump(&db).await;
    let again = db.scan(run, &path, NORMAL, true).await.unwrap();
    assert!(!again.skipped);
    assert_eq!(pages_dump(&db).await, before, "mêmes lignes, mêmes rowid");
}

#[tokio::test]
async fn moved_copy_reuses_the_volume() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::manga(dir.path());
    let copy = dir.path().join("Manga T01 (copie).cbz");
    std::fs::copy(&path, &copy).unwrap();

    let a = scan(&db, run, &path).await;
    let b = scan(&db, run, &copy).await;
    assert!(!b.skipped, "chemin inconnu : il faut hasher");
    assert_eq!(a.report.volume_id, b.report.volume_id);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM volumes").await, 1);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM file_locations").await, 2);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM pages").await, 12);
}

#[tokio::test]
async fn modified_file_gets_a_new_volume() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let one = png(8, 8, [1, 1, 1]);
    let path = write_zip(dir.path(), "t.cbz", &[("001.png", &one, DEFLATED)]);
    let a = scan(&db, run, &path).await;

    let two = png(8, 9, [2, 2, 2]);
    write_zip(
        dir.path(),
        "t.cbz",
        &[("001.png", &two, DEFLATED), ("002.png", &one, DEFLATED)],
    );
    let b = scan(&db, run, &path).await;
    assert!(!b.skipped);
    assert_ne!(a.report.volume_id, b.report.volume_id);
    // L'ancien tome reste (sa progression future ne doit pas disparaître) ; le chemin
    // pointe vers le nouveau.
    assert_eq!(count(&db, "SELECT COUNT(*) FROM volumes").await, 2);
    let located: String = sqlx::query("SELECT volume_id FROM file_locations")
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get(0);
    assert_eq!(located, b.report.volume_id.unwrap().to_string());
}

#[tokio::test]
async fn scanner_version_change_forces_a_rescan() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::manga(dir.path());
    scan(&db, run, &path).await;
    sqlx::query("UPDATE volumes SET scanner_version = 0")
        .execute(db.pool())
        .await
        .unwrap();
    assert!(!scan(&db, run, &path).await.skipped);
    assert!(scan(&db, run, &path).await.skipped);
}

/// Un rescan ne supprime jamais une page encore présente : une table qui la référence
/// (la progression de lecture, plus tard) garde ses lignes.
#[tokio::test]
async fn rescans_never_cascade_onto_referencing_rows() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::corrupt_body(dir.path());
    let first = scan(&db, run, &path).await;
    assert_eq!(first.report.status, Status::Ok);

    sqlx::query(
        "CREATE TABLE progress (page_id TEXT NOT NULL REFERENCES pages(id) ON DELETE CASCADE)",
    )
    .execute(db.pool())
    .await
    .unwrap();
    let page = first.report.pages[0].id.to_string();
    sqlx::query("INSERT INTO progress VALUES (?)")
        .bind(&page)
        .execute(db.pool())
        .await
        .unwrap();

    // Le scan approfondi découvre que la page 1 est illisible : statut mis à jour sur place.
    let deep = db
        .scan(run, &path, ScanOptions { deep: true }, true)
        .await
        .unwrap();
    assert_eq!(deep.report.status, Status::Error);
    let status: String = sqlx::query("SELECT status FROM pages WHERE id = ?")
        .bind(&page)
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get(0);
    assert_eq!(status, "broken");
    assert_eq!(count(&db, "SELECT COUNT(*) FROM progress").await, 1);
}

#[tokio::test]
async fn pages_gone_from_the_archive_are_removed() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::manga(dir.path());
    let s = scan(&db, run, &path).await;
    let volume = s.report.volume_id.unwrap().to_string();
    // Page fantôme d'une version précédente des règles de scan.
    sqlx::query(
        "INSERT INTO pages (id, volume_id, ordinal, entry_name, entry_name_raw, archive_index,
                            compressed_size, size, crc32, status)
         VALUES ('00000000000000000000000000000000', ?, 99, 'fantome.png', x'00', 0, 0, 0, 0, 'ok')",
    )
    .bind(&volume)
    .execute(db.pool())
    .await
    .unwrap();
    db.scan(run, &path, NORMAL, true).await.unwrap();
    assert_eq!(count(&db, "SELECT COUNT(*) FROM pages").await, 12);
}

#[tokio::test]
async fn broken_archives_are_remembered() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::not_a_zip(dir.path());
    let a = scan(&db, run, &path).await;
    assert_eq!(a.report.status, Status::Error);
    let b = scan(&db, run, &path).await;
    assert!(b.skipped, "un fichier cassé et inchangé n'est pas rescanné");
    assert_eq!(b.report.issues[0].code, IssueCode::ArchiveUnreadable);
    assert_eq!(b.report.status, Status::Error);
}

#[tokio::test]
async fn missing_files_leave_no_volume_and_are_retried() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = dir.path().join("absent.cbz");
    for _ in 0..2 {
        let s = scan(&db, run, &path).await;
        assert!(!s.skipped);
        assert_eq!(s.report.issues[0].code, IssueCode::ArchiveUnreadable);
    }
    assert_eq!(count(&db, "SELECT COUNT(*) FROM volumes").await, 0);
    assert_eq!(count(&db, "SELECT COUNT(*) FROM file_locations").await, 0);
    // Pas de doublon d'erreur d'un scan à l'autre pour un même chemin.
    assert_eq!(
        count(
            &db,
            "SELECT COUNT(*) FROM scan_issues WHERE volume_id IS NULL"
        )
        .await,
        1
    );
}

#[tokio::test]
async fn broken_pages_survive_a_reload() {
    let dir = tempfile::tempdir().unwrap();
    let (db, run) = open(dir.path()).await;
    let path = fixtures::bad_crc(dir.path());
    scan(&db, run, &path).await;
    let reloaded = scan(&db, run, &path).await;
    assert!(reloaded.skipped);
    assert_eq!(reloaded.report.pages[1].status, PageStatus::Broken);
    assert_eq!(reloaded.report.issues[0].code, IssueCode::CrcMismatch);
}

#[tokio::test]
async fn reopening_an_existing_database_keeps_data() {
    let dir = tempfile::tempdir().unwrap();
    let path = fixtures::manga(dir.path());
    {
        let (db, run) = open(dir.path()).await;
        scan(&db, run, &path).await;
    }
    let (db, run) = open(dir.path()).await;
    assert!(scan(&db, run, &path).await.skipped);
}

#[tokio::test]
async fn unknown_volume_has_no_report() {
    let dir = tempfile::tempdir().unwrap();
    let (db, _) = open(dir.path()).await;
    let volume = shiori_core::VolumeId::from_reader(&b"inconnu"[..]).unwrap();
    assert!(
        db.load_report(&volume, Path::new("x.cbz"))
            .await
            .unwrap()
            .is_none()
    );
}
