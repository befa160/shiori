//! API HTTP de bout en bout : routeur en mémoire, vraies fixtures scannées.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::server::{self, ServeArgs};
use shiori_core::ScanOptions;
use shiori_db::Db;
use shiori_testkit::fixtures;
use tower::ServiceExt;

async fn call(app: &Router, method: &str, uri: &str, body: Option<&str>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_owned())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

async fn get(app: &Router, uri: &str) -> (StatusCode, Value) {
    call(app, "GET", uri, None).await
}

struct Fixture {
    _dir: tempfile::TempDir,
    root: PathBuf,
    db: Arc<Db>,
    app: Router,
}

/// Bibliothèque : une série-dossier de deux tomes, un tome cassé posé à la racine.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("manga");
    let series = root.join("Série A [Digital]");
    std::fs::create_dir_all(&series).unwrap();
    let manga = fixtures::manga(&series);
    std::fs::rename(&manga, series.join("Série A T02.cbz")).unwrap();
    let unpadded = fixtures::unpadded(&series);
    std::fs::rename(&unpadded, series.join("Série A T01.cbz")).unwrap();
    fixtures::bad_crc(&root);
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let app = server::router(db.clone(), false);
    Fixture {
        _dir: dir,
        root,
        db,
        app,
    }
}

/// Ce que fera le job de scan (lot 2.6) : scanner, rattacher, recalculer les séries.
async fn scan_library(f: &Fixture, library_id: i64) {
    let library =
        f.db.libraries()
            .await
            .unwrap()
            .into_iter()
            .find(|l| l.id == library_id)
            .unwrap();
    let run = f.db.start_run(&library.root, "{}").await.unwrap();
    for path in cbz_files(&f.root) {
        f.db.scan(run, &path, ScanOptions::default(), false)
            .await
            .unwrap();
        f.db.attach(&library, &path).await.unwrap();
    }
    f.db.refresh_series(&library).await.unwrap();
}

fn cbz_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            out.extend(cbz_files(&path));
        } else {
            out.push(path);
        }
    }
    out.sort();
    out
}

async fn create(f: &Fixture) -> i64 {
    let body = json!({ "name": "Manga", "root": f.root }).to_string();
    let (status, created) = call(&f.app, "POST", "/api/libraries", Some(&body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["name"], "Manga");
    created["id"].as_i64().unwrap()
}

#[tokio::test]
async fn health() {
    let f = fixture().await;
    let (status, body) = get(&f.app, "/api/health").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["status"], "ok");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
}

#[tokio::test]
async fn browse_libraries_series_volumes_and_pages() {
    let f = fixture().await;
    let id = create(&f).await;
    scan_library(&f, id).await;

    let (status, libraries) = get(&f.app, "/api/libraries").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(libraries[0]["series_count"], 2);
    assert_eq!(libraries[0]["volume_count"], 3);

    let (_, series) = get(&f.app, &format!("/api/libraries/{id}/series")).await;
    let names: Vec<&str> = series
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["bad-crc", "Série A"]);
    let serie_a = &series[1];
    assert_eq!(serie_a["volume_count"], 2);

    let (status, detail) = get(&f.app, &format!("/api/series/{}", serie_a["id"])).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(detail["folder"], "Série A [Digital]");
    let files: Vec<&str> = detail["volumes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v["file_name"].as_str().unwrap())
        .collect();
    // Ordre de la série (numéro), pas ordre alphabétique ni ordre du scan.
    assert_eq!(files, ["Série A T01.cbz", "Série A T02.cbz"]);
    let first = &detail["volumes"][0];
    assert_eq!(first["kind"], "volume");
    assert_eq!(first["number"], 1.0);
    // La couverture de la série est la page 0 de son premier tome.
    assert_eq!(serie_a["cover_page_id"], first["cover_page_id"]);

    let volume = first["volume_id"].as_str().unwrap();
    let (status, v) = get(&f.app, &format!("/api/volumes/{volume}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["format"], "cbz");
    assert_eq!(v["locations"][0]["series_id"], serie_a["id"]);
    assert_eq!(v["issues"][0]["code"], "archive_order_differs");

    let (status, pages) = get(&f.app, &format!("/api/volumes/{volume}/pages")).await;
    assert_eq!(status, StatusCode::OK);
    let pages = pages.as_array().unwrap();
    assert_eq!(pages.len() as i64, first["page_count"].as_i64().unwrap());
    for (n, page) in pages.iter().enumerate() {
        assert_eq!(page["ordinal"], n);
        assert_eq!(page["id"].as_str().unwrap().len(), 32);
    }
    assert_eq!(pages[0]["id"], first["cover_page_id"]);

    // Le tome cassé de la racine est visible, en erreur (principe 4).
    let (_, broken) = get(&f.app, &format!("/api/series/{}", series[0]["id"])).await;
    assert_eq!(broken["folder"], Value::Null);
    assert_eq!(broken["volumes"][0]["status"], "error");
}

#[tokio::test]
async fn library_creation_errors() {
    let f = fixture().await;
    create(&f).await;
    let post = |body: Value| {
        let app = f.app.clone();
        async move { call(&app, "POST", "/api/libraries", Some(&body.to_string())).await }
    };
    let cases = [
        (
            json!({ "name": " ", "root": f.root }),
            StatusCode::BAD_REQUEST,
            "invalid_name",
        ),
        (
            json!({ "name": "Manga", "root": f.root.join("Série A [Digital]") }),
            StatusCode::CONFLICT,
            "name_taken",
        ),
        (
            json!({ "name": "B", "root": f.root.join("absent") }),
            StatusCode::BAD_REQUEST,
            "invalid_root",
        ),
        (
            json!({ "name": "B", "root": "manga" }),
            StatusCode::BAD_REQUEST,
            "invalid_root",
        ),
        (
            json!({ "name": "C", "root": f.root.join("Série A [Digital]") }),
            StatusCode::CONFLICT,
            "root_overlaps",
        ),
        (
            json!({ "name": "D" }),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
    ];
    for (body, status, code) in cases {
        let (got, error) = post(body.clone()).await;
        assert_eq!(
            (got, error["code"].as_str().unwrap()),
            (status, code),
            "{body}"
        );
        assert!(!error["message"].as_str().unwrap().is_empty());
    }
    let (status, error) = call(&f.app, "POST", "/api/libraries", Some("{pas du json")).await;
    assert_eq!(
        (status, error["code"].as_str().unwrap()),
        (StatusCode::BAD_REQUEST, "invalid_body")
    );
}

#[tokio::test]
async fn delete_keeps_files_and_volumes() {
    let f = fixture().await;
    let id = create(&f).await;
    scan_library(&f, id).await;
    let (_, series) = get(&f.app, &format!("/api/libraries/{id}/series")).await;
    let (_, detail) = get(&f.app, &format!("/api/series/{}", series[1]["id"])).await;
    let volume = detail["volumes"][0]["volume_id"]
        .as_str()
        .unwrap()
        .to_owned();

    let (status, body) = call(&f.app, "DELETE", &format!("/api/libraries/{id}"), None).await;
    assert_eq!((status, body), (StatusCode::NO_CONTENT, Value::Null));
    let (status, _) = call(&f.app, "DELETE", &format!("/api/libraries/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Le tome reste connu, ses pages aussi ; les fichiers sont intacts.
    let (status, v) = get(&f.app, &format!("/api/volumes/{volume}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(v["locations"], json!([]));
    assert_eq!(
        get(&f.app, &format!("/api/volumes/{volume}/pages")).await.0,
        StatusCode::OK
    );
    assert_eq!(cbz_files(&f.root).len(), 3);
}

#[tokio::test]
async fn errors_are_json_with_stable_codes() {
    let f = fixture().await;
    let unknown = "0".repeat(64);
    let cases = [
        (
            "GET",
            "/api/nope".to_owned(),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "PUT",
            "/api/health".to_owned(),
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
        ),
        (
            "GET",
            "/api/libraries/abc/series".to_owned(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "GET",
            "/api/libraries/99/series".to_owned(),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "DELETE",
            "/api/libraries/x".to_owned(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "GET",
            "/api/series/99".to_owned(),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "GET",
            "/api/series/-".to_owned(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "GET",
            "/api/volumes/xyz".to_owned(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
        (
            "GET",
            format!("/api/volumes/{unknown}"),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "GET",
            format!("/api/volumes/{unknown}/pages"),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "GET",
            "/api/volumes/xyz/pages".to_owned(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
    ];
    for (method, uri, status, code) in cases {
        let (got, error) = call(&f.app, method, &uri, None).await;
        assert_eq!(
            (got, error["code"].as_str()),
            (status, Some(code)),
            "{method} {uri}"
        );
    }
}

/// Une panne de la base devient une erreur 500 générique, sans détail interne.
#[tokio::test]
async fn database_failures_are_internal_errors() {
    let f = fixture().await;
    let id = create(&f).await;
    f.db.pool().close().await;
    let body = json!({ "name": "Autre", "root": f.root }).to_string();
    for (method, uri, body) in [
        ("GET", "/api/libraries".to_owned(), None),
        ("GET", format!("/api/libraries/{id}/series"), None),
        ("POST", "/api/libraries".to_owned(), Some(body.as_str())),
    ] {
        let (status, error) = call(&f.app, method, &uri, body).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR, "{method} {uri}");
        assert_eq!(
            error,
            json!({ "code": "internal", "message": "erreur interne" })
        );
    }
}

/// En écoute locale, un `Host` étranger (DNS rebinding) est refusé ; les noms locaux passent.
#[tokio::test]
async fn local_server_rejects_foreign_hosts() {
    let f = fixture().await;
    let app = server::router(f.db.clone(), true);
    let status = |host: &'static str| {
        let app = app.clone();
        async move {
            let request = Request::get("/api/health").header("host", host);
            let response = app.oneshot(request.body(Body::empty()).unwrap()).await;
            response.unwrap().status()
        }
    };
    for ok in [
        "localhost",
        "localhost:7878",
        "127.0.0.1:7878",
        "[::1]:7878",
        "[::1]",
    ] {
        assert_eq!(status(ok).await, StatusCode::OK, "{ok}");
    }
    for bad in [
        "evil.example",
        "evil.example:7878",
        "localhost.evil.example",
        "localhost:",
        "127.0.0.1:x",
    ] {
        assert_eq!(status(bad).await, StatusCode::FORBIDDEN, "{bad}");
    }
    // Sans en-tête `Host` du tout.
    let (code, error) = call(&app, "GET", "/api/health", None).await;
    assert_eq!(
        (code, error["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("invalid_host"))
    );
}

/// Le vrai serveur TCP répond, puis s'arrête proprement sur demande.
#[tokio::test]
async fn serves_over_tcp_and_shuts_down() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let f = fixture().await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let task = tokio::spawn(server::serve(listener, f.db.clone(), async {
        let _ = stopped.await;
    }));

    let mut stream = tokio::net::TcpStream::connect(addr).await.unwrap();
    stream
        .write_all(b"GET /api/health HTTP/1.1\r\nHost: localhost:7878\r\nConnection: close\r\n\r\n")
        .await
        .unwrap();
    let mut response = String::new();
    stream.read_to_string(&mut response).await.unwrap();
    assert!(response.starts_with("HTTP/1.1 200 OK"), "{response}");
    assert!(response.contains(r#""status":"ok""#), "{response}");

    stop.send(()).unwrap();
    task.await.unwrap().unwrap();
}

#[tokio::test]
async fn run_opens_the_database_and_reports_bind_errors() {
    let dir = tempfile::tempdir().unwrap();
    let args = |listen: &str| ServeArgs {
        listen: listen.parse().unwrap(),
        db: Some(dir.path().join("sous-dossier/shiori.db")),
    };
    // Arrêt immédiat : la base est créée (avec son dossier) puis le serveur s'arrête.
    server::run(args("127.0.0.1:0"), false, async {})
        .await
        .unwrap();
    assert!(dir.path().join("sous-dossier/shiori.db").exists());

    let taken = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let addr = taken.local_addr().unwrap().to_string();
    let err = server::run(args(&addr), true, async {}).await.unwrap_err();
    assert!(format!("{err:#}").contains(&addr), "{err:#}");
}
