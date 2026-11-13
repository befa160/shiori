use shiori_core::ScanOptions;
use shiori_testkit::fixtures;
use sqlx::Row;

use super::*;

async fn db() -> (tempfile::TempDir, Db) {
    let dir = tempfile::tempdir().unwrap();
    let db = Db::open(&dir.path().join("t.db")).await.unwrap();
    (dir, db)
}

async fn run_row(db: &Db, run: i64) -> sqlx::sqlite::SqliteRow {
    sqlx::query("SELECT * FROM scan_runs WHERE id = ?")
        .bind(run)
        .fetch_one(db.pool())
        .await
        .unwrap()
}

async fn logs(db: &Db, run: i64) -> Vec<(String, Option<String>, String)> {
    sqlx::query("SELECT level, code, message FROM scan_run_logs WHERE scan_run_id = ? ORDER BY id")
        .bind(run)
        .fetch_all(db.pool())
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get("level"), r.get("code"), r.get("message")))
        .collect()
}

#[tokio::test]
async fn a_run_records_its_counts_and_its_log() {
    let (dir, db) = db().await;
    let run = db
        .begin_run(RunOrigin::Api, None, "/m", "{}")
        .await
        .unwrap();
    assert_eq!(
        run_row(&db, run).await.get::<String, _>("status"),
        "running"
    );
    db.set_run_total(run, 2).await.unwrap();

    let mut counts = RunCounts::default();
    let unpadded = fixtures::unpadded(dir.path());
    for path in [unpadded.clone(), fixtures::bad_crc(dir.path())] {
        let scanned = db
            .scan(run, &path, ScanOptions::default(), false)
            .await
            .unwrap();
        counts.add(&scanned);
        db.log_file(run, &scanned).await.unwrap();
    }
    // Rescan : inchangé, compté mais pas journalisé.
    let again = db
        .scan(run, &unpadded, ScanOptions::default(), false)
        .await
        .unwrap();
    counts.add(&again);
    db.log_file(run, &again).await.unwrap();
    counts.removed = 4;
    db.finish_run(run, RunStatus::Completed, &counts, None)
        .await
        .unwrap();

    assert_eq!(
        counts,
        RunCounts {
            ok: 0,
            warning: 2,
            error: 1,
            unchanged: 1,
            removed: 4
        }
    );
    let row = run_row(&db, run).await;
    assert_eq!(row.get::<String, _>("status"), "completed");
    assert_eq!(row.get::<String, _>("origin"), "api");
    assert_eq!(row.get::<i64, _>("files_total"), 2);
    assert_eq!(row.get::<i64, _>("files_warning"), 2);
    assert_eq!(row.get::<i64, _>("files_error"), 1);
    assert_eq!(row.get::<i64, _>("files_unchanged"), 1);
    assert_eq!(row.get::<i64, _>("files_removed"), 4);
    assert!(row.get::<Option<String>, _>("finished_at").is_some());

    let log = logs(&db, run).await;
    assert!(log[0].2.starts_with("scanné : warning"), "{log:?}");
    assert!(
        log.iter()
            .any(|l| l.1.as_deref() == Some("archive_order_differs"))
    );
    assert!(log.iter().any(|(level, code, msg)| level == "error"
        && code.as_deref() == Some("crc_mismatch")
        && msg.starts_with("02.png : ")));
    // 2 fichiers scannés, chacun une ligne + ses problèmes ; le fichier inchangé n'ajoute rien.
    assert_eq!(log.iter().filter(|l| l.1.is_none()).count(), 2);
}

#[tokio::test]
async fn cli_runs_failures_and_interruptions() {
    let (_dir, db) = db().await;
    let cli = db.start_run("/", "{}").await.unwrap();
    assert_eq!(run_row(&db, cli).await.get::<String, _>("origin"), "cli");
    let failed = db.start_run("/", "{}").await.unwrap();
    db.finish_run(
        failed,
        RunStatus::Failed,
        &RunCounts::default(),
        Some("base pleine"),
    )
    .await
    .unwrap();
    let api = db.begin_run(RunOrigin::Api, None, "/", "{}").await.unwrap();
    assert_eq!(db.interrupt_running_runs().await.unwrap(), 1);
    assert_eq!(
        run_row(&db, api).await.get::<String, _>("status"),
        "interrupted"
    );
    // Une CLI qui tourne encore n'est pas déclarée interrompue.
    assert_eq!(
        run_row(&db, cli).await.get::<String, _>("status"),
        "running"
    );
    let row = run_row(&db, failed).await;
    assert_eq!(row.get::<String, _>("status"), "failed");
    assert_eq!(
        row.get::<Option<String>, _>("error").as_deref(),
        Some("base pleine")
    );
}

/// Les vieilles passes partent, sauf celle d'où viennent les problèmes actuels d'un tome.
#[tokio::test]
async fn purge_keeps_runs_still_holding_current_issues() {
    let (dir, db) = db().await;
    let old_with_issues = db.start_run("/", "{}").await.unwrap();
    db.scan(
        old_with_issues,
        &fixtures::bad_crc(dir.path()),
        ScanOptions::default(),
        false,
    )
    .await
    .unwrap();
    let old_empty = db.start_run("/", "{}").await.unwrap();
    db.log(old_empty, Severity::Info, None, None, "rien")
        .await
        .unwrap();
    let recent = db.start_run("/", "{}").await.unwrap();
    for run in [old_with_issues, old_empty, recent] {
        db.finish_run(run, RunStatus::Completed, &RunCounts::default(), None)
            .await
            .unwrap();
    }
    sqlx::query("UPDATE scan_runs SET finished_at = '2020-01-01T00:00:00.000Z' WHERE id IN (?, ?)")
        .bind(old_with_issues)
        .bind(old_empty)
        .execute(db.pool())
        .await
        .unwrap();
    let running = db.start_run("/", "{}").await.unwrap();

    assert_eq!(db.purge_runs(30).await.unwrap(), 1);
    let left: Vec<i64> = sqlx::query("SELECT id FROM scan_runs ORDER BY id")
        .fetch_all(db.pool())
        .await
        .unwrap()
        .iter()
        .map(|r| r.get("id"))
        .collect();
    assert_eq!(left, [old_with_issues, recent, running]);
    assert!(
        logs(&db, old_empty).await.is_empty(),
        "journal supprimé avec sa passe"
    );
    let issues: i64 = sqlx::query("SELECT COUNT(*) AS n FROM scan_issues")
        .fetch_one(db.pool())
        .await
        .unwrap()
        .get("n");
    assert!(issues > 0);
}

/// Une ligne de journal mène au tome de son fichier (et à sa série quand il en a une) ; un
/// chemin qui n'est plus suivi n'y mène nulle part.
#[tokio::test]
async fn log_lines_point_to_their_volume() {
    let (dir, db) = db().await;
    let root = dir.path().join("lib");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let manga = fixtures::manga(&folder);
    let library = db
        .create_library("Lib", &root, crate::LibraryKind::Manga)
        .await
        .unwrap();
    let run = db
        .begin_run(RunOrigin::Api, Some(library.id), "/", "{}")
        .await
        .unwrap();
    let scanned = db
        .scan(run, &manga, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(&library, &manga).await.unwrap();
    db.log_file(run, &scanned).await.unwrap();
    db.log(
        run,
        Severity::Warning,
        Some("/nulle/part.cbz"),
        None,
        "disparu",
    )
    .await
    .unwrap();
    let lines = db.run_logs(run, Severity::Info, None, 50).await.unwrap();
    let series = db.series_of(library.id).await.unwrap().unwrap()[0].id;
    let volume = scanned.report.volume_id.unwrap().to_string();
    assert_eq!(
        (lines[0].series_id, lines[0].volume_id.as_ref()),
        (Some(series), Some(&volume))
    );
    let last = lines.last().unwrap();
    assert_eq!((last.series_id, last.volume_id.as_ref()), (None, None));
}
