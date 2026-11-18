//! API d'administration des scans : lancer, suivre, annuler, historique et journaux.

mod common;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::server::App;
use shiori_db::Db;
use shiori_testkit::{STORED, fixtures, page_color, png, write_zip};
use tower::ServiceExt;

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    state: Arc<App>,
    app: Router,
    library: i64,
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("manga");
        std::fs::create_dir_all(root.join("Série A")).unwrap();
        fixtures::unpadded(&root.join("Série A"));
        fixtures::bad_crc(&root);
        let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
        let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
        // Un fichier à la fois : un scan dure assez pour être suivi puis annulé.
        let state = Arc::new(App::build(db, images, 1, 30, Vec::new()));
        let app = common::router(state.clone()).await;
        let mut f = Self {
            _dir: dir,
            root,
            state,
            app,
            library: 0,
        };
        let body = json!({ "name": "Manga", "root": f.root }).to_string();
        let (status, created) = f.call("POST", "/api/libraries", Some(&body)).await;
        assert_eq!(status, StatusCode::CREATED);
        f.library = created["id"].as_i64().unwrap();
        f
    }

    async fn call(&self, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
        let request = Request::builder()
            .method(method)
            .uri(uri)
            .header("content-type", "application/json")
            .body(body.map_or_else(Body::empty, |b| Body::from(b.to_owned())))
            .unwrap();
        let response = self.app.clone().oneshot(request).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        (status, value)
    }

    async fn start(&self, body: Value) -> (StatusCode, Value) {
        self.call("POST", "/api/admin/scans", Some(&body.to_string()))
            .await
    }

    /// Lance un scan de la bibliothèque et attend sa fin ; renvoie le numéro de passe.
    async fn scan(&self) -> i64 {
        let (status, body) = self.start(json!({ "library_id": self.library })).await;
        assert_eq!(status, StatusCode::ACCEPTED, "{body}");
        self.state.scanner.wait().await;
        body["run"].as_i64().unwrap()
    }
}

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
async fn scan_then_read_history_detail_and_logs() {
    let f = Fixture::new().await;
    let run = f.scan().await;
    assert_eq!(
        f.call("GET", "/api/admin/scans/current", None).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );

    let (status, detail) = f
        .call("GET", &format!("/api/admin/scans/{run}"), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["status"], "completed");
    assert_eq!(detail["origin"], "api");
    assert_eq!(detail["library_id"], f.library);
    assert_eq!(detail["files_total"], 2);
    assert_eq!(
        detail["counts"],
        json!({ "ok": 0, "warning": 1, "error": 1, "unchanged": 0, "removed": 0 })
    );
    assert_eq!(
        detail["options"],
        json!({ "deep": false, "rehash": false, "trigger": "manual" })
    );
    assert!(detail["duration_ms"].as_i64().unwrap() >= 0);

    // La série est consultable aussitôt.
    let (_, series) = f
        .call("GET", &format!("/api/libraries/{}/series", f.library), None)
        .await;
    assert_eq!(series.as_array().unwrap().len(), 2);

    let logs_uri = format!("/api/admin/scans/{run}/logs");
    let (status, all) = f.call("GET", &logs_uri, None).await;
    assert_eq!(status, StatusCode::OK);
    let all = all.as_array().unwrap().clone();
    assert!(all.len() >= 4, "{all:?}");
    let (_, errors) = f
        .call("GET", &format!("{logs_uri}?level=error"), None)
        .await;
    let errors = errors.as_array().unwrap();
    assert!(!errors.is_empty());
    assert!(errors.iter().all(|l| l["level"] == "error"));
    assert!(errors.iter().any(|l| l["code"] == "crc_mismatch"));
    let (_, warnings) = f
        .call("GET", &format!("{logs_uri}?level=warning"), None)
        .await;
    assert!(
        warnings
            .as_array()
            .unwrap()
            .iter()
            .all(|l| l["level"] != "info")
    );

    // Pagination du journal.
    let (_, first) = f.call("GET", &format!("{logs_uri}?limit=2"), None).await;
    let last = first[1]["id"].as_i64().unwrap();
    let (_, next) = f
        .call("GET", &format!("{logs_uri}?limit=2&after={last}"), None)
        .await;
    assert_eq!(next[0], all[2]);

    // Historique : du plus récent au plus ancien, filtres et pagination.
    let second = f.scan().await;
    let (_, history) = f.call("GET", "/api/admin/scans", None).await;
    let ids: Vec<i64> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_i64().unwrap())
        .collect();
    assert_eq!(ids, [second, run]);
    let (_, older) = f
        .call(
            "GET",
            &format!("/api/admin/scans?before={second}&limit=1"),
            None,
        )
        .await;
    assert_eq!(older.as_array().unwrap().len(), 1);
    assert_eq!(older[0]["id"], run);
    let (_, filtered) = f
        .call(
            "GET",
            &format!("/api/admin/scans?status=completed&library_id={}", f.library),
            None,
        )
        .await;
    assert_eq!(filtered.as_array().unwrap().len(), 2);
    let (_, none) = f.call("GET", "/api/admin/scans?status=failed", None).await;
    assert_eq!(none, json!([]));
    // Le second passage n'a rien rescanné.
    let (_, detail) = f
        .call("GET", &format!("/api/admin/scans/{second}"), None)
        .await;
    assert_eq!(detail["counts"]["unchanged"], 2);
}

#[tokio::test]
async fn follow_and_cancel_a_running_scan() {
    let f = Fixture::new().await;
    many(&f.root, 12);
    // Premier fichier du tri : 1 Gio creux à hacher avant toute écriture, le test a le temps de
    // prendre le verrou (12 petits tomes se scannaient parfois avant, sous Windows).
    std::fs::File::create(f.root.join("0 gros.cbz"))
        .unwrap()
        .set_len(1 << 30)
        .unwrap();
    let (status, started) = f
        .start(json!({ "library_id": f.library, "rehash": true }))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let run = started["run"].as_i64().unwrap();
    // Verrou d'écriture tenu par le test : la passe attend pour enregistrer son premier fichier
    // et ne peut pas finir avant l'annulation (suivre et annuler ne font que lire).
    let mut lock = f.state.db.pool().begin().await.unwrap();
    sqlx::query("UPDATE app_settings SET id = id")
        .execute(&mut *lock)
        .await
        .unwrap();

    let (status, progress) = f.call("GET", "/api/admin/scans/current", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(progress["run"], run);
    assert_eq!(progress["library_id"], f.library);
    let (status, error) = f.start(json!({ "library_id": f.library })).await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::CONFLICT, Some("scan_already_running"))
    );

    let (status, body) = f
        .call("POST", "/api/admin/scans/current/cancel", None)
        .await;
    assert_eq!(
        (status, body),
        (StatusCode::ACCEPTED, json!({ "cancelling": true }))
    );
    lock.rollback().await.unwrap();
    f.state.scanner.wait().await;
    let (_, detail) = f
        .call("GET", &format!("/api/admin/scans/{run}"), None)
        .await;
    assert_eq!(detail["status"], "cancelled");
    let (status, error) = f
        .call("POST", "/api/admin/scans/current/cancel", None)
        .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("no_scan_running"))
    );
}

#[tokio::test]
async fn scan_a_folder_of_the_library() {
    let f = Fixture::new().await;
    let folder = f.root.join("Série A");
    let (status, body) = f
        .start(json!({ "library_id": f.library, "path": folder, "deep": true }))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    f.state.scanner.wait().await;
    let (_, detail) = f
        .call("GET", &format!("/api/admin/scans/{}", body["run"]), None)
        .await;
    assert_eq!(detail["files_total"], 1);
    assert_eq!(detail["root"], folder.display().to_string());
    assert_eq!(detail["options"]["deep"], true);
}

#[tokio::test]
async fn errors_have_stable_codes() {
    let f = Fixture::new().await;
    let outside = f.root.parent().unwrap().to_path_buf();
    let starts = [
        (
            json!({ "library_id": f.library, "path": outside }),
            StatusCode::BAD_REQUEST,
            "path_outside_library",
        ),
        (
            json!({ "library_id": f.library, "path": f.root.join("absent") }),
            StatusCode::NOT_FOUND,
            "path_not_found",
        ),
        (
            json!({ "library_id": 999 }),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            json!({ "path": "/" }),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
    ];
    for (body, status, code) in starts {
        let (got, error) = f.start(body.clone()).await;
        assert_eq!(
            (got, error["code"].as_str()),
            (status, Some(code)),
            "{body}"
        );
    }
    for (uri, status, code) in [
        ("/api/admin/scans/999", StatusCode::NOT_FOUND, "not_found"),
        (
            "/api/admin/scans/abc",
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "/api/admin/scans/999/logs",
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "/api/admin/scans/abc/logs",
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "/api/admin/scans/1/logs?level=fatal",
            StatusCode::BAD_REQUEST,
            "invalid_query",
        ),
        (
            "/api/admin/scans?status=perdu",
            StatusCode::BAD_REQUEST,
            "invalid_query",
        ),
        (
            "/api/admin/scans?limit=-1",
            StatusCode::BAD_REQUEST,
            "invalid_query",
        ),
    ] {
        let (got, error) = f.call("GET", uri, None).await;
        assert_eq!((got, error["code"].as_str()), (status, Some(code)), "{uri}");
    }
}

/// Une panne de la base au lancement : 500 générique, sans détail.
#[tokio::test]
async fn database_failure_when_starting() {
    let f = Fixture::new().await;
    f.state.db.pool().close().await;
    let (status, error) = f.start(json!({ "library_id": f.library })).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        error,
        json!({ "code": "internal", "message": "erreur interne" })
    );
}

/// Santé : états des tomes, fichiers à réparer filtrés, date de la vérification complète posée
/// par une passe `deep` de toute la bibliothèque, pas par celle d'un dossier.
#[tokio::test]
async fn health_shows_what_to_repair_and_the_last_deep_check() {
    let f = Fixture::new().await;
    f.scan().await;
    let (status, health) = f.call("GET", "/api/admin/health", None).await;
    assert_eq!(status, StatusCode::OK);
    let library = &health["libraries"][0];
    assert_eq!(library["error"], 1, "{health}");
    assert_eq!(library["deep_checked_at"], Value::Null);
    let code = health["codes"][0]["code"].as_str().unwrap().to_owned();
    assert_eq!(health["codes"][0]["severity"], "error");

    let (_, files) = f.call("GET", "/api/admin/health/files", None).await;
    let bad = files
        .as_array()
        .unwrap()
        .iter()
        .find(|f| f["status"] == "error")
        .expect("le tome en erreur");
    assert!(bad["path"].as_str().unwrap().ends_with("bad-crc.cbz"));
    assert!(bad["series_id"].is_i64() && bad["volume_id"].is_string());
    let uri = format!(
        "/api/admin/health/files?library_id={}&code={code}&limit=1",
        f.library
    );
    let (_, filtered) = f.call("GET", &uri, None).await;
    assert_eq!(filtered.as_array().unwrap().len(), 1);
    let (status, error) = f
        .call("GET", "/api/admin/health/files?code=inconnu", None)
        .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_query"))
    );

    // Un dossier vérifié en profondeur ne suffit pas.
    let folder = json!({ "library_id": f.library, "path": f.root.join("Série A"), "deep": true });
    assert_eq!(f.start(folder).await.0, StatusCode::ACCEPTED);
    f.state.scanner.wait().await;
    let (_, health) = f.call("GET", "/api/admin/health", None).await;
    assert_eq!(health["libraries"][0]["deep_checked_at"], Value::Null);
    let all = json!({ "library_id": f.library, "deep": true });
    assert_eq!(f.start(all).await.0, StatusCode::ACCEPTED);
    f.state.scanner.wait().await;
    let (_, health) = f.call("GET", "/api/admin/health", None).await;
    assert!(
        health["libraries"][0]["deep_checked_at"].is_string(),
        "{health}"
    );
}
