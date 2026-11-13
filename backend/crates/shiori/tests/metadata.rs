//! Métadonnées web par l'API (lot 5.5b) : fausses sources, aucun appel réseau.

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use shiori::cache::Images;
use shiori::job::ScanRequest;
use shiori::server::{self, App};
use shiori_db::{Db, LibraryKind};
use shiori_meta::{FetchError, Source, SourceId, Text, WebMetadata};
use shiori_testkit::{STORED, page_color, png, write_zip};
use tower::ServiceExt;

type Answer<T> = Result<T, FetchError>;

/// Source qui rejoue les réponses prévues, dans l'ordre, et note chaque appel.
struct Fake {
    id: SourceId,
    searches: Mutex<VecDeque<Answer<Vec<WebMetadata>>>>,
    fetches: Mutex<VecDeque<Answer<WebMetadata>>>,
    calls: Mutex<Vec<String>>,
    delay: Duration,
}

impl Fake {
    fn search_gives(&self, answer: Answer<Vec<WebMetadata>>) {
        self.searches.lock().unwrap().push_back(answer);
    }

    fn fetch_gives(&self, answer: Answer<WebMetadata>) {
        self.fetches.lock().unwrap().push_back(answer);
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

impl Source for Fake {
    fn id(&self) -> SourceId {
        self.id
    }

    fn search(&self, query: &str) -> Answer<Vec<WebMetadata>> {
        self.calls.lock().unwrap().push(format!("search {query}"));
        std::thread::sleep(self.delay);
        self.searches
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Ok(vec![]))
    }

    fn fetch(&self, id: &str) -> Answer<WebMetadata> {
        self.calls.lock().unwrap().push(format!("fetch {id}"));
        self.fetches
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(Err(FetchError::NotFound))
    }

    /// Une vraie image (600 × 900) pour l'adresse prévue, autre chose sinon.
    fn image(&self, url: &str) -> Answer<Vec<u8>> {
        self.calls.lock().unwrap().push(format!("image {url}"));
        match url {
            "https://s4.anilist.co/c.jpg" => Ok(png(600, 900, page_color(3))),
            _ => Ok(b"pas une image".to_vec()),
        }
    }
}

fn fiche(id: &str, title: &str) -> WebMetadata {
    WebMetadata {
        id: id.into(),
        titles: vec![Text {
            lang: Some("en".into()),
            text: title.into(),
        }],
        summaries: vec![Text {
            lang: Some("en".into()),
            text: format!("About {title}."),
        }],
        authors: vec![shiori_meta::Author {
            name: "Auteur".into(),
            story: true,
            art: true,
        }],
        cover_url: Some("https://s4.anilist.co/c.jpg".into()),
        ..WebMetadata::default()
    }
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
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

struct Fixture {
    _dir: tempfile::TempDir,
    state: Arc<App>,
    app: Router,
    anilist: Arc<Fake>,
    library: i64,
}

/// Deux séries-dossiers d'un tome : « Blue Lock » et « Gigant ».
async fn fixture(delay: Duration) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("manga");
    for (i, name) in ["Blue Lock", "Gigant"].iter().enumerate() {
        let folder = root.join(name);
        std::fs::create_dir_all(&folder).unwrap();
        let page = png(4, 6, page_color(i));
        write_zip(
            &folder,
            &format!("{name} T01.cbz"),
            &[("01.png", &page, STORED)],
        );
    }
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let library = db
        .create_library("M", &root, LibraryKind::Manga)
        .await
        .unwrap()
        .id;
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let anilist = Arc::new(Fake {
        id: SourceId::Anilist,
        searches: Mutex::default(),
        fetches: Mutex::default(),
        calls: Mutex::default(),
        delay,
    });
    let sources: Vec<Arc<dyn Source>> = vec![anilist.clone()];
    // Un fichier à la fois : les séries reçoivent leurs ids dans l'ordre des dossiers.
    let state = Arc::new(App::build(db, images, 1, 30, sources));
    let app = server::router(state.clone(), false);
    Fixture {
        _dir: dir,
        state,
        app,
        anilist,
        library,
    }
}

/// Passe complète par le job de scan (elle lance l'identification à la fin), puis attente.
async fn scan(f: &Fixture) {
    let request = ScanRequest {
        library_id: f.library,
        path: None,
        deep: false,
        rehash: false,
    };
    f.state
        .scanner
        .start(f.state.db.clone(), request)
        .await
        .unwrap();
    f.state.scanner.wait().await;
    f.state.metadata.wait().await;
}

async fn series_id(f: &Fixture, folder: &str) -> i64 {
    let (_, all) = call(&f.app, "GET", "/api/series", None).await;
    let detail = |s: &Value| s["id"].as_i64().unwrap();
    for s in all.as_array().unwrap() {
        let (_, d) = call(&f.app, "GET", &format!("/api/series/{}", detail(s)), None).await;
        if d["folder"] == folder {
            return detail(s);
        }
    }
    panic!("{folder} introuvable")
}

async fn enable(f: &Fixture) {
    let body = json!({ "metadata_online": true, "metadata_sources": ["anilist"] });
    let (status, settings) = call(&f.app, "PUT", "/api/settings", Some(body)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        (
            settings["metadata_online"].as_bool(),
            &settings["metadata_sources"]
        ),
        (Some(true), &json!(["anilist"]))
    );
}

#[tokio::test]
async fn nothing_leaves_while_the_network_is_off() {
    let f = fixture(Duration::ZERO).await;
    scan(&f).await;
    let id = series_id(&f, "Gigant").await;
    let (status, err) = call(
        &f.app,
        "GET",
        &format!("/api/series/{id}/metadata/search?source=anilist&q=Gigant"),
        None,
    )
    .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("metadata_disabled"))
    );
    let (status, _) = call(&f.app, "POST", "/api/admin/metadata/identify", None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, err) = call(
        &f.app,
        "POST",
        &format!("/api/series/{id}/metadata/refresh"),
        None,
    )
    .await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("metadata_disabled"))
    );
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["online"], false);
    // Français par défaut : MangaDex d'abord.
    assert_eq!(overview["sources"], json!(["mangadex", "anilist"]));
    assert!(f.anilist.calls().is_empty(), "{:?}", f.anilist.calls());

    // Réglages invalides.
    for bad in [
        json!({ "metadata_online": null }),
        json!({ "metadata_sources": [] }),
        json!({ "metadata_sources": ["anilist", "anilist"] }),
        json!({ "metadata_sources": ["kitsu"] }),
    ] {
        let (status, _) = call(&f.app, "PUT", "/api/settings", Some(bad.clone())).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (_, settings) = call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "metadata_sources": null })),
    )
    .await;
    assert_eq!(settings["metadata_sources"], Value::Null);
}

#[tokio::test]
async fn a_scan_identifies_new_series_then_one_is_confirmed_by_hand() {
    let f = fixture(Duration::ZERO).await;
    enable(&f).await;
    // Ordre des séries : par id (Blue Lock, puis Gigant).
    f.anilist.search_gives(Ok(vec![
        fiche("1", "Blue Lock"),
        fiche("2", "Blue Lock: Episode Nagi"),
    ]));
    f.anilist
        .search_gives(Ok(vec![fiche("3", "Gigant"), fiche("4", "GIGANT")]));
    scan(&f).await;
    assert_eq!(f.anilist.calls(), ["search Blue Lock", "search Gigant"]);
    let (blue, gigant) = (
        series_id(&f, "Blue Lock").await,
        series_id(&f, "Gigant").await,
    );

    // Un seul titre exact : lié tout seul, et la fiche nourrit la série.
    let (_, detail) = call(&f.app, "GET", &format!("/api/series/{blue}"), None).await;
    assert_eq!(detail["metadata"]["summary"], "About Blue Lock.");
    assert_eq!(detail["sources"][0]["matched_by"], "auto");
    // Deux titres exacts : à confirmer, sans adresse de couverture pour le navigateur.
    let (_, detail) = call(&f.app, "GET", &format!("/api/series/{gigant}"), None).await;
    let candidates = detail["sources"][0]["candidates"].as_array().unwrap();
    assert_eq!(candidates.len(), 2);
    assert_eq!(candidates[0]["cover_url"], Value::Null);
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(
        overview["identify"],
        json!({ "running": false, "done": 2, "total": 2, "stopped": null })
    );
    assert_eq!(
        overview["issues"],
        json!([{ "series_id": gigant, "series_name": "Gigant", "source": "anilist", "candidates": 2, "last_error": null }])
    );

    // Recherche à la main, puis choix : la fiche est relue avant d'être liée.
    f.anilist.search_gives(Ok(vec![fiche("4", "GIGANT")]));
    let (status, found) = call(
        &f.app,
        "GET",
        &format!("/api/series/{gigant}/metadata/search?source=anilist&q=%20gigant%20"),
        None,
    )
    .await;
    assert_eq!(
        (status, found[0]["id"].as_str(), &found[0]["cover_url"]),
        (StatusCode::OK, Some("4"), &Value::Null)
    );
    f.anilist.fetch_gives(Ok(fiche("4", "GIGANT")));
    let uri = format!("/api/series/{gigant}/metadata/links/anilist");
    let (status, detail) = call(&f.app, "PUT", &uri, Some(json!({ "external_id": "4" }))).await;
    assert_eq!(
        (status, detail["name"].as_str()),
        (StatusCode::OK, Some("GIGANT"))
    );
    assert_eq!(
        (
            detail["sources"][0]["matched_by"].as_str(),
            detail["sources"][0]["candidates"].as_array().map(Vec::len)
        ),
        (Some("user"), Some(0))
    );
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["issues"], json!([]));

    // Relecture : un échec est noté, la fiche d'avant reste.
    f.anilist
        .fetch_gives(Err(FetchError::RateLimited(Duration::from_secs(60))));
    let (status, detail) = call(
        &f.app,
        "POST",
        &format!("/api/series/{gigant}/metadata/refresh"),
        None,
    )
    .await;
    assert_eq!(
        (status, detail["name"].as_str()),
        (StatusCode::OK, Some("GIGANT"))
    );
    assert_eq!(detail["sources"][0]["last_error"], "rate_limited");
    // Délier : la série reprend son nom.
    let (status, detail) = call(&f.app, "DELETE", &uri, None).await;
    assert_eq!(
        (
            status,
            detail["name"].as_str(),
            detail["sources"].as_array().map(Vec::len)
        ),
        (StatusCode::OK, Some("Gigant"), Some(0))
    );
    assert_eq!(
        f.anilist.calls().last().map(String::as_str),
        Some("fetch 4")
    );
}

#[tokio::test]
async fn errors_are_explicit_and_a_rate_limit_stops_the_identification() {
    let f = fixture(Duration::ZERO).await;
    scan(&f).await;
    enable(&f).await;
    let gigant = series_id(&f, "Gigant").await;
    let search = |q: &str| format!("/api/series/{gigant}/metadata/search?source=anilist&q={q}");
    // Source injoignable, fiche inconnue, requête vide, série ou source inconnues.
    f.anilist
        .search_gives(Err(FetchError::Offline("dns".into())));
    let (status, err) = call(&f.app, "GET", &search("x"), None).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("offline"))
    );
    let link = format!("/api/series/{gigant}/metadata/links/anilist");
    let (status, err) = call(&f.app, "PUT", &link, Some(json!({ "external_id": "999" }))).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let (status, _) = call(&f.app, "GET", &search("%20"), None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, _) = call(
        &f.app,
        "GET",
        "/api/series/999/metadata/search?source=anilist&q=x",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(
        &f.app,
        "GET",
        &format!("/api/series/{gigant}/metadata/search?source=mangadex&q=x"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "source non configurée");
    let (status, _) = call(
        &f.app,
        "DELETE",
        &format!("/api/series/{gigant}/metadata/links/kitsu"),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = call(&f.app, "PUT", &link, Some(json!({ "id": "1" }))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, err) = call(&f.app, "GET", &search("x"), None).await;
    assert_eq!((status, err), (StatusCode::OK, json!([])));

    // Identification : la source demande d'attendre dès la première série ; rien n'est noté.
    f.anilist
        .search_gives(Err(FetchError::RateLimited(Duration::from_secs(60))));
    let (status, _) = call(&f.app, "POST", "/api/admin/metadata/identify", None).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    f.state.metadata.wait().await;
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["identify"]["stopped"], "rate_limited");
    assert_eq!(overview["identify"]["done"], 0);
    assert_eq!(overview["issues"], json!([]));
    // Une autre erreur est notée série par série, et retentée à la passe suivante.
    f.anilist.search_gives(Err(FetchError::Http(500)));
    f.anilist.search_gives(Ok(vec![]));
    f.state.metadata.identify().unwrap();
    f.state.metadata.wait().await;
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["issues"][0]["last_error"], "http_error");
    assert_eq!(
        overview["identify"],
        json!({ "running": false, "done": 2, "total": 2, "stopped": null })
    );
    // Relire une série sans fiche liée n'appelle rien.
    let before = f.anilist.calls().len();
    let uri = format!(
        "/api/series/{}/metadata/refresh",
        overview["issues"][0]["series_id"]
    );
    let (status, _) = call(&f.app, "POST", &uri, None).await;
    assert_eq!((status, f.anilist.calls().len()), (StatusCode::OK, before));
    // Ordre par défaut (MangaDex d'abord) : la source non configurée est passée.
    call(
        &f.app,
        "PUT",
        "/api/settings",
        Some(json!({ "metadata_sources": null })),
    )
    .await;
    f.state.metadata.identify().unwrap();
    f.state.metadata.wait().await;
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["sources"], json!(["mangadex", "anilist"]));
    assert_eq!(
        overview["identify"]["total"], 1,
        "seule la série en échec est retentée"
    );
}

#[tokio::test]
async fn only_one_identification_at_a_time() {
    let f = fixture(Duration::from_millis(300)).await;
    scan(&f).await;
    enable(&f).await;
    f.state.metadata.identify().unwrap();
    let (status, err) = call(&f.app, "POST", "/api/admin/metadata/identify", None).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::CONFLICT, Some("already_running"))
    );
    let (_, overview) = call(&f.app, "GET", "/api/admin/metadata", None).await;
    assert_eq!(overview["identify"]["running"], true);
    f.state.metadata.wait().await;
}

#[tokio::test]
async fn source_covers_are_relayed_kept_and_served() {
    let f = fixture(Duration::ZERO).await;
    scan(&f).await;
    let gigant = series_id(&f, "Gigant").await;
    let (status, _) = call(&f.app, "GET", "/api/metadata/thumbnail/anilist/4", None).await;
    assert_eq!(status, StatusCode::CONFLICT, "réseau désactivé");
    enable(&f).await;

    // Vignette d'un résultat de recherche : relayée par le serveur, gardée un jour.
    let other = WebMetadata {
        cover_url: Some("https://x/bad.jpg".into()),
        ..fiche("5", "Autre")
    };
    f.anilist
        .search_gives(Ok(vec![fiche("4", "GIGANT"), other]));
    call(
        &f.app,
        "GET",
        &format!("/api/series/{gigant}/metadata/search?source=anilist&q=gigant"),
        None,
    )
    .await;
    let response = raw(&f.app, "/api/metadata/thumbnail/anilist/4", None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["content-type"], "image/png");
    assert_eq!(
        response.headers()["cache-control"],
        "private, max-age=86400"
    );
    let (status, err) = call(&f.app, "GET", "/api/metadata/thumbnail/anilist/5", None).await;
    assert_eq!(
        (status, err["code"].as_str()),
        (StatusCode::BAD_GATEWAY, Some("bad_response")),
        "pas une image"
    );
    let (status, _) = call(
        &f.app,
        "GET",
        "/api/metadata/thumbnail/anilist/inconnu",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Pas de fiche liée : pas de couverture de source.
    let use_cover = format!("/api/series/{gigant}/metadata/cover");
    let (status, _) = call(
        &f.app,
        "POST",
        &use_cover,
        Some(json!({ "source": "anilist" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Liée : sa couverture devient celle de la série, gardée par le serveur.
    f.anilist.fetch_gives(Ok(fiche("4", "GIGANT")));
    let link = format!("/api/series/{gigant}/metadata/links/anilist");
    call(&f.app, "PUT", &link, Some(json!({ "external_id": "4" }))).await;
    let (status, detail) = call(
        &f.app,
        "POST",
        &use_cover,
        Some(json!({ "source": "anilist" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let id = detail["metadata"]["cover_image_id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(id.len(), 64);
    assert!(
        detail["locked"]
            .as_array()
            .unwrap()
            .contains(&json!("cover_image_id"))
    );
    let (_, all) = call(&f.app, "GET", "/api/series", None).await;
    assert!(
        all.as_array()
            .unwrap()
            .iter()
            .any(|s| s["cover_image_id"] == id.as_str())
    );

    // Servie : originale, ou redimensionnée en JPEG ; immuable, 304 sur ETag.
    let original = raw(&f.app, &format!("/api/covers/{id}"), None).await;
    assert_eq!(original.status(), StatusCode::OK);
    assert_eq!(original.headers()["content-type"], "image/png");
    let small = raw(&f.app, &format!("/api/covers/{id}?w=320&f=jpeg"), None).await;
    assert_eq!(small.headers()["content-type"], "image/jpeg");
    let etag = small.headers()["etag"].to_str().unwrap().to_owned();
    assert!(
        small.headers()["cache-control"]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    let bytes = small.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(imagesize::blob_size(&bytes).unwrap().width, 320);
    let again = raw(
        &f.app,
        &format!("/api/covers/{id}?w=320&f=jpeg"),
        Some(&etag),
    )
    .await;
    assert_eq!(again.status(), StatusCode::NOT_MODIFIED);
    // Plus large que l'originale : l'originale.
    let wide = raw(&f.app, &format!("/api/covers/{id}?w=1080&f=jpeg"), None).await;
    assert_eq!(wide.headers()["content-type"], "image/png");
    for bad in [
        "/api/covers/zz".to_owned(),
        format!("/api/covers/{}", "0".repeat(64)),
        format!("/api/covers/{id}?w=333"),
    ] {
        assert_ne!(
            raw(&f.app, &bad, None).await.status(),
            StatusCode::OK,
            "{bad}"
        );
    }

    // Une seule couverture choisie : une page remplace l'image ; une image inconnue est refusée.
    let uri = format!("/api/series/{gigant}");
    let (_, detail) = call(&f.app, "GET", &uri, None).await;
    let page = detail["volumes"][0]["cover_page_id"].clone();
    let (_, detail) = call(
        &f.app,
        "PATCH",
        &uri,
        Some(json!({ "cover_page_id": page })),
    )
    .await;
    assert_eq!(detail["metadata"]["cover_image_id"], Value::Null);
    assert_eq!(detail["metadata"]["cover_page_id"], page);
    let (status, _) = call(
        &f.app,
        "PATCH",
        &uri,
        Some(json!({ "cover_image_id": "0".repeat(64) })),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, detail) = call(&f.app, "PATCH", &uri, Some(json!({ "cover_image_id": id }))).await;
    assert_eq!(detail["metadata"]["cover_page_id"], Value::Null);
    assert!(
        f.anilist
            .calls()
            .contains(&"image https://s4.anilist.co/c.jpg".to_owned())
    );
}

/// Réponse brute (en-têtes, corps binaire), avec un `If-None-Match` éventuel.
async fn raw(app: &Router, uri: &str, etag: Option<&str>) -> axum::response::Response {
    let mut request = Request::builder().uri(uri);
    if let Some(etag) = etag {
        request = request.header("if-none-match", etag);
    }
    app.clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}
