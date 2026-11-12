//! Front embarqué (phase 3, lot 3.2) : `web/dist` est intégré au binaire en release et lu sur
//! le disque en debug (`rust-embed`). Le binaire compile sans `web/dist` (développement du
//! backend seul) et sert alors une page qui explique comment construire le front.

use axum::body::Body;
use axum::http::{HeaderMap, Method, StatusCode, header};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::server::{ApiError, IMMUTABLE, etag_matches};

#[derive(RustEmbed)]
#[folder = "../../../web/dist"]
#[allow_missing = true]
pub struct Dist;

/// Toute requête qu'aucune route n'a prise : erreur JSON sous `/api`, front ailleurs.
pub async fn fallback(method: Method, uri: axum::http::Uri, headers: HeaderMap) -> Response {
    let path = uri.path();
    if path == "/api" || path.starts_with("/api/") {
        return ApiError::new(StatusCode::NOT_FOUND, "not_found", "route inconnue").into_response();
    }
    if method != Method::GET && method != Method::HEAD {
        return ApiError::new(
            StatusCode::METHOD_NOT_ALLOWED,
            "method_not_allowed",
            "méthode non autorisée sur cette route",
        )
        .into_response();
    }
    serve::<Dist>(path, &headers)
}

/// Le fichier demandé, ou `index.html` pour une route de l'application (`/read/…`), que
/// React Router résout côté navigateur. Un chemin avec extension introuvable (ancien
/// `assets/index-abc.js` après une mise à jour) reste un vrai 404, jamais `index.html`.
pub fn serve<E: RustEmbed>(path: &str, headers: &HeaderMap) -> Response {
    let path = path.trim_start_matches('/');
    let app_route = path
        .rsplit('/')
        .next()
        .is_none_or(|last| !last.contains('.'));
    let (name, file) = match E::get(path).filter(|_| !path.is_empty()) {
        Some(file) => (path, file),
        None if !app_route => {
            return (StatusCode::NOT_FOUND, [(header::CACHE_CONTROL, "no-store")]).into_response();
        }
        None => match E::get("index.html") {
            Some(file) => ("index.html", file),
            None => return missing(),
        },
    };
    let etag = format!(
        "\"{}\"",
        file.metadata
            .sha256_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
    // Les noms de `assets/` contiennent un hash de leur contenu (Vite) : immuables. Le reste
    // (`index.html`, favicon) est revalidé à chaque fois, pour voir tout de suite une mise à jour.
    let cache = if name.starts_with("assets/") {
        IMMUTABLE
    } else {
        "no-cache"
    };
    if etag_matches(headers, &etag) {
        let headers = [(header::ETAG, etag), (header::CACHE_CONTROL, cache.into())];
        return (StatusCode::NOT_MODIFIED, headers).into_response();
    }
    let headers = [
        (header::CONTENT_TYPE, file.metadata.mimetype().to_owned()),
        (header::CACHE_CONTROL, cache.into()),
        (header::ETAG, etag),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
    ];
    (headers, Body::from(file.data)).into_response()
}

fn missing() -> Response {
    let page = "<!doctype html><html lang=\"fr\"><meta charset=\"utf-8\"><title>Shiori</title>\
        <body style=\"background:#121212;color:#f4f4f5;font-family:sans-serif;padding:2rem\">\
        <h1>Interface web absente</h1>\
        <p>Ce binaire a été compilé sans le front. Dans <code>web/</code> : \
        <code>npm ci &amp;&amp; npm run build</code>, puis recompilez <code>shiori</code> \
        (en debug, relancer suffit). L'API reste disponible sous <code>/api</code>.</p>";
    let headers = [
        (header::CONTENT_TYPE, "text/html; charset=utf-8"),
        (header::CACHE_CONTROL, "no-store"),
    ];
    (StatusCode::SERVICE_UNAVAILABLE, headers, page).into_response()
}

#[cfg(test)]
mod tests;
