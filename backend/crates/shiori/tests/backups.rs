//! Sauvegardes de la base depuis l'administration : créer, télécharger, téléverser, restaurer.

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use http_body_util::BodyExt;
use serde_json::Value;
use shiori::cache::Images;
use shiori::server::App;
use shiori_db::{Db, Role};
use tower::ServiceExt;

async fn call(app: &Router, method: &str, uri: &str, body: Vec<u8>) -> (StatusCode, Vec<u8>) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .body(Body::from(body))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

async fn json(app: &Router, method: &str, uri: &str) -> (StatusCode, Value) {
    let (status, bytes) = call(app, method, uri, Vec::new()).await;
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn names(listing: &Value) -> Vec<String> {
    listing["files"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["name"].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
async fn back_up_download_upload_and_restore_from_the_interface() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let state = Arc::new(App::build(db, images, 1, 30, Vec::new()));
    let app = common::router(state.clone()).await;

    let (status, listing) = json(&app, "GET", "/api/admin/backups").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (listing["every_hours"].clone(), listing["keep"].clone()),
        (Value::Null, 7.into())
    );
    assert!(listing["dir"].as_str().unwrap().ends_with("backups"));
    assert_eq!(names(&listing), Vec::<String>::new());

    let (status, made) = json(&app, "POST", "/api/admin/backups").await;
    assert_eq!(status, StatusCode::CREATED);
    let name = made["name"].as_str().unwrap().to_owned();
    assert!(made["size"].as_u64().unwrap() > 0 && made["modified_ms"].as_u64().unwrap() > 0);
    state
        .db
        .create_user("bruno", "mot de passe", Role::Reader)
        .await
        .unwrap();

    let request = Request::get(format!("/api/admin/backups/{name}"))
        .body(Body::empty())
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        format!("attachment; filename=\"{name}\"").as_str()
    );
    let saved = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    assert!(saved.starts_with(b"SQLite format 3\0"));
    assert_eq!(saved.len() as u64, made["size"].as_u64().unwrap());

    // Téléversée : gardée sous un nouveau nom, à restaurer ensuite.
    let (status, bytes) = call(&app, "POST", "/api/admin/backups/upload", saved.clone()).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    let uploaded: Value = serde_json::from_slice(&bytes).unwrap();
    let uploaded = uploaded["name"].as_str().unwrap().to_owned();
    assert!(uploaded.starts_with("shiori-importee-"), "{uploaded}");
    assert_eq!(
        std::fs::read(dir.path().join("backups").join(&uploaded)).unwrap(),
        saved
    );
    let (status, error) = call(
        &app,
        "POST",
        "/api/admin/backups/upload",
        b"pas une base".to_vec(),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert!(String::from_utf8_lossy(&error).contains("invalid_backup"));
    let mut fake = b"SQLite format 3\0".to_vec();
    fake.extend([0; 100]);
    let (_, fake) = call(&app, "POST", "/api/admin/backups/upload", fake).await;
    let fake: Value = serde_json::from_slice(&fake).unwrap();
    let (status, error) = json(
        &app,
        "POST",
        &format!(
            "/api/admin/backups/{}/restore",
            fake["name"].as_str().unwrap()
        ),
    )
    .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_backup"))
    );
    assert!(state.db.user_by_name("bruno").await.unwrap().is_some());

    let (status, _) = json(
        &app,
        "POST",
        &format!("/api/admin/backups/{uploaded}/restore"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert!(state.db.user_by_name("bruno").await.unwrap().is_none());
    // Sessions fermées : il faut se reconnecter.
    let (status, _) = json(&app, "GET", "/api/admin/backups").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let app = common::router(state.clone()).await;
    let (_, listing) = json(&app, "GET", "/api/admin/backups").await;
    assert!(
        names(&listing)
            .iter()
            .any(|n| n.starts_with("shiori-avant-restauration-")),
        "{listing}"
    );

    let (status, _) = json(&app, "DELETE", &format!("/api/admin/backups/{uploaded}")).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for uri in [
        format!("/api/admin/backups/{uploaded}"),
        "/api/admin/backups/%2E%2E%2Ft.db".to_owned(),
    ] {
        let (status, error) = json(&app, "GET", &uri).await;
        assert_eq!(
            (status, error["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found")),
            "{uri}"
        );
    }
    let (status, _) = json(&app, "POST", "/api/admin/backups/absente.db/restore").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn no_restore_while_a_scan_writes() {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let state = Arc::new(App::build(db, images, 1, 30, Vec::new()));
    let app = common::router(state.clone()).await;
    let (_, made) = json(&app, "POST", "/api/admin/backups").await;
    let root = dir.path().join("manga");
    std::fs::create_dir_all(&root).unwrap();
    // 1 Gio creux à hacher : la passe dure le temps de prendre le verrou d'écriture.
    std::fs::File::create(root.join("gros.cbz"))
        .unwrap()
        .set_len(1 << 30)
        .unwrap();
    let library = state
        .db
        .create_library("Manga", &root, shiori_db::LibraryKind::Manga)
        .await
        .unwrap();
    let request = shiori::job::ScanRequest {
        library_id: library.id,
        path: None,
        deep: false,
        rehash: true,
    };
    state
        .scanner
        .start(state.db.clone(), request)
        .await
        .unwrap();
    let mut lock = state.db.pool().begin().await.unwrap();
    sqlx::query("UPDATE app_settings SET id = id")
        .execute(&mut *lock)
        .await
        .unwrap();

    let uri = format!(
        "/api/admin/backups/{}/restore",
        made["name"].as_str().unwrap()
    );
    let (status, error) = json(&app, "POST", &uri).await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::CONFLICT, Some("scan_already_running"))
    );
    lock.rollback().await.unwrap();
    state.scanner.wait().await;
}
