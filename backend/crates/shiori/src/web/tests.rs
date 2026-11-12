use axum::body::to_bytes;
use axum::http::HeaderValue;

use super::*;

#[derive(RustEmbed)]
#[folder = "tests/web-fixture"]
struct Fixture;

#[derive(RustEmbed)]
#[folder = "tests/absent"]
#[allow_missing = true]
struct Absent;

async fn body(response: Response) -> String {
    String::from_utf8(
        to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap()
}

fn header(response: &Response, name: header::HeaderName) -> &str {
    response.headers()[name].to_str().unwrap()
}

#[tokio::test]
async fn index_and_app_routes_get_index_html_revalidated() {
    for path in ["/", "/read/0123abcd", "/series/4"] {
        let response = serve::<Fixture>(path, &HeaderMap::new());
        assert_eq!(response.status(), StatusCode::OK, "{path}");
        assert_eq!(header(&response, header::CONTENT_TYPE), "text/html");
        assert_eq!(header(&response, header::CACHE_CONTROL), "no-cache");
        assert!(body(response).await.contains("<div id=\"root\">"));
    }
}

#[tokio::test]
async fn hashed_assets_are_immutable_and_other_files_revalidated() {
    let asset = serve::<Fixture>("/assets/index-abc123.js", &HeaderMap::new());
    assert_eq!(asset.status(), StatusCode::OK);
    assert_eq!(header(&asset, header::CACHE_CONTROL), IMMUTABLE);
    assert_eq!(header(&asset, header::CONTENT_TYPE), "text/javascript");
    assert_eq!(header(&asset, header::X_CONTENT_TYPE_OPTIONS), "nosniff");
    assert_eq!(body(asset).await, "console.log(\"app\")\n");
    let icon = serve::<Fixture>("/favicon.svg", &HeaderMap::new());
    assert_eq!(header(&icon, header::CONTENT_TYPE), "image/svg+xml");
    assert_eq!(header(&icon, header::CACHE_CONTROL), "no-cache");
}

#[tokio::test]
async fn etag_gives_a_304() {
    let first = serve::<Fixture>("/", &HeaderMap::new());
    let etag = first.headers()[header::ETAG].clone();
    assert_eq!(etag.len(), 66, "sha256 en hexadécimal entre guillemets");
    let mut headers = HeaderMap::new();
    headers.insert(header::IF_NONE_MATCH, etag.clone());
    let cached = serve::<Fixture>("/read/x", &headers);
    assert_eq!(cached.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(cached.headers()[header::ETAG], etag);
    headers.insert(header::IF_NONE_MATCH, HeaderValue::from_static("\"autre\""));
    assert_eq!(serve::<Fixture>("/", &headers).status(), StatusCode::OK);
}

/// Un fichier introuvable n'est jamais remplacé par `index.html` : un vieux script demandé
/// après une mise à jour recevrait du HTML à la place du JavaScript.
#[tokio::test]
async fn missing_files_are_real_404s() {
    for path in [
        "/assets/index-ancien.js",
        "/robots.txt",
        "/../Cargo.toml",
        "/assets/../../Cargo.toml",
    ] {
        let response = serve::<Fixture>(path, &HeaderMap::new());
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{path}");
        assert_eq!(header(&response, header::CACHE_CONTROL), "no-store");
    }
}

#[tokio::test]
async fn without_a_built_front_the_page_says_how_to_build_it() {
    let response = serve::<Absent>("/", &HeaderMap::new());
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(header(&response, header::CACHE_CONTROL), "no-store");
    assert!(body(response).await.contains("npm run build"));
}
