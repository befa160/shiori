//! Service des images : pages (originales ou redimensionnées), couvertures, vignettes des
//! sources ; ETag et cache permanent (principes 2, 4, 6 et 7).

use std::future::Future;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use axum::extract::rejection::QueryRejection;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use shiori_core::{Archive, PageId, PageStatus, open_archive as open_file};
use shiori_db::{PageFile, PageSource};
use shiori_meta::FetchError;

use super::auth::{Admin, Me};
use super::{ApiError, App, invalid_query, source_id};
use crate::cache::Images;
use crate::meta::MetaError;
use crate::render::{self, Format, RENDER_VERSION, WIDTHS};

/// Une page ne change jamais sous un même ID (principe 2) : cache permanent.
pub(crate) const IMMUTABLE: &str = "private, max-age=31536000, immutable";

#[derive(Deserialize)]
pub(super) struct ImageQuery {
    w: Option<String>,
    f: Option<String>,
}

/// Largeur et format demandés, validés contre les listes fermées ; `None` = originale.
fn wanted(query: &ImageQuery) -> Result<Option<(u32, Format)>, ApiError> {
    let Some(w) = query.w.as_deref() else {
        return match query.f {
            None => Ok(None),
            Some(_) => Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_width",
                "le format `f` demande une largeur `w`",
            )),
        };
    };
    let width = w
        .parse()
        .ok()
        .filter(|w| WIDTHS.contains(w))
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_width",
                format!("largeur parmi {WIDTHS:?}"),
            )
        })?;
    let format = match query.f.as_deref() {
        None | Some("avif") => Format::Avif,
        Some("jpeg") => Format::Jpeg,
        Some(_) => {
            return Err(ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_format",
                "format `avif` ou `jpeg`",
            ));
        }
    };
    Ok(Some((width, format)))
}

/// Page lue par son `archive_index` (principes 1, 4 et 6), originale ou redimensionnée
/// (`?w=&f=`, principe 7). Jamais une autre page à la place : toute incohérence est une
/// erreur explicite.
pub(super) async fn page_image(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    query: Result<Query<ImageQuery>, QueryRejection>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let id: PageId = id
        .parse()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", format!("{e}")))?;
    let Query(query) = query
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_query", e.body_text()))?;
    let wanted = wanted(&query)?;
    let page =
        app.db.page_source(&me, &id).await?.ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, "page_not_found", "page inconnue")
        })?;
    if page.status == PageStatus::Broken {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_broken",
            "page illisible dans l'archive (voir les problèmes du tome)",
        ));
    }
    // Pas de décodeur AVIF pur Rust (Q2), et on n'agrandit jamais : l'originale est servie,
    // sauf un BMP (non compressé, plusieurs Mo), réencodé à sa propre largeur.
    let bmp = page.media_type.as_deref() == Some("image/bmp");
    let resize = wanted.and_then(|(w, f)| match page.width {
        _ if page.media_type.as_deref() == Some("image/avif") => None,
        Some(pw) if w >= pw => bmp.then_some((pw, f)),
        _ => Some((w, f)),
    });
    let etag = match resize {
        Some((w, f)) => format!("\"{id}-w{w}-{}-v{RENDER_VERSION}\"", f.extension()),
        None => format!("\"{id}\""),
    };
    if etag_matches(&headers, &etag) {
        let cache = [
            (header::ETAG, etag),
            (header::CACHE_CONTROL, IMMUTABLE.into()),
        ];
        return Ok((StatusCode::NOT_MODIFIED, cache).into_response());
    }
    let (path, stamp) = match &page.file {
        PageFile::Ready(path, stamp) => (path.clone(), *stamp),
        PageFile::Changed => {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "volume_changed",
                "le fichier a changé depuis le scan : à rescanner",
            ));
        }
        PageFile::Missing => {
            return Err(ApiError::new(
                StatusCode::GONE,
                "volume_unavailable",
                "aucun fichier connu de ce tome n'existe encore",
            ));
        }
    };
    let media_type = page.media_type.clone();
    let original = async {
        let app = app.clone();
        tokio::task::spawn_blocking(move || read_page(&app.images, &path, stamp, &page))
            .await
            .map_err(|_| corrupt("lecture interrompue"))?
    };
    let (bytes, media_type) = match resize {
        None => {
            let media_type = media_type.unwrap_or_else(|| "application/octet-stream".into());
            (original.await?, media_type)
        }
        Some((w, f)) => (
            resized(&app, id, w, f, original).await?,
            f.media_type().into(),
        ),
    };
    let headers = [
        (header::CONTENT_TYPE, media_type),
        (header::CACHE_CONTROL, IMMUTABLE.into()),
        (header::ETAG, etag),
        // Le contenu vient d'une archive extérieure : le navigateur ne doit pas deviner son type.
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff".into()),
    ];
    Ok((headers, bytes).into_response())
}

/// Rendu en cache, sinon calculé (nombre de rendus simultanés borné) puis mis en cache.
async fn resized(
    app: &Arc<App>,
    id: PageId,
    width: u32,
    format: Format,
    original: impl Future<Output = Result<Vec<u8>, ApiError>>,
) -> Result<Vec<u8>, ApiError> {
    let cache = app.clone();
    let hit = tokio::task::spawn_blocking(move || cache.images.get(&id, width, format)).await;
    if let Ok(Some(bytes)) = hit {
        return Ok(bytes);
    }
    // Jeton d'abord : au plus `renders` originales en mémoire à la fois, pas une par requête.
    let _permit = app.renders.acquire().await.expect("sémaphore jamais fermé");
    let original = original.await?;
    let cache = app.clone();
    let failed = |e: String| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "render_failed", e);
    tokio::task::spawn_blocking(move || {
        let bytes = render::render(&original, width, format)?;
        // Un cache plein ou en lecture seule ne doit pas empêcher de servir la page.
        if let Err(e) = cache.images.put(&id, width, format, &bytes) {
            tracing::warn!("cache d'images : {e}");
        }
        Ok(bytes)
    })
    .await
    .map_err(|_| failed("rendu interrompu".into()))?
    .map_err(failed)
}

fn corrupt(message: impl Into<String>) -> ApiError {
    ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "page_corrupt", message)
}

/// Lit l'entrée et vérifie que c'est bien la page enregistrée : même nom brut, même CRC.
/// Le CRC est revérifié à la lecture. Un RAR solid est servi depuis son extraction en cache.
fn read_page(
    images: &Images,
    path: &std::path::Path,
    stamp: (i64, i64),
    page: &PageSource,
) -> Result<Vec<u8>, ApiError> {
    let index = page.archive_index;
    let archive = open_archive(path, stamp)?;
    let mut archive = archive.lock().unwrap_or_else(PoisonError::into_inner);
    let same = archive
        .entries()
        .get(index)
        .is_some_and(|e| e.name_raw == page.entry_name_raw && e.crc32 == page.crc32);
    if !same {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "page_mismatch",
            "l'archive ne contient plus cette page à sa place : à rescanner",
        ));
    }
    if archive.is_solid() {
        return images
            .extracted(&page.volume_id, index, page.size, |dir| {
                extract_all(&mut **archive, dir)
            })
            .map_err(|e| corrupt(e.to_string()));
    }
    archive
        .read_entry(index)
        .map_err(|e| corrupt(e.to_string()))
}

/// Toutes les pages lisibles d'un tome, en une passe, dans `dir/<index>`. Une page illisible
/// manque simplement : sa lecture échouera avec une erreur explicite.
fn extract_all(archive: &mut dyn Archive, dir: &std::path::Path) -> std::io::Result<()> {
    let files: Vec<usize> = archive
        .entries()
        .iter()
        .filter(|e| e.kind == shiori_core::EntryKind::File)
        .map(|e| e.index)
        .collect();
    let mut written = Ok(());
    archive.read_each(&files, &mut |index, bytes| {
        if let (Ok(bytes), Ok(())) = (bytes, &written) {
            written = std::fs::write(dir.join(index.to_string()), bytes);
        }
    });
    written
}

/// Archive ouverte, partagée entre les requêtes d'un même tome.
type OpenArchive = Arc<Mutex<Box<dyn Archive + Send>>>;

/// Annuaires centraux ouverts récemment, indexés par (chemin, taille, date) : un fichier
/// modifié puis rescanné n'est jamais lu avec son ancien annuaire. Ouvrir celui d'un webtoon
/// de 12 760 pages coûte ~11 ms par requête (mesuré au lot 2.4), lire la page ~1 ms.
type OpenArchives = Vec<(PathBuf, (i64, i64), OpenArchive)>;
static OPEN_ARCHIVES: Mutex<OpenArchives> = Mutex::new(Vec::new());
const MAX_OPEN_ARCHIVES: usize = 4;

fn open_archive(path: &std::path::Path, stamp: (i64, i64)) -> Result<OpenArchive, ApiError> {
    let lock = || OPEN_ARCHIVES.lock().unwrap_or_else(PoisonError::into_inner);
    {
        let mut open = lock();
        if let Some(i) = open.iter().position(|(p, s, _)| p == path && *s == stamp) {
            let hit = open.remove(i);
            let archive = hit.2.clone();
            open.push(hit);
            return Ok(archive);
        }
    }
    // Ouverture hors du verrou : les autres tomes restent servis pendant ce temps.
    let archive = Arc::new(Mutex::new(
        open_file(path).map_err(|e| corrupt(e.to_string()))?,
    ));
    let mut open = lock();
    open.retain(|(p, _, _)| p != path);
    open.push((path.to_path_buf(), stamp, archive.clone()));
    if open.len() > MAX_OPEN_ARCHIVES {
        open.remove(0);
    }
    Ok(archive)
}

/// `If-None-Match` : liste d'ETags, éventuellement faibles (`W/`), ou `*`.
pub(crate) fn etag_matches(headers: &HeaderMap, etag: &str) -> bool {
    headers
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|v: &HeaderValue| v.to_str().ok())
        .flat_map(|v| v.split(','))
        .map(|t| t.trim().trim_start_matches("W/"))
        .any(|t| t == etag || t == "*")
}

/// Vignette d'une fiche, relayée ; gardée un jour par le navigateur.
pub(super) async fn thumbnail(
    _: Admin,
    State(app): State<Arc<App>>,
    Path((source, id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let bytes = app.metadata.thumbnail(source_id(&source)?, &id).await?;
    let media_type = match imagesize::image_type(&bytes) {
        Ok(imagesize::ImageType::Png) => "image/png",
        Ok(imagesize::ImageType::Webp) => "image/webp",
        Ok(imagesize::ImageType::Jpeg) => "image/jpeg",
        _ => return Err(MetaError::from(FetchError::BadResponse("pas une image".into())).into()),
    };
    let headers = [
        (header::CONTENT_TYPE, media_type),
        (header::CACHE_CONTROL, "private, max-age=86400"),
    ];
    Ok((headers, bytes).into_response())
}

/// Couverture gardée, originale ou redimensionnée (largeurs fixes) ; URL immuable : son
/// identifiant est le hash de son contenu.
pub(super) async fn cover(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    query: Result<Query<ImageQuery>, QueryRejection>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let Query(query) = query.map_err(invalid_query)?;
    let wanted = wanted(&query)?;
    // L'identifiant est un hash hexadécimal : rien d'autre n'est cherché.
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::not_found("couverture"));
    }
    if !app.db.cover_visible(&me, &id).await? {
        return Err(ApiError::not_found("couverture"));
    }
    let found = app
        .db
        .cover(&id)
        .await?
        .ok_or_else(|| ApiError::not_found("couverture"))?;
    // On n'agrandit jamais : au-delà de sa largeur, l'originale.
    let resize = wanted.filter(|&(w, _)| w < found.width);
    let etag = match resize {
        Some((w, f)) => format!("\"{id}-w{w}-{}-v{RENDER_VERSION}\"", f.extension()),
        None => format!("\"{id}\""),
    };
    let cache = [
        (header::ETAG, etag.clone()),
        (header::CACHE_CONTROL, IMMUTABLE.into()),
    ];
    if etag_matches(&headers, &etag) {
        return Ok((StatusCode::NOT_MODIFIED, cache).into_response());
    }
    // shortcut: rendu à chaque demande, sans cache disque (couvertures de 500 px au plus, gardées par le navigateur) ; passer par `Images` si le CPU le demande.
    let (bytes, media_type) = match resize {
        None => (found.bytes, found.media_type.to_owned()),
        Some((w, f)) => {
            let _permit = app.renders.acquire().await.expect("sémaphore jamais fermé");
            let failed =
                |e: String| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "render_failed", e);
            let bytes = tokio::task::spawn_blocking(move || render::render(&found.bytes, w, f))
                .await
                .map_err(|_| failed("rendu interrompu".into()))?
                .map_err(failed)?;
            (bytes, f.media_type().to_owned())
        }
    };
    Ok(([(header::CONTENT_TYPE, media_type)], cache, bytes).into_response())
}
