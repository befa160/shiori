//! Lecture par HTTP : progression d'un tome, réglages d'une série, type de bibliothèque.

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::server::{self, App};
use shiori_core::ScanOptions;
use shiori_db::Db;
use shiori_testkit::fixtures;
use tower::ServiceExt;

struct Fixture {
    _dir: tempfile::TempDir,
    app: Router,
    series: i64,
    volume: String,
    pages: Vec<String>,
}

async fn call(app: &Router, method: &str, uri: &str, body: Option<Value>) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json")
        .body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
        .unwrap();
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = match bytes.is_empty() {
        true => Value::Null,
        false => serde_json::from_slice(&bytes).unwrap(),
    };
    (status, value)
}

/// Bibliothèque créée par l'API avec le type demandé, une série d'un tome scanné.
async fn fixture(kind: Option<&str>) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("bibliotheque");
    let folder = root.join("Série");
    std::fs::create_dir_all(&folder).unwrap();
    let manga = fixtures::manga(&folder);
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let app = server::router(Arc::new(App::new(db.clone(), images)), None);
    let mut body = json!({ "name": "Lib", "root": root });
    if let Some(kind) = kind {
        body["kind"] = json!(kind);
    }
    let (status, created) = call(&app, "POST", "/api/libraries", Some(body)).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    assert_eq!(created["kind"], kind.unwrap_or("manga"));
    let library = db.libraries().await.unwrap().remove(0);
    let run = db.start_run("/", "{}").await.unwrap();
    let scanned = db
        .scan(run, &manga, ScanOptions::default(), false)
        .await
        .unwrap();
    db.attach(&library, &manga).await.unwrap();
    db.refresh_series(&library).await.unwrap();
    let series = db.series_of(library.id).await.unwrap().unwrap()[0].id;
    Fixture {
        _dir: dir,
        app,
        series,
        volume: scanned.report.volume_id.unwrap().to_string(),
        pages: scanned
            .report
            .pages
            .iter()
            .map(|p| p.id.to_string())
            .collect(),
    }
}

#[tokio::test]
async fn progress_round_trip() {
    let f = fixture(None).await;
    let uri = format!("/api/volumes/{}/progress", f.volume);
    assert_eq!(
        call(&f.app, "GET", &uri, None).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );

    let (status, saved) = call(&f.app, "PUT", &uri, Some(json!({ "page_id": f.pages[4] }))).await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    assert_eq!(saved["page_id"], f.pages[4]);
    assert_eq!(
        (saved["ordinal"].as_i64(), saved["page_count"].as_i64()),
        (Some(4), Some(12))
    );
    assert_eq!(saved["finished"], false);
    assert_eq!(
        call(&f.app, "GET", &uri, None).await,
        (StatusCode::OK, saved)
    );

    let last = f.pages.last().unwrap();
    let (_, done) = call(&f.app, "PUT", &uri, Some(json!({ "page_id": last }))).await;
    assert_eq!(done["finished"], true);

    // Le détail du tome et celui de la série portent la progression.
    let (_, volume) = call(&f.app, "GET", &format!("/api/volumes/{}", f.volume), None).await;
    assert_eq!(volume["progress"], done);
    let (_, series) = call(&f.app, "GET", &format!("/api/series/{}", f.series), None).await;
    assert_eq!(series["volumes"][0]["progress"], done);

    assert_eq!(
        call(&f.app, "DELETE", &uri, None).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(
        call(&f.app, "GET", &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    let (_, series) = call(&f.app, "GET", &format!("/api/series/{}", f.series), None).await;
    assert!(series["volumes"][0].get("progress").is_none());
}

#[tokio::test]
async fn progress_errors_have_stable_codes() {
    let f = fixture(None).await;
    let uri = format!("/api/volumes/{}/progress", f.volume);
    let unknown = format!("/api/volumes/{}/progress", "0".repeat(64));
    let foreign = "0".repeat(32);
    let cases = [
        (
            "PUT",
            uri.clone(),
            Some(json!({ "page_id": foreign })),
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_not_in_volume",
        ),
        (
            "PUT",
            uri.clone(),
            Some(json!({ "page_id": "pas-un-id" })),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            "PUT",
            uri.clone(),
            Some(json!({ "page": 3 })),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            "PUT",
            unknown.clone(),
            Some(json!({ "page_id": f.pages[0] })),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            "GET",
            unknown.clone(),
            None,
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        ("DELETE", unknown, None, StatusCode::NOT_FOUND, "not_found"),
        (
            "GET",
            "/api/volumes/abc/progress".into(),
            None,
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
    ];
    for (method, uri, body, status, code) in cases {
        let (got, error) = call(&f.app, method, &uri, body).await;
        assert_eq!(
            (got, error["code"].as_str()),
            (status, Some(code)),
            "{method} {uri}"
        );
    }
}

#[tokio::test]
async fn page_turn_follows_the_global_default_unless_the_series_sets_it() {
    let f = fixture(Some("manga")).await;
    let uri = format!("/api/series/{}/settings", f.series);
    let settings = |f: &Fixture| {
        let series = format!("/api/series/{}", f.series);
        let app = f.app.clone();
        async move { call(&app, "GET", &series, None).await.1["settings"].clone() }
    };
    assert_eq!(settings(&f).await["page_turn"], "reading");
    call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "page_turn": "ltr" })),
    )
    .await;
    let s = settings(&f).await;
    assert_eq!(
        (s["page_turn"].as_str(), &s["page_turn_override"]),
        (Some("ltr"), &Value::Null)
    );
    let (_, s) = call(&f.app, "PATCH", &uri, Some(json!({ "page_turn": "rtl" }))).await;
    assert_eq!(
        (s["page_turn"].as_str(), s["page_turn_override"].as_str()),
        (Some("rtl"), Some("rtl"))
    );
    // Le sens de lecture n'a pas bougé, et `null` rend le défaut global.
    assert_eq!(s["reading_mode"], "rtl");
    let (_, s) = call(&f.app, "PATCH", &uri, Some(json!({ "page_turn": null }))).await;
    assert_eq!(s["page_turn"], "ltr");
    let (_, volume) = call(&f.app, "GET", &format!("/api/volumes/{}", f.volume), None).await;
    assert_eq!(volume["settings"], s);
}

#[tokio::test]
async fn series_settings_patch() {
    let f = fixture(Some("comics")).await;
    let series_uri = format!("/api/series/{}", f.series);
    let uri = format!("{series_uri}/settings");
    let (_, series) = call(&f.app, "GET", &series_uri, None).await;
    assert_eq!(
        series["settings"],
        json!({ "reading_mode": "ltr", "page_layout": "auto", "page_turn": "reading",
                "reading_mode_override": null, "page_layout_override": null,
                "page_turn_override": null })
    );

    let (status, s) = call(
        &f.app,
        "PATCH",
        &uri,
        Some(json!({ "reading_mode": "rtl" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{s}");
    assert_eq!(
        (
            s["reading_mode"].as_str(),
            s["reading_mode_override"].as_str()
        ),
        (Some("rtl"), Some("rtl"))
    );
    // Champ absent : inchangé ; `null` : retour au défaut de la bibliothèque.
    let (_, s) = call(
        &f.app,
        "PATCH",
        &uri,
        Some(json!({ "page_layout": "single" })),
    )
    .await;
    assert_eq!(
        (s["reading_mode"].as_str(), s["page_layout"].as_str()),
        (Some("rtl"), Some("single"))
    );
    let (_, s) = call(&f.app, "PATCH", &uri, Some(json!({ "reading_mode": null }))).await;
    assert_eq!(
        (s["reading_mode"].as_str(), s["page_layout"].as_str()),
        (Some("ltr"), Some("single"))
    );
    let (_, volume) = call(&f.app, "GET", &format!("/api/volumes/{}", f.volume), None).await;
    assert_eq!(volume["settings"], s);

    for (body, uri, status, code) in [
        (
            json!({ "reading_mode": "diagonal" }),
            uri.clone(),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            json!({ "couleur": "rouge" }),
            uri.clone(),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            json!({}),
            "/api/series/999/settings".into(),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            json!({}),
            "/api/series/x/settings".into(),
            StatusCode::BAD_REQUEST,
            "invalid_id",
        ),
    ] {
        let (got, error) = call(&f.app, "PATCH", &uri, Some(body.clone())).await;
        assert_eq!(
            (got, error["code"].as_str()),
            (status, Some(code)),
            "{body}"
        );
    }
}

#[tokio::test]
async fn library_kind_is_listed_and_validated() {
    let f = fixture(Some("webtoon")).await;
    let (_, libraries) = call(&f.app, "GET", "/api/libraries", None).await;
    assert_eq!(libraries[0]["kind"], "webtoon");
    let (_, series) = call(&f.app, "GET", &format!("/api/series/{}", f.series), None).await;
    assert_eq!(series["settings"]["reading_mode"], "vertical");
    let body = json!({ "name": "Autre", "root": "/", "kind": "roman" });
    let (status, error) = call(&f.app, "POST", "/api/libraries", Some(body)).await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_body"))
    );
}

#[tokio::test]
async fn language_setting_round_trip() {
    let f = fixture(None).await;
    assert_eq!(
        call(&f.app, "GET", "/api/settings", None).await,
        (
            StatusCode::OK,
            json!({ "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system" })
        )
    );
    let (status, saved) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "language": "en" })),
    )
    .await;
    assert_eq!(
        (status, saved),
        (
            StatusCode::OK,
            json!({ "language": "en", "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system" })
        )
    );
    assert_eq!(
        call(&f.app, "GET", "/api/settings", None).await.1,
        json!({ "language": "en", "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system" })
    );
    let (_, reset) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "language": null })),
    )
    .await;
    assert_eq!(
        reset,
        json!({ "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system" })
    );
    // Un champ absent reste inchangé.
    let (_, turned) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "page_turn": "ltr" })),
    )
    .await;
    assert_eq!(
        turned,
        json!({ "language": null, "page_turn": "ltr", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system" })
    );
    let (_, themed) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "theme": "sepia" })),
    )
    .await;
    assert_eq!(
        (themed["theme"].as_str(), themed["page_turn"].as_str()),
        (Some("sepia"), Some("ltr"))
    );
    for body in [
        json!({ "language": "de" }),
        json!({ "theme": null }),
        json!({ "theme": "rose" }),
        json!({ "page_turn": null }),
        json!({ "language": "fr", "page_turn": null }),
        json!({ "page_turn": "haut" }),
        json!({ "langue": "fr" }),
        json!({}),
    ] {
        let (status, error) = call(&f.app, "PUT", "/api/settings", Some(body.clone())).await;
        assert_eq!(
            (status, error["code"].as_str()),
            (StatusCode::BAD_REQUEST, Some("invalid_body")),
            "{body}"
        );
    }
}
