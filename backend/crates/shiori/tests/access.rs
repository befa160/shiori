//! Rôles, accès par bibliothèque, compte connecté et gestion des comptes (lot 7.4b).

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::server::App;
use shiori_core::ScanOptions;
use shiori_db::{Db, LibraryKind, Role};
use shiori_testkit::fixtures;
use tower::ServiceExt;

struct Fixture {
    dir: tempfile::TempDir,
    db: Arc<Db>,
    /// Requêtes de l'administrateur (`tests/common`), sauf cookie donné.
    app: Router,
    library: i64,
    series: i64,
    volume: String,
    page: String,
}

/// Une bibliothèque, une série d'un tome scanné.
async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("manga");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let manga = fixtures::manga(&folder);
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let app = common::router(Arc::new(App::new(db.clone(), images))).await;
    let library = db
        .create_library("Manga", &root, LibraryKind::Manga)
        .await
        .unwrap();
    let run = db.start_run("/", "{}").await.unwrap();
    let scanned = db
        .scan(run, &manga, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(&library, &manga).await.unwrap();
    db.refresh_series(&library).await.unwrap();
    let series: i64 = sqlx::query_scalar("SELECT id FROM series")
        .fetch_one(db.pool())
        .await
        .unwrap();
    Fixture {
        dir,
        db,
        app,
        library: library.id,
        series,
        volume: scanned.report.volume_id.unwrap().to_string(),
        page: scanned.report.pages[0].id.to_string(),
    }
}

/// Requête avec ce cookie (`None` : celui de l'administrateur des tests).
async fn call(
    f: &Fixture,
    cookie: Option<&str>,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let mut request = Request::builder().method(method).uri(uri);
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if body.is_some() {
        request = request.header("content-type", "application/json");
    }
    let body = body.map_or_else(Body::empty, |b| Body::from(b.to_string()));
    let response = f
        .app
        .clone()
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => Value::Null,
    };
    (status, value)
}

/// Lecteur au mot de passe définitif, et le cookie d'une session à lui.
async fn reader(f: &Fixture, name: &str) -> (i64, String) {
    let user =
        f.db.create_user(name, "mot de passe", Role::Reader)
            .await
            .unwrap();
    f.db.set_password(name, "mot de passe", false, None)
        .await
        .unwrap();
    let session = f.db.create_session(user.id, "tests").await.unwrap();
    (user.id, format!("shiori_session={}", session.token))
}

async fn login(f: &Fixture, name: &str, password: &str) -> String {
    let request = Request::post("/api/auth/login")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "username": name, "password": password }).to_string(),
        ))
        .unwrap();
    let response = f.app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn a_reader_is_refused_every_admin_route() {
    let f = fixture().await;
    let (_, bob) = reader(&f, "bob").await;
    let routes = [
        ("POST", "/api/libraries"),
        ("DELETE", "/api/libraries/1"),
        ("PATCH", "/api/series/1"),
        ("GET", "/api/series/1/metadata/search?source=anilist&q=x"),
        ("PUT", "/api/series/1/metadata/links/anilist"),
        ("DELETE", "/api/series/1/metadata/links/anilist"),
        ("POST", "/api/series/1/metadata/refresh"),
        ("POST", "/api/series/1/metadata/cover"),
        ("GET", "/api/metadata/thumbnail/anilist/1"),
        ("GET", "/api/admin/metadata"),
        ("GET", "/api/admin/storage"),
        ("POST", "/api/admin/metadata/identify"),
        ("PATCH", "/api/volumes/x"),
        ("GET", "/api/admin/scans"),
        ("POST", "/api/admin/scans"),
        ("GET", "/api/admin/scans/current"),
        ("POST", "/api/admin/scans/current/cancel"),
        ("GET", "/api/admin/scans/1"),
        ("GET", "/api/admin/scans/1/logs"),
        ("PUT", "/api/settings"),
        ("GET", "/api/admin/users"),
        ("POST", "/api/admin/users"),
        ("PATCH", "/api/admin/users/1"),
        ("DELETE", "/api/admin/users/1"),
        ("POST", "/api/admin/users/1/password"),
        ("DELETE", "/api/admin/users/1/sessions"),
        ("DELETE", "/api/admin/users/1/totp"),
    ];
    for (method, uri) in routes {
        let (status, body) = call(&f, Some(&bob), method, uri, Some(json!({}))).await;
        assert_eq!(
            (status, body["code"].as_str()),
            (StatusCode::FORBIDDEN, Some("forbidden")),
            "{method} {uri}"
        );
    }
    for uri in ["/api/libraries", "/api/series", "/api/settings", "/api/me"] {
        let (status, _) = call(&f, Some(&bob), "GET", uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn a_closed_library_does_not_exist_for_the_reader() {
    let f = fixture().await;
    let (bob_id, bob) = reader(&f, "bob").await;
    let closed = json!({ "libraries": [] });
    let uri = format!("/api/admin/users/{bob_id}");
    let (status, body) = call(&f, None, "PATCH", &uri, Some(closed)).await;
    assert_eq!((status, &body["libraries"]), (StatusCode::OK, &json!([])));

    let (_, libraries) = call(&f, Some(&bob), "GET", "/api/libraries", None).await;
    assert_eq!(libraries, json!([]));
    let (_, series) = call(&f, Some(&bob), "GET", "/api/series", None).await;
    assert_eq!(series, json!([]));
    let hidden = [
        format!("/api/libraries/{}/series", f.library),
        format!("/api/series/{}", f.series),
        format!("/api/volumes/{}", f.volume),
        format!("/api/volumes/{}/pages", f.volume),
        format!("/api/volumes/{}/progress", f.volume),
        format!("/api/pages/{}/image", f.page),
    ];
    for uri in &hidden {
        let (status, _) = call(&f, Some(&bob), "GET", uri, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{uri}");
    }
    let progress = format!("/api/volumes/{}/progress", f.volume);
    let page = json!({ "page_id": f.page });
    let (status, _) = call(&f, Some(&bob), "PUT", &progress, Some(page)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let settings = format!("/api/series/{}/settings", f.series);
    let mode = json!({ "reading_mode": "ltr" });
    let (status, _) = call(&f, Some(&bob), "PATCH", &settings, Some(mode)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let open = json!({ "libraries": [f.library] });
    call(&f, None, "PATCH", &uri, Some(open)).await;
    for uri in &hidden[..4] {
        let (status, _) = call(&f, Some(&bob), "GET", uri, None).await;
        assert_eq!(status, StatusCode::OK, "{uri}");
    }
}

#[tokio::test]
async fn a_new_library_is_shared_only_when_asked() {
    let f = fixture().await;
    let (_, bob) = reader(&f, "bob").await;
    for (name, share) in [("Privée", false), ("Partagée", true)] {
        let root = f.dir.path().join(name);
        std::fs::create_dir(&root).unwrap();
        let body = json!({ "name": name, "root": root, "share": share });
        let (status, _) = call(&f, None, "POST", "/api/libraries", Some(body)).await;
        assert_eq!(status, StatusCode::CREATED);
    }
    let (_, libraries) = call(&f, Some(&bob), "GET", "/api/libraries", None).await;
    let names: Vec<&str> = libraries
        .as_array()
        .unwrap()
        .iter()
        .map(|l| l["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["Manga", "Partagée"]);
}

#[tokio::test]
async fn a_temporary_password_must_be_changed_first() {
    let f = fixture().await;
    let body = json!({ "username": "carol" });
    let (status, created) = call(&f, None, "POST", "/api/admin/users", Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["user"]["role"], "reader");
    assert_eq!(
        created["user"]["libraries"],
        json!([f.library]),
        "toutes par défaut"
    );
    let temporary = created["password"].as_str().unwrap().to_owned();

    let other = login(&f, "carol", &temporary).await;
    let carol = login(&f, "carol", &temporary).await;
    let (status, body) = call(&f, Some(&carol), "GET", "/api/libraries", None).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("password_change_required"))
    );
    let (_, me) = call(&f, Some(&carol), "GET", "/api/me", None).await;
    assert_eq!(me["user"]["must_change_password"], true);

    let wrong = json!({ "current": "faux", "new": "mon secret à moi" });
    let (status, body) = call(&f, Some(&carol), "POST", "/api/me/password", Some(wrong)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::FORBIDDEN, Some("wrong_password"))
    );
    let weak = json!({ "current": temporary, "new": "court" });
    let (status, _) = call(&f, Some(&carol), "POST", "/api/me/password", Some(weak)).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let good = json!({ "current": temporary, "new": "mon secret à moi" });
    let (status, _) = call(&f, Some(&carol), "POST", "/api/me/password", Some(good)).await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    let (status, _) = call(&f, Some(&carol), "GET", "/api/libraries", None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "la session d'où l'on change reste ouverte"
    );
    let (status, _) = call(&f, Some(&other), "GET", "/api/me", None).await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "les autres appareils sont déconnectés"
    );
}

#[tokio::test]
async fn preferences_and_devices_of_the_signed_in_account() {
    let f = fixture().await;
    let (_, bob) = reader(&f, "bob").await;
    let (_, me) = call(&f, Some(&bob), "GET", "/api/me", None).await;
    assert_eq!(me["user"]["username"], "bob");
    assert_eq!(
        me["preferences"],
        json!({ "language": null, "theme": null, "page_turn": null })
    );
    let patch = json!({ "language": "en", "theme": "oled" });
    let (_, me) = call(&f, Some(&bob), "PUT", "/api/me", Some(patch)).await;
    assert_eq!(me["preferences"]["language"], "en");
    let patch = json!({ "language": null, "page_turn": "rtl" });
    let (_, me) = call(&f, Some(&bob), "PUT", "/api/me", Some(patch)).await;
    assert_eq!(
        me["preferences"],
        json!({ "language": null, "theme": "oled", "page_turn": "rtl" })
    );
    let (status, _) = call(&f, Some(&bob), "PUT", "/api/me", Some(json!({ "x": 1 }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    let phone = login(&f, "bob", "mot de passe").await;
    let (_, devices) = call(&f, Some(&bob), "GET", "/api/me/sessions", None).await;
    let devices = devices.as_array().unwrap();
    assert_eq!(devices.len(), 2);
    let mine: Vec<bool> = devices
        .iter()
        .map(|d| d["current"].as_bool().unwrap())
        .collect();
    assert_eq!(mine.iter().filter(|c| **c).count(), 1);
    let other = devices.iter().find(|d| d["current"] == false).unwrap();
    let uri = format!("/api/me/sessions/{}", other["id"]);
    let (status, _) = call(&f, Some(&bob), "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&f, Some(&bob), "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&f, Some(&phone), "GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn the_admin_manages_accounts_but_keeps_the_last_admin() {
    let f = fixture().await;
    let (bob_id, bob) = reader(&f, "bob").await;
    let (_, users) = call(&f, None, "GET", "/api/admin/users", None).await;
    let names: Vec<&str> = users
        .as_array()
        .unwrap()
        .iter()
        .map(|u| u["username"].as_str().unwrap())
        .collect();
    assert_eq!(names, ["admin", "bob"]);

    let admin_id = users[0]["id"].as_i64().unwrap();
    let demote = json!({ "role": "reader" });
    let uri = format!("/api/admin/users/{admin_id}");
    let (status, body) = call(&f, None, "PATCH", &uri, Some(demote)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::CONFLICT, Some("last_admin"))
    );
    let (status, _) = call(&f, None, "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::CONFLICT);

    let bad = json!({ "username": "a b" });
    let (status, body) = call(&f, None, "POST", "/api/admin/users", Some(bad)).await;
    assert_eq!(
        (status, body["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_username"))
    );
    let taken = json!({ "username": "BOB" });
    let (status, _) = call(&f, None, "POST", "/api/admin/users", Some(taken)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let admin = json!({ "username": "dave", "role": "admin", "libraries": [] });
    let (_, created) = call(&f, None, "POST", "/api/admin/users", Some(admin)).await;
    assert_eq!(created["user"]["role"], "admin");

    // Mot de passe oublié : nouveau provisoire, sessions fermées.
    let uri = format!("/api/admin/users/{bob_id}/password");
    let (status, body) = call(&f, None, "POST", &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["password"].as_str().unwrap().len(), 16);
    let (status, _) = call(&f, Some(&bob), "GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let password = body["password"].as_str().unwrap().to_owned();
    let bob = login(&f, "bob", &password).await;

    let uri = format!("/api/admin/users/{bob_id}/sessions");
    let (status, _) = call(&f, None, "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = call(&f, Some(&bob), "GET", "/api/me", None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let uri = format!("/api/admin/users/{bob_id}");
    let (status, body) = call(&f, None, "PATCH", &uri, Some(json!({ "disabled": true }))).await;
    assert_eq!(
        (status, body["disabled"].as_bool()),
        (StatusCode::OK, Some(true))
    );
    let (status, _) = call(&f, None, "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    for (method, uri) in [
        ("DELETE", uri.clone()),
        ("PATCH", uri.clone()),
        ("POST", format!("{uri}/password")),
        ("DELETE", format!("{uri}/sessions")),
    ] {
        let (status, _) = call(&f, None, method, &uri, Some(json!({}))).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {uri}");
    }
}
