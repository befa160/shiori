//! Pages redimensionnées (`?w=&f=`) : la bonne page, à la bonne taille, depuis le cache
//! quand elle y est, et une erreur explicite sinon.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use axum::response::Response;
use http_body_util::BodyExt;
use serde_json::Value;
use shiori::cache::Images;
use shiori::server::{self, App};
use shiori_core::ScanOptions;
use shiori_db::Db;
use shiori_testkit::{STORED, png, write_zip};
use tower::ServiceExt;

/// Couleurs très espacées : même après JPEG, la plus proche désigne la bonne page.
fn color(n: usize) -> [u8; 3] {
    [(n * 20) as u8, 255 - (n * 20) as u8, 90]
}

const PAGES: usize = 11;

struct Fixture {
    dir: tempfile::TempDir,
    db: Arc<Db>,
    app: Router,
}

impl Fixture {
    async fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
        let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
        let app = server::router(Arc::new(App::new(db.clone(), images)), None);
        Self { dir, db, app }
    }

    /// Archive rangée dans le désordre (`1, 10, 11, 2…`), pages de 640×960.
    fn unordered(&self) -> PathBuf {
        let order = [1, 10, 11, 2, 3, 4, 5, 6, 7, 8, 9];
        let pages: Vec<(String, Vec<u8>)> = order
            .iter()
            .map(|&n| (format!("{n}.png"), png(640, 960, color(n - 1))))
            .collect();
        let files: Vec<_> = pages
            .iter()
            .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
            .collect();
        write_zip(self.dir.path(), "desordre.cbz", &files)
    }

    async fn pages(&self, path: &Path) -> Vec<Value> {
        let run = self.db.start_run("/", "{}").await.unwrap();
        let scanned = self.db.scan(run, path, ScanOptions::default(), false);
        let volume = scanned.await.unwrap().report.volume_id.unwrap();
        let response = self
            .get(&format!("/api/volumes/{volume}/pages"), None)
            .await;
        serde_json::from_slice(&body(response).await).unwrap()
    }

    async fn get(&self, uri: &str, if_none_match: Option<&str>) -> Response {
        let mut request = Request::get(uri);
        if let Some(tag) = if_none_match {
            request = request.header(header::IF_NONE_MATCH, tag);
        }
        let request = request.body(Body::empty()).unwrap();
        self.app.clone().oneshot(request).await.unwrap()
    }

    async fn error(&self, uri: &str) -> (StatusCode, String) {
        let response = self.get(uri, None).await;
        let status = response.status();
        let body: Value = serde_json::from_slice(&body(response).await).unwrap();
        (status, body["code"].as_str().unwrap().to_owned())
    }

    fn cached(&self, page: &str, file: &str) -> PathBuf {
        self.dir
            .path()
            .join("cache/pages")
            .join(&page[..2])
            .join(file)
    }
}

async fn body(response: Response) -> Vec<u8> {
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    bytes.to_vec()
}

fn nearest(bytes: &[u8]) -> (usize, (u32, u32)) {
    let image = image::load_from_memory(bytes).unwrap().to_rgb8();
    let got = image.get_pixel(image.width() / 2, image.height() / 2).0;
    let distance = |n: usize| -> u32 {
        got.iter()
            .zip(color(n))
            .map(|(g, e)| u32::from(g.abs_diff(e)))
            .sum()
    };
    let n = (0..PAGES).min_by_key(|&n| distance(n)).unwrap();
    (n, image.dimensions())
}

/// L'invariant de lecture tient aussi pour les pages redimensionnées.
#[tokio::test]
async fn resized_pages_are_the_right_pages_at_the_right_size() {
    let f = Fixture::new().await;
    let pages = f.pages(&f.unordered()).await;
    assert_eq!(pages.len(), PAGES);
    for page in &pages {
        let id = page["id"].as_str().unwrap();
        let response = f
            .get(&format!("/api/pages/{id}/image?w=320&f=jpeg"), None)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/jpeg");
        assert_eq!(
            response.headers()[header::ETAG],
            format!("\"{id}-w320-jpg-v1\"").as_str()
        );
        assert_eq!(
            response.headers()[header::CACHE_CONTROL],
            "public, max-age=31536000, immutable"
        );
        let ordinal = page["ordinal"].as_u64().unwrap() as usize;
        assert_eq!(nearest(&body(response).await), (ordinal, (320, 480)));
    }
}

#[tokio::test]
async fn avif_by_default_with_the_requested_width() {
    let f = Fixture::new().await;
    let id = f.pages(&f.unordered()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let response = f.get(&format!("/api/pages/{id}/image?w=160"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/avif");
    let bytes = body(response).await;
    let size = imagesize::blob_size(&bytes).unwrap();
    assert_eq!((size.width, size.height), (160, 240));
    assert!(f.cached(&id, &format!("{id}-w160-v1.avif")).exists());
}

/// Une page déjà plus étroite que demandé n'est jamais agrandie : l'originale est servie.
#[tokio::test]
async fn narrow_pages_are_served_as_they_are() {
    let f = Fixture::new().await;
    let id = f.pages(&f.unordered()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let original = body(f.get(&format!("/api/pages/{id}/image"), None).await).await;
    for w in [640, 800, 2048] {
        let response = f
            .get(&format!("/api/pages/{id}/image?w={w}&f=jpeg"), None)
            .await;
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(
            response.headers()[header::ETAG],
            format!("\"{id}\"").as_str()
        );
        assert_eq!(body(response).await, original, "w={w}");
    }
}

/// Pas de décodeur AVIF pur Rust : une page AVIF est servie telle quelle, même large.
#[tokio::test]
async fn avif_sources_are_served_as_they_are() {
    let f = Fixture::new().await;
    let path = write_zip(
        f.dir.path(),
        "avif.cbz",
        &[("01.avif", shiori_testkit::AVIF_8X12, STORED)],
    );
    let id = f.pages(&path).await[0]["id"].as_str().unwrap().to_owned();
    // On fait croire que la page est large, pour que seule la règle AVIF décide.
    sqlx::query("UPDATE pages SET width = 1000 WHERE id = ?")
        .bind(&id)
        .execute(f.db.pool())
        .await
        .unwrap();
    let response = f.get(&format!("/api/pages/{id}/image?w=320"), None).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "image/avif");
    assert_eq!(body(response).await, shiori_testkit::AVIF_8X12);
}

#[tokio::test]
async fn second_request_comes_from_the_cache_and_304_works() {
    let f = Fixture::new().await;
    let id = f.pages(&f.unordered()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let uri = format!("/api/pages/{id}/image?w=320&f=jpeg");
    assert_eq!(f.get(&uri, None).await.status(), StatusCode::OK);
    // Le fichier du cache est servi tel quel, sans nouveau rendu.
    std::fs::write(
        f.cached(&id, &format!("{id}-w320-v1.jpg")),
        b"depuis le cache",
    )
    .unwrap();
    assert_eq!(body(f.get(&uri, None).await).await, b"depuis le cache");

    let tag = format!("\"{id}-w320-jpg-v1\"");
    let cached = f.get(&uri, Some(&tag)).await;
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    // L'ETag de l'originale ne vaut pas pour la version redimensionnée.
    let other = f.get(&uri, Some(&format!("\"{id}\""))).await;
    assert_eq!(other.status(), StatusCode::OK);
}

#[tokio::test]
async fn invalid_parameters() {
    let f = Fixture::new().await;
    let id = f.pages(&f.unordered()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    for (query, code) in [
        ("w=333", "invalid_width"),
        ("w=abc", "invalid_width"),
        ("w=-1", "invalid_width"),
        ("f=jpeg", "invalid_width"),
        ("w=320&f=webp", "invalid_format"),
        ("w=320&w=640", "invalid_query"),
    ] {
        let got = f.error(&format!("/api/pages/{id}/image?{query}")).await;
        assert_eq!(got, (StatusCode::BAD_REQUEST, code.to_owned()), "{query}");
    }
}

/// En-tête lisible au scan, contenu tronqué : le rendu échoue, explicitement.
#[tokio::test]
async fn undecodable_page_is_a_render_error() {
    let f = Fixture::new().await;
    let full = png(800, 1200, color(0));
    let path = write_zip(
        f.dir.path(),
        "tronque.cbz",
        &[("01.png", &full[..full.len() / 2], STORED)],
    );
    let page = &f.pages(&path).await[0];
    assert_eq!(page["status"], "ok");
    let id = page["id"].as_str().unwrap();
    assert_eq!(
        f.error(&format!("/api/pages/{id}/image?w=320&f=jpeg"))
            .await,
        (StatusCode::INTERNAL_SERVER_ERROR, "render_failed".into())
    );
}

/// Un cache impossible à écrire n'empêche pas de servir la page.
#[tokio::test]
async fn cache_write_failure_still_serves_the_page() {
    let f = Fixture::new().await;
    let id = f.pages(&f.unordered()).await[0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    std::fs::write(f.dir.path().join("cache/pages").join(&id[..2]), b"").unwrap();
    let response = f
        .get(&format!("/api/pages/{id}/image?w=320&f=jpeg"), None)
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(nearest(&body(response).await).1, (320, 480));
}
