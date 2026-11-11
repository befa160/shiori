//! Pages servies par HTTP : la bonne image à chaque ordinal, en-têtes immuables, et une
//! erreur explicite pour chaque incohérence (principes 1, 2 et 4).

use std::fs::{File, FileTimes};
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
use shiori_testkit::{STORED, fixtures, page_color, png, replace_all, write_zip};
use tower::ServiceExt;

struct Fixture {
    dir: tempfile::TempDir,
    db: Arc<Db>,
    app: Router,
}

async fn fixture() -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let db = Arc::new(Db::open(&dir.path().join("t.db")).await.unwrap());
    let images = Images::open(dir.path().join("cache"), u64::MAX).unwrap();
    let app = server::router(Arc::new(App::new(db.clone(), images)), false);
    Fixture { dir, db, app }
}

impl Fixture {
    /// Scanne un fichier et renvoie l'ID de son tome.
    async fn scan(&self, path: &Path) -> String {
        let run = self.db.start_run("/", "{}").await.unwrap();
        let scanned = self
            .db
            .scan(run, path, ScanOptions::default(), false)
            .await
            .unwrap();
        scanned.report.volume_id.unwrap().to_string()
    }

    async fn request(&self, uri: &str, if_none_match: Option<&str>) -> Response {
        let mut request = Request::get(uri);
        if let Some(tag) = if_none_match {
            request = request.header(header::IF_NONE_MATCH, tag);
        }
        self.app
            .clone()
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap()
    }

    async fn pages(&self, volume: &str) -> Vec<Value> {
        let response = self
            .request(&format!("/api/volumes/{volume}/pages"), None)
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice::<Vec<Value>>(&bytes).unwrap()
    }

    async fn image(&self, page: &str) -> (StatusCode, Response) {
        let response = self
            .request(&format!("/api/pages/{page}/image"), None)
            .await;
        (response.status(), response)
    }

    /// Code d'erreur JSON renvoyé pour une page.
    async fn error(&self, page: &str) -> (StatusCode, String) {
        let (status, response) = self.image(page).await;
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: Value = serde_json::from_slice(&bytes).unwrap();
        (status, body["code"].as_str().unwrap().to_owned())
    }
}

async fn body(response: Response) -> Vec<u8> {
    response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec()
}

/// Ordinal dont la couleur (`page_color`) est la plus proche du pixel en haut à gauche.
/// Exact pour le PNG ; le plus proche pour le JPEG, qui est avec pertes. Deux pages voisines
/// ne diffèrent que d'une unité : une tolérance fixe laisserait passer une page voisine.
fn ordinal_of(bytes: &[u8], pages: usize) -> usize {
    let image = image::load_from_memory(bytes).unwrap().to_rgb8();
    let got = image.get_pixel(0, 0).0;
    let distance = |n: usize| -> u32 {
        got.iter()
            .zip(page_color(n))
            .map(|(g, e)| u32::from(g.abs_diff(e)))
            .sum()
    };
    (0..pages).min_by_key(|&n| distance(n)).unwrap()
}

/// Remet la date de modification : le fichier « n'a pas changé » pour la détection rapide.
fn restore_mtime(path: &Path, mtime: std::time::SystemTime) {
    let file = File::options().write(true).open(path).unwrap();
    file.set_times(FileTimes::new().set_modified(mtime))
        .unwrap();
}

fn mtime(path: &Path) -> std::time::SystemTime {
    std::fs::metadata(path).unwrap().modified().unwrap()
}

/// L'invariant de lecture : la page n servie par HTTP est l'image de l'ordinal n, même
/// quand l'archive est rangée dans le désordre (`1, 10, 11, 2…`).
#[tokio::test]
async fn every_page_served_over_http_is_the_right_image() {
    let f = fixture().await;
    for path in [
        fixtures::unpadded(f.dir.path()),
        fixtures::webtoon(f.dir.path()),
        fixtures::manga(f.dir.path()),
    ] {
        let volume = f.scan(&path).await;
        let pages = f.pages(&volume).await;
        assert!(!pages.is_empty());
        for page in &pages {
            let ordinal = page["ordinal"].as_u64().unwrap() as usize;
            let (status, response) = f.image(page["id"].as_str().unwrap()).await;
            assert_eq!(status, StatusCode::OK);
            assert_eq!(
                response.headers()[header::CONTENT_TYPE],
                page["media_type"].as_str().unwrap()
            );
            let lossless = page["media_type"] != "image/jpeg";
            let bytes = body(response).await;
            assert_eq!(
                ordinal_of(&bytes, pages.len()),
                ordinal,
                "{}",
                path.display()
            );
            if lossless {
                let image = image::load_from_memory(&bytes).unwrap().to_rgb8();
                assert_eq!(image.get_pixel(0, 0).0, page_color(ordinal));
            }
        }
    }
}

#[tokio::test]
async fn images_are_immutable_with_a_strong_etag() {
    let f = fixture().await;
    let volume = f.scan(&fixtures::unpadded(f.dir.path())).await;
    let page = f.pages(&volume).await[0]["id"].as_str().unwrap().to_owned();
    let uri = format!("/api/pages/{page}/image");

    let response = f.request(&uri, None).await;
    let etag = response.headers()[header::ETAG]
        .to_str()
        .unwrap()
        .to_owned();
    assert_eq!(etag, format!("\"{page}\""));
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );

    for tag in [etag.clone(), format!("\"autre\", W/{etag}"), "*".to_owned()] {
        let cached = f.request(&uri, Some(&tag)).await;
        assert_eq!(cached.status(), StatusCode::NOT_MODIFIED, "{tag}");
        assert_eq!(cached.headers()[header::ETAG], etag.as_str());
        assert!(body(cached).await.is_empty());
    }
    let other = f.request(&uri, Some("\"autre\"")).await;
    assert_eq!(other.status(), StatusCode::OK);
}

#[tokio::test]
async fn unknown_or_malformed_ids() {
    let f = fixture().await;
    let unknown = "0".repeat(32);
    assert_eq!(
        f.error(&unknown).await,
        (StatusCode::NOT_FOUND, "page_not_found".into())
    );
    assert_eq!(
        f.error("pas-un-id").await,
        (StatusCode::BAD_REQUEST, "invalid_id".into())
    );
}

#[tokio::test]
async fn a_broken_page_is_an_explicit_error_never_another_page() {
    let f = fixture().await;
    let volume = f.scan(&fixtures::bad_crc(f.dir.path())).await;
    let pages = f.pages(&volume).await;
    assert_eq!(pages[1]["status"], "broken");
    assert_eq!(
        f.error(pages[1]["id"].as_str().unwrap()).await,
        (StatusCode::UNPROCESSABLE_ENTITY, "page_broken".into())
    );
    // La page saine voisine reste servie.
    assert_eq!(
        f.image(pages[0]["id"].as_str().unwrap()).await.0,
        StatusCode::OK
    );
}

/// Un CBZ d'une page dont le contenu contient un marqueur modifiable sans toucher à l'image.
fn marked(dir: &Path) -> PathBuf {
    let mut page = png(8, 12, page_color(0));
    page.extend_from_slice(b"SHIORI-MARKER-OK!");
    write_zip(dir, "marque.cbz", &[("01.png", &page, STORED)])
}

async fn first_page(f: &Fixture, path: &Path) -> String {
    let volume = f.scan(path).await;
    f.pages(&volume).await[0]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn file_changed_since_the_scan() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let page = first_page(&f, &path).await;
    restore_mtime(&path, mtime(&path) + std::time::Duration::from_secs(60));
    assert_eq!(
        f.error(&page).await,
        (StatusCode::CONFLICT, "volume_changed".into())
    );
}

#[tokio::test]
async fn file_gone() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let page = first_page(&f, &path).await;
    std::fs::remove_file(&path).unwrap();
    assert_eq!(
        f.error(&page).await,
        (StatusCode::GONE, "volume_unavailable".into())
    );
}

/// Même taille, même date, mais l'entrée a changé de nom : on ne sert pas « une » page.
#[tokio::test]
async fn entry_replaced_behind_our_back() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let page = first_page(&f, &path).await;
    let before = mtime(&path);
    replace_all(&path, b"01.png", b"02.png");
    restore_mtime(&path, before);
    assert_eq!(
        f.error(&page).await,
        (StatusCode::CONFLICT, "page_mismatch".into())
    );
}

/// Même taille, même date, même nom, mais le contenu ne correspond plus au CRC.
#[tokio::test]
async fn content_corrupted_behind_our_back() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let page = first_page(&f, &path).await;
    let before = mtime(&path);
    replace_all(&path, b"SHIORI-MARKER-OK!", b"SHIORI-MARKER-KO!");
    restore_mtime(&path, before);
    assert_eq!(
        f.error(&page).await,
        (StatusCode::INTERNAL_SERVER_ERROR, "page_corrupt".into())
    );
}

/// Même taille, même date, mais ce n'est plus une archive : erreur explicite.
#[tokio::test]
async fn archive_destroyed_behind_our_back() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let page = first_page(&f, &path).await;
    let before = mtime(&path);
    let size = std::fs::metadata(&path).unwrap().len() as usize;
    std::fs::write(&path, vec![0u8; size]).unwrap();
    restore_mtime(&path, before);
    assert_eq!(
        f.error(&page).await,
        (StatusCode::INTERNAL_SERVER_ERROR, "page_corrupt".into())
    );
}

/// Deux copies du même fichier : si l'une a disparu, l'autre sert la page.
#[tokio::test]
async fn another_copy_of_the_volume_is_used() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let copy = f.dir.path().join("copie.cbz");
    std::fs::copy(&path, &copy).unwrap();
    let page = first_page(&f, &path).await;
    assert_eq!(first_page(&f, &copy).await, page);
    std::fs::remove_file(&path).unwrap();
    assert_eq!(f.image(&page).await.0, StatusCode::OK);
}

/// Fichier modifié puis rescanné : ses nouvelles pages ne sont jamais lues avec l'annuaire
/// gardé en mémoire de l'ancienne version, et les anciennes pages n'existent plus nulle part.
#[tokio::test]
async fn a_rescanned_file_is_never_read_with_its_old_directory() {
    let f = fixture().await;
    let path = marked(f.dir.path());
    let old = first_page(&f, &path).await;
    assert_eq!(f.image(&old).await.0, StatusCode::OK);

    let pages: Vec<(String, Vec<u8>)> = (0..3)
        .map(|i| (format!("{:02}.png", 9 - i), png(8, 12, page_color(i + 5))))
        .collect();
    let files: Vec<_> = pages
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_slice(), STORED))
        .collect();
    write_zip(f.dir.path(), "marque.cbz", &files);
    restore_mtime(&path, mtime(&path) + std::time::Duration::from_secs(60));
    let volume = f.scan(&path).await;

    assert_eq!(
        f.error(&old).await,
        (StatusCode::GONE, "volume_unavailable".into())
    );
    let pages = f.pages(&volume).await;
    for page in &pages {
        let (status, response) = f.image(page["id"].as_str().unwrap()).await;
        assert_eq!(status, StatusCode::OK);
        let ordinal = page["ordinal"].as_u64().unwrap() as usize;
        assert_eq!(ordinal_of(&body(response).await, 10), 7 - ordinal);
    }
}
