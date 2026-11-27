//! Lecture par HTTP : progression d'un tome, marque-pages, réglages d'une série, type de bibliothèque.

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
use shiori_db::Db;
use shiori_testkit::fixtures;
use tower::ServiceExt;

struct Fixture {
    _dir: tempfile::TempDir,
    db: Arc<Db>,
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
    let app = common::router(Arc::new(App::new(db.clone(), images))).await;
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
    // Le compte de la session des tests (créé par `common::router`).
    let admin = db.user_by_name("admin").await.unwrap().unwrap();
    let viewer = db.viewer(admin.id).await.unwrap();
    let series = db.series_of(&viewer, library.id).await.unwrap().unwrap()[0].id;
    Fixture {
        _dir: dir,
        db,
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
            json!({ "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false })
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
            json!({ "language": "en", "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false })
        )
    );
    assert_eq!(
        call(&f.app, "GET", "/api/settings", None).await.1,
        json!({ "language": "en", "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false })
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
        json!({ "language": null, "page_turn": "reading", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false })
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
        json!({ "language": null, "page_turn": "ltr", "metadata_online": false, "metadata_sources": null, "metadata_adult": false, "theme": "system", "full_scan_hours": 24, "metadata_refresh_days": 0, "comicvine_key_set": false })
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
    let (_, hourly) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "full_scan_hours": 6 })),
    )
    .await;
    assert_eq!(hourly["full_scan_hours"], 6);
    for body in [
        json!({ "full_scan_hours": 5 }),
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

/// Chaque compte a sa progression et ses réglages (Q48) : un autre lecteur ne voit rien de ce
/// que l'administrateur a lu ou réglé.
#[tokio::test]
async fn progress_and_settings_belong_to_the_account() {
    let f = fixture(None).await;
    let uri = format!("/api/volumes/{}/progress", f.volume);
    call(&f.app, "PUT", &uri, Some(json!({ "page_id": f.pages[4] }))).await;
    let settings = format!("/api/series/{}/settings", f.series);
    call(
        &f.app,
        "PATCH",
        &settings,
        Some(json!({ "reading_mode": "ltr" })),
    )
    .await;

    let bob =
        f.db.create_user("bob", "mot de passe", shiori_db::Role::Reader)
            .await
            .unwrap();
    f.db.set_password("bob", "mot de passe", false, None)
        .await
        .unwrap();
    let session = f.db.create_session(bob.id, "tests").await.unwrap();
    let as_bob = |method: &str, uri: &str| {
        Request::builder()
            .method(method)
            .uri(uri)
            .header("cookie", format!("shiori_session={}", session.token))
            .body(Body::empty())
            .unwrap()
    };
    let response = f.app.clone().oneshot(as_bob("GET", &uri)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    let response = f
        .app
        .clone()
        .oneshot(as_bob("GET", &format!("/api/series/{}", f.series)))
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let series: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(series["settings"]["reading_mode"], "rtl");
    assert!(series["volumes"][0].get("progress").is_none());

    let (_, mine) = call(&f.app, "GET", &uri, None).await;
    assert_eq!(mine["ordinal"], 4);
}

/// « Marquer la série comme lue / non lue » renvoie son détail à jour ; une série inconnue est 404.
#[tokio::test]
async fn a_series_is_marked_read_then_unread() {
    let f = fixture(None).await;
    let uri = format!("/api/series/{}/read", f.series);
    let (status, detail) = call(&f.app, "PUT", &uri, None).await;
    assert_eq!(status, StatusCode::OK, "{detail}");
    let progress = &detail["volumes"][0]["progress"];
    assert_eq!(
        (progress["finished"].as_bool(), progress["page_id"].as_str()),
        (Some(true), Some(f.pages[11].as_str()))
    );
    let (status, detail) = call(&f.app, "DELETE", &uri, None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(detail["volumes"][0]["progress"].is_null(), "{detail}");
    let volume = format!("/api/volumes/{}/progress", f.volume);
    assert_eq!(
        call(&f.app, "GET", &volume, None).await.0,
        StatusCode::NO_CONTENT
    );
    for method in ["PUT", "DELETE"] {
        let (status, err) = call(&f.app, method, "/api/series/999999/read", None).await;
        assert_eq!(
            (status, err["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found")),
            "{method}"
        );
    }
}

/// Suivre une série, voir ses tomes ajoutés dans les nouveautés, puis « Tout marquer comme vu ».
#[tokio::test]
async fn followed_series_list_their_new_volumes() {
    let f = fixture(None).await;
    let uri = format!("/api/series/{}/follow", f.series);
    let detail = format!("/api/series/{}", f.series);
    assert_eq!(
        call(&f.app, "GET", &detail, None).await.1["followed"],
        false
    );
    assert_eq!(
        call(&f.app, "PUT", &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(call(&f.app, "GET", &detail, None).await.1["followed"], true);
    tokio::time::sleep(std::time::Duration::from_millis(3)).await;

    let folder = f._dir.path().join("bibliotheque/Série");
    let file = fixtures::unpadded(&folder);
    let library = f.db.libraries().await.unwrap().remove(0);
    let run = f.db.start_run("/", "{}").await.unwrap();
    let scanned =
        f.db.scan(run, &file, ScanOptions::default(), false)
            .await
            .unwrap();
    f.db.attach(&library, &file).await.unwrap();
    f.db.refresh_series(&library).await.unwrap();
    let (status, new) = call(&f.app, "GET", "/api/me/new", None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(new.as_array().unwrap().len(), 1, "{new}");
    assert_eq!(
        new[0]["volume"]["volume_id"],
        scanned.report.volume_id.unwrap().to_string()
    );
    assert_eq!(new[0]["series_id"], f.series);
    assert!(new[0]["added_at"].is_string());

    let seen = call(&f.app, "POST", "/api/me/new/seen", None).await;
    assert_eq!(seen.0, StatusCode::NO_CONTENT);
    assert_eq!(call(&f.app, "GET", "/api/me/new", None).await.1, json!([]));
    assert_eq!(
        call(&f.app, "DELETE", &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        call(&f.app, "GET", &detail, None).await.1["followed"],
        false
    );
    for method in ["PUT", "DELETE"] {
        let (status, err) = call(&f.app, method, "/api/series/999999/follow", None).await;
        assert_eq!(
            (status, err["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found")),
            "{method}"
        );
    }
}

/// « À lire » : mise de côté, visible dans la liste des séries, puis retirée en commençant.
#[tokio::test]
async fn a_series_goes_to_read_then_leaves_when_started() {
    let f = fixture(None).await;
    let uri = format!("/api/series/{}/to-read", f.series);
    let detail = format!("/api/series/{}", f.series);
    assert_eq!(
        call(&f.app, "PUT", &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(call(&f.app, "GET", &detail, None).await.1["to_read"], true);
    let (_, list) = call(&f.app, "GET", "/api/series", None).await;
    assert!(list[0]["to_read_at"].is_string(), "{list}");

    let progress = format!("/api/volumes/{}/progress", f.volume);
    let body = json!({ "page_id": f.pages[1] });
    assert_eq!(
        call(&f.app, "PUT", &progress, Some(body)).await.0,
        StatusCode::OK
    );
    assert_eq!(call(&f.app, "GET", &detail, None).await.1["to_read"], false);
    let (_, list) = call(&f.app, "GET", "/api/series", None).await;
    assert!(list[0]["to_read_at"].is_null(), "{list}");

    call(&f.app, "PUT", &uri, None).await;
    assert_eq!(
        call(&f.app, "DELETE", &uri, None).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(call(&f.app, "GET", &detail, None).await.1["to_read"], false);
    for method in ["PUT", "DELETE"] {
        let (status, err) = call(&f.app, method, "/api/series/999999/to-read", None).await;
        assert_eq!(
            (status, err["code"].as_str()),
            (StatusCode::NOT_FOUND, Some("not_found")),
            "{method}"
        );
    }
}

#[tokio::test]
async fn reading_stats_follow_the_progress() {
    let f = fixture(None).await;
    let uri = format!("/api/volumes/{}/progress", f.volume);
    // Lu hors ligne le 1er janvier à 23 h UTC sur un appareil à UTC+14 (décalage hors limites
    // ramené à 14 h) : compté le 2 janvier. Puis la dernière page, maintenant (11 pages de plus).
    let body =
        json!({ "page_id": f.pages[0], "read_at": "2020-01-01T23:00:00Z", "utc_offset": 5000 });
    assert_eq!(
        call(&f.app, "PUT", &uri, Some(body)).await.0,
        StatusCode::OK
    );
    let last = f.pages.last().unwrap();
    let body = json!({ "page_id": last, "utc_offset": 60 });
    assert_eq!(
        call(&f.app, "PUT", &uri, Some(body)).await.0,
        StatusCode::OK
    );

    let (status, stats) = call(&f.app, "GET", "/api/me/stats?days=7&utc_offset=60", None).await;
    assert_eq!(status, StatusCode::OK, "{stats}");
    assert_eq!(stats["days"].as_array().unwrap().len(), 1);
    assert_eq!(stats["days"][0]["finished"], 1);
    assert_eq!(
        stats["total"],
        json!({ "pages": 12, "seconds": 0, "finished": 1 })
    );
    assert_eq!(stats["series_finished"], 1);
    let day: (String,) = sqlx::query_as("SELECT min(day) FROM reading_activity")
        .fetch_one(f.db.pool())
        .await
        .unwrap();
    assert_eq!(day.0, "2020-01-02");

    // « Marquer comme lu » (`stats: false`) n'y entre pas.
    call(&f.app, "DELETE", &uri, None).await;
    let body = json!({ "page_id": last, "stats": false });
    assert_eq!(
        call(&f.app, "PUT", &uri, Some(body)).await.0,
        StatusCode::OK
    );
    let (_, again) = call(&f.app, "GET", "/api/me/stats", None).await;
    assert_eq!(again["total"], stats["total"]);

    // Défauts (84 jours, UTC) ; paramètre invalide refusé avec un code stable.
    assert_eq!(
        call(&f.app, "GET", "/api/me/stats", None).await.0,
        StatusCode::OK
    );
    let (status, error) = call(&f.app, "GET", "/api/me/stats?days=abc", None).await;
    assert_eq!(
        (status, &error["code"]),
        (StatusCode::BAD_REQUEST, &json!("invalid_query"))
    );
}

#[tokio::test]
async fn bookmarks_round_trip() {
    let f = fixture(None).await;
    let list = format!("/api/volumes/{}/bookmarks", f.volume);
    assert_eq!(
        call(&f.app, "GET", &list, None).await,
        (StatusCode::OK, json!([]))
    );

    let (status, late) = call(
        &f.app,
        "POST",
        &list,
        Some(json!({ "page_id": f.pages[9], "note": "  combat " })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{late}");
    assert_eq!(
        (
            late["page_id"].as_str(),
            late["ordinal"].as_i64(),
            late["note"].as_str()
        ),
        (Some(&*f.pages[9]), Some(9), Some("combat"))
    );
    assert_eq!(late["volume_id"], f.volume);
    // Page déjà marquée : 200 et le marque-page d'origine.
    let again = call(
        &f.app,
        "POST",
        &list,
        Some(json!({ "page_id": f.pages[9], "note": "autre" })),
    )
    .await;
    assert_eq!(again, (StatusCode::OK, late.clone()));
    let (_, early) = call(
        &f.app,
        "POST",
        &list,
        Some(json!({ "page_id": f.pages[1] })),
    )
    .await;
    assert_eq!(early["note"], "");

    let (_, listed) = call(&f.app, "GET", &list, None).await;
    assert_eq!(listed, json!([early, late]));
    let (_, of_series) = call(
        &f.app,
        "GET",
        &format!("/api/series/{}/bookmarks", f.series),
        None,
    )
    .await;
    assert_eq!(of_series, listed);

    let one = format!("/api/bookmarks/{}", early["id"]);
    let (status, noted) = call(&f.app, "PATCH", &one, Some(json!({ "note": "début" }))).await;
    assert_eq!(
        (status, noted["note"].as_str()),
        (StatusCode::OK, Some("début"))
    );
    assert_eq!(
        call(&f.app, "DELETE", &one, None).await,
        (StatusCode::NO_CONTENT, Value::Null)
    );
    assert_eq!(
        call(&f.app, "DELETE", &one, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&f.app, "PATCH", &one, Some(json!({ "note": "x" })))
            .await
            .0,
        StatusCode::NOT_FOUND
    );

    // Refus : note trop longue, page d'un autre tome, champ inconnu, identifiants invalides.
    let long = "x".repeat(shiori_db::MAX_NOTE + 1);
    for (body, status, code) in [
        (
            json!({ "page_id": f.pages[2], "note": long }),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            json!({ "page_id": "0".repeat(32) }),
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_not_in_volume",
        ),
        (
            json!({ "page_id": "pas un id" }),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
        (
            json!({ "page_id": f.pages[2], "nope": 1 }),
            StatusCode::BAD_REQUEST,
            "invalid_body",
        ),
    ] {
        let (got, error) = call(&f.app, "POST", &list, Some(body)).await;
        assert_eq!(
            (got, error["code"].as_str()),
            (status, Some(code)),
            "{error}"
        );
    }
    let late_uri = format!("/api/bookmarks/{}", late["id"]);
    let (got, error) = call(&f.app, "PATCH", &late_uri, Some(json!({ "note": long }))).await;
    assert_eq!(
        (got, error["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_body"))
    );
    assert_eq!(
        call(&f.app, "DELETE", "/api/bookmarks/abc", None).await.0,
        StatusCode::BAD_REQUEST
    );
    let unknown = format!("/api/volumes/{}/bookmarks", "0".repeat(64));
    assert_eq!(
        call(&f.app, "GET", &unknown, None).await.0,
        StatusCode::NOT_FOUND
    );
    let (got, _) = call(
        &f.app,
        "POST",
        &unknown,
        Some(json!({ "page_id": f.pages[2] })),
    )
    .await;
    assert_eq!(got, StatusCode::NOT_FOUND);
    assert_eq!(
        call(&f.app, "GET", "/api/series/999/bookmarks", None)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
