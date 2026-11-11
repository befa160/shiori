//! `shiori serve` : API HTTP JSON sous `/api`.
//!
//! Erreurs toujours au format `{ "code", "message" }`, codes stables pour l'UI. Pas
//! d'authentification en phase 2 : écoute sur 127.0.0.1 par défaut (Q10).

use std::future::Future;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, Request, State};
use axum::http::{StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get};
use axum::{Json, Router};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shiori_core::VolumeId;
use shiori_db::{Db, LibraryError};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;

#[derive(Args)]
pub struct ServeArgs {
    /// Adresse d'écoute. Sans authentification : 0.0.0.0 ouvre l'API à tout le réseau local.
    #[arg(long, default_value = "127.0.0.1:7878")]
    pub listen: SocketAddr,
    /// Base SQLite (défaut : <dossier de données>/shiori.db, ou $SHIORI_DATA_DIR/shiori.db).
    #[arg(long, value_name = "FICHIER")]
    pub db: Option<PathBuf>,
}

/// Ouvre la base, écoute, et sert jusqu'à `shutdown` (Ctrl-C dans le binaire).
pub async fn run(
    args: ServeArgs,
    quiet: bool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let db = crate::open_db(args.db).await?;
    let listener = TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("écoute sur {}", args.listen))?;
    if !quiet {
        eprintln!("Shiori écoute sur http://{}", listener.local_addr()?);
    }
    serve(listener, Arc::new(db), shutdown).await
}

pub async fn serve(
    listener: TcpListener,
    db: Arc<Db>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let local_only = listener.local_addr()?.ip().is_loopback();
    axum::serve(listener, router(db, local_only))
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}

/// `local_only` : n'accepte que les requêtes adressées à `localhost` (voir [`local_host_only`]).
pub fn router(db: Arc<Db>, local_only: bool) -> Router {
    let router = Router::new()
        .route("/api/health", get(health))
        .route("/api/libraries", get(libraries).post(create_library))
        .route("/api/libraries/{id}", delete(delete_library))
        .route("/api/libraries/{id}/series", get(library_series))
        .route("/api/series/{id}", get(series))
        .route("/api/volumes/{id}", get(volume))
        .route("/api/volumes/{id}/pages", get(pages))
        .fallback(async || ApiError::new(StatusCode::NOT_FOUND, "not_found", "route inconnue"))
        .method_not_allowed_fallback(async || {
            ApiError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "méthode non autorisée sur cette route",
            )
        })
        .layer(TraceLayer::new_for_http())
        .with_state(db);
    match local_only {
        true => router.layer(middleware::from_fn(local_host_only)),
        false => router,
    }
}

/// Parade au DNS rebinding : sans elle, une page web malveillante peut faire pointer son
/// propre domaine vers 127.0.0.1 et piloter l'API depuis le navigateur. En écoute locale, le
/// `Host` doit donc être local. En écoute réseau (`0.0.0.0`), aucune protection tant qu'il
/// n'y a pas d'authentification.
async fn local_host_only(request: Request, next: Next) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
        .map_or(host, |(name, _)| name);
    match name {
        "localhost" | "127.0.0.1" | "[::1]" => next.run(request).await,
        _ => ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_host",
            "en écoute locale, l'API ne répond qu'à localhost",
        )
        .into_response(),
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
}

impl ApiError {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }

    fn not_found(what: &str) -> Self {
        Self::new(
            StatusCode::NOT_FOUND,
            "not_found",
            format!("{what} introuvable"),
        )
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = Json(json!({ "code": self.code, "message": self.message }));
        (self.status, body).into_response()
    }
}

impl From<shiori_db::Error> for ApiError {
    fn from(e: shiori_db::Error) -> Self {
        tracing::error!("erreur de base : {e}");
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal",
            "erreur interne",
        )
    }
}

impl From<LibraryError> for ApiError {
    fn from(e: LibraryError) -> Self {
        let (status, code) = match &e {
            LibraryError::EmptyName => (StatusCode::BAD_REQUEST, "invalid_name"),
            LibraryError::NotADirectory(_) => (StatusCode::BAD_REQUEST, "invalid_root"),
            LibraryError::NameTaken(_) => (StatusCode::CONFLICT, "name_taken"),
            LibraryError::Overlaps(..) => (StatusCode::CONFLICT, "root_overlaps"),
            LibraryError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        if status.is_server_error() {
            tracing::error!("erreur de base : {e}");
            return Self::new(status, code, "erreur interne");
        }
        Self::new(status, code, e.to_string())
    }
}

type ApiResult<T> = Result<Json<T>, ApiError>;

fn int_id(raw: &str) -> Result<i64, ApiError> {
    raw.parse().map_err(|_| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_id",
            "identifiant invalide",
        )
    })
}

fn volume_id(raw: &str) -> Result<VolumeId, ApiError> {
    raw.parse()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_id", format!("{e}")))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok", "version": env!("CARGO_PKG_VERSION") }))
}

async fn libraries(State(db): State<Arc<Db>>) -> ApiResult<Vec<shiori_db::LibrarySummary>> {
    Ok(Json(db.library_summaries().await?))
}

#[derive(Deserialize)]
struct NewLibrary {
    name: String,
    root: PathBuf,
}

#[derive(Serialize)]
struct CreatedLibrary {
    id: i64,
    name: String,
    root: String,
}

async fn create_library(
    State(db): State<Arc<Db>>,
    body: Result<Json<NewLibrary>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedLibrary>), ApiError> {
    let Json(new) =
        body.map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", e.body_text()))?;
    let library = db.create_library(&new.name, &new.root).await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedLibrary {
            id: library.id,
            name: library.name,
            root: library.root,
        }),
    ))
}

async fn delete_library(
    State(db): State<Arc<Db>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match db.delete_library(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("bibliothèque")),
    }
}

async fn library_series(
    State(db): State<Arc<Db>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    db.series_of(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("bibliothèque"))
}

async fn series(
    State(db): State<Arc<Db>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    db.series_detail(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn volume(
    State(db): State<Arc<Db>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::VolumeDetail> {
    db.volume_detail(&volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

async fn pages(
    State(db): State<Arc<Db>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::PageInfo>> {
    db.pages_of(&volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}
