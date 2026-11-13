//! `shiori serve` : API HTTP JSON sous `/api`.
//!
//! Erreurs toujours au format `{ "code", "message" }`, codes stables pour l'UI. Pas
//! d'authentification en phase 2 : écoute sur 127.0.0.1 par défaut (Q10).

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, PoisonError};

use anyhow::{Context, Result};
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shiori_core::{Archive, PageId, PageStatus, Severity, VolumeId, open_archive as open_file};
use shiori_db::{
    Db, EditError, Edited, Language, LibraryError, LibraryKind, PageFile, PageLayout, PageSource,
    PageTurn, ReadingMode, RunFilter, RunStatus, SaveProgress, SettingsChange,
};
use shiori_meta::{FetchError, Source, SourceId, WebMetadata};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::cache::Images;
use crate::job::{ScanRequest, Scanner, StartError};
use crate::meta::{IdentifyStatus, MetaError, Metadata, web_sources};
use crate::render::{self, Format, RENDER_VERSION, WIDTHS};

#[derive(Args)]
pub struct ServeArgs {
    /// Adresse d'écoute. Sans authentification : 0.0.0.0 ouvre l'API à tout le réseau local.
    #[arg(long, default_value = "127.0.0.1:7878")]
    pub listen: SocketAddr,
    /// Base SQLite (défaut : <dossier de données>/shiori.db, ou $SHIORI_DATA_DIR/shiori.db).
    #[arg(long, value_name = "FICHIER")]
    pub db: Option<PathBuf>,
    /// Cache des pages redimensionnées et des CBR solid extraits (défaut : dossier de cache de l'OS, ou $SHIORI_CACHE_DIR).
    #[arg(long, value_name = "DOSSIER")]
    pub cache_dir: Option<PathBuf>,
    /// Taille maximale de ce cache, en Mio ; au-delà, ce qui a été le moins récemment lu est effacé.
    #[arg(long, value_name = "MIO", default_value_t = 2048)]
    pub cache_max_mib: u64,
    /// Fichiers scannés en parallèle par un scan lancé depuis l'API (défaut : nombre de cœurs).
    #[arg(long, value_name = "N")]
    pub jobs: Option<NonZeroUsize>,
    /// Durée de conservation des passes de scan et de leurs journaux, en jours.
    #[arg(long, value_name = "JOURS", default_value_t = 30)]
    pub scan_log_days: u32,
}

/// État partagé par les requêtes.
pub struct App {
    pub db: Arc<Db>,
    pub images: Images,
    pub scanner: Scanner,
    pub metadata: Arc<Metadata>,
    /// Décoder et encoder coûte du CPU et de la mémoire : nombre de rendus simultanés borné.
    renders: Semaphore,
}

impl App {
    pub fn new(db: Arc<Db>, images: Images) -> Self {
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        Self::build(db, images, cores, 30, web_sources())
    }

    /// `jobs` fichiers scannés en parallèle, passes gardées `log_days` jours, `sources` web
    /// (de faux serveurs dans les tests).
    pub fn build(
        db: Arc<Db>,
        images: Images,
        jobs: usize,
        log_days: u32,
        sources: Vec<Arc<dyn Source>>,
    ) -> Self {
        // shortcut: 4 rendus au plus (machine de 4 Gio) ; à rendre réglable si un serveur plus gros le demande.
        let cores = std::thread::available_parallelism().map_or(1, |n| n.get());
        let metadata = Arc::new(Metadata::new(db.clone(), sources));
        Self {
            db,
            images,
            scanner: Scanner::new(jobs, log_days).with_metadata(metadata.clone()),
            metadata,
            renders: Semaphore::new(cores.min(4)),
        }
    }
}

/// Ouvre la base, écoute, et sert jusqu'à `shutdown` (Ctrl-C dans le binaire).
pub async fn run(
    args: ServeArgs,
    quiet: bool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let db = crate::open_db(args.db).await?;
    let cache = match args.cache_dir {
        Some(dir) => dir,
        None => crate::cache_dir()?,
    };
    let images = Images::open(
        cache.clone(),
        args.cache_max_mib.saturating_mul(1024 * 1024),
    )
    .with_context(|| format!("cache d'images {}", cache.display()))?;
    let listener = TcpListener::bind(args.listen)
        .await
        .with_context(|| format!("écoute sur {}", args.listen))?;
    if !quiet {
        eprintln!("Shiori écoute sur http://{}", listener.local_addr()?);
    }
    // Une passe restée `running` vient d'un arrêt brutal : elle ne reprendra pas.
    db.interrupt_running_runs().await?;
    db.purge_runs(args.scan_log_days).await?;
    let jobs = args
        .jobs
        .or_else(|| std::thread::available_parallelism().ok())
        .map_or(1, NonZeroUsize::get);
    let app = App::build(
        Arc::new(db),
        images,
        jobs,
        args.scan_log_days,
        web_sources(),
    );
    serve(listener, Arc::new(app), shutdown).await
}

pub async fn serve(
    listener: TcpListener,
    app: Arc<App>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let local_only = listener.local_addr()?.ip().is_loopback();
    axum::serve(listener, router(app.clone(), local_only))
        .with_graceful_shutdown(shutdown)
        .await?;
    // Le scan en cours est annulé et sa passe close `cancelled`, jamais laissée `running`.
    app.scanner.shutdown().await;
    // shortcut: une identification en cours finit sa requête (jusqu'à 10 s d'attente du débit + 15 s de délai réseau, fils bloquants non interruptibles) avant que le processus s'arrête ; ajouter un drapeau d'arrêt lu avant chaque appel si cette attente gêne.
    Ok(())
}

/// `local_only` : n'accepte que les requêtes adressées à `localhost` (voir [`local_host_only`]).
pub fn router(app: Arc<App>, local_only: bool) -> Router {
    let router = Router::new()
        .route("/api/health", get(health))
        .route("/api/settings", get(settings).put(save_settings))
        .route("/api/libraries", get(libraries).post(create_library))
        .route("/api/libraries/{id}", delete(delete_library))
        .route("/api/libraries/{id}/series", get(library_series))
        .route("/api/series", get(all_series))
        .route("/api/series/{id}", get(series).patch(patch_series))
        .route("/api/continue", get(continue_reading))
        .route("/api/series/{id}/settings", patch(series_settings))
        .route("/api/series/{id}/metadata/search", get(search_metadata))
        .route(
            "/api/series/{id}/metadata/links/{source}",
            put(link_source).delete(unlink_source),
        )
        .route("/api/series/{id}/metadata/refresh", post(refresh_metadata))
        .route("/api/series/{id}/metadata/cover", post(use_cover))
        .route("/api/metadata/thumbnail/{source}/{id}", get(thumbnail))
        .route("/api/covers/{id}", get(cover))
        .route("/api/admin/metadata", get(metadata_status))
        .route("/api/admin/metadata/identify", post(identify))
        .route("/api/volumes/{id}", get(volume).patch(patch_volume))
        .route(
            "/api/volumes/{id}/progress",
            get(progress).put(save_progress).delete(clear_progress),
        )
        .route("/api/volumes/{id}/pages", get(pages))
        .route("/api/pages/{id}/image", get(page_image))
        .route("/api/admin/scans", get(scans).post(start_scan))
        .route("/api/admin/scans/current", get(current_scan))
        .route("/api/admin/scans/current/cancel", post(cancel_scan))
        .route("/api/admin/scans/{id}", get(scan))
        .route("/api/admin/scans/{id}/logs", get(scan_logs))
        .fallback(crate::web::fallback)
        .method_not_allowed_fallback(async || {
            ApiError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "méthode non autorisée sur cette route",
            )
        })
        // Front (JS, CSS) et JSON compressés (brotli ou gzip, selon le navigateur) : environ trois
        // fois moins à transférer. Les images, déjà compressées, sont exclues par défaut.
        .layer(CompressionLayer::new())
        .layer(TraceLayer::new_for_http())
        .with_state(app);
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
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
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
        // Une erreur n'est jamais mise en cache : la même URL peut redevenir lisible après un rescan.
        let no_store = [(header::CACHE_CONTROL, "no-store")];
        (self.status, no_store, body).into_response()
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

async fn settings(State(app): State<Arc<App>>) -> ApiResult<shiori_db::Settings> {
    Ok(Json(app.db.settings().await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewSettings {
    /// Absent : inchangée ; `null` : revenir à la langue du navigateur.
    #[serde(default, deserialize_with = "present")]
    language: Option<Option<Language>>,
    /// Absent : inchangé ; jamais `null` (ce réglage a toujours une valeur).
    #[serde(default, deserialize_with = "present")]
    page_turn: Option<Option<PageTurn>>,
    /// Absent : inchangé ; jamais `null`.
    #[serde(default, deserialize_with = "present")]
    metadata_online: Option<Option<bool>>,
    /// Absent : inchangé ; `null` : ordre d'après la langue ; sinon sources sans doublon.
    #[serde(default, deserialize_with = "present")]
    metadata_sources: Option<Option<Vec<SourceId>>>,
}

async fn save_settings(
    State(app): State<Arc<App>>,
    body: Result<Json<NewSettings>, JsonRejection>,
) -> ApiResult<shiori_db::Settings> {
    let Json(new) = body.map_err(invalid_body)?;
    let bad = |msg: &str| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", msg);
    if new.page_turn == Some(None) {
        return Err(bad("page_turn ne peut pas être null"));
    }
    if new.metadata_online == Some(None) {
        return Err(bad("metadata_online ne peut pas être null"));
    }
    if let Some(Some(sources)) = &new.metadata_sources
        && (sources.is_empty() || (1..sources.len()).any(|i| sources[..i].contains(&sources[i])))
    {
        return Err(bad(
            "metadata_sources : au moins une source, chacune une fois",
        ));
    }
    // Un corps vide est une erreur, jamais une remise à zéro silencieuse.
    if new.language.is_none()
        && new.page_turn.is_none()
        && new.metadata_online.is_none()
        && new.metadata_sources.is_none()
    {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            "aucun réglage à enregistrer",
        ));
    }
    if let Some(language) = new.language {
        app.db.set_language(language).await?;
    }
    if let Some(Some(turn)) = new.page_turn {
        app.db.set_page_turn(turn).await?;
    }
    if let Some(Some(online)) = new.metadata_online {
        app.db.set_metadata_online(online).await?;
    }
    if let Some(sources) = new.metadata_sources {
        app.db.set_metadata_sources(sources).await?;
    }
    Ok(Json(app.db.settings().await?))
}

async fn libraries(State(app): State<Arc<App>>) -> ApiResult<Vec<shiori_db::LibrarySummary>> {
    Ok(Json(app.db.library_summaries().await?))
}

#[derive(Deserialize)]
struct NewLibrary {
    name: String,
    root: PathBuf,
    /// Donne le sens de lecture par défaut des séries (Q12) ; `manga` si absent.
    #[serde(default)]
    kind: LibraryKind,
}

#[derive(Serialize)]
struct CreatedLibrary {
    id: i64,
    name: String,
    root: String,
    kind: LibraryKind,
}

async fn create_library(
    State(app): State<Arc<App>>,
    body: Result<Json<NewLibrary>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedLibrary>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let library = app
        .db
        .create_library(&new.name, &new.root, new.kind)
        .await?;
    Ok((
        StatusCode::CREATED,
        Json(CreatedLibrary {
            id: library.id,
            name: library.name,
            root: library.root,
            kind: library.kind,
        }),
    ))
}

async fn delete_library(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.delete_library(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("bibliothèque")),
    }
}

async fn library_series(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    app.db
        .series_of(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("bibliothèque"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SeriesQuery {
    library_id: Option<i64>,
}

/// Toutes les séries, ou celles d'une bibliothèque (`?library_id=`).
async fn all_series(
    State(app): State<Arc<App>>,
    query: Result<Query<SeriesQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    let Query(q) = query.map_err(invalid_query)?;
    match q.library_id {
        None => Ok(Json(app.db.all_series().await?)),
        Some(id) => app
            .db
            .series_of(id)
            .await?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("bibliothèque")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ContinueQuery {
    limit: Option<u32>,
}

/// « Continuer la lecture » : 20 entrées par défaut, 100 au plus.
async fn continue_reading(
    State(app): State<Arc<App>>,
    query: Result<Query<ContinueQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::ContinueItem>> {
    let Query(q) = query.map_err(invalid_query)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100) as usize;
    Ok(Json(app.db.continue_reading(limit).await?))
}

fn invalid_body(e: JsonRejection) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", e.body_text())
}

impl From<EditError> for ApiError {
    fn from(e: EditError) -> Self {
        match e {
            EditError::Invalid(msg) => ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", msg),
            EditError::Db(e) => e.into(),
        }
    }
}

/// Modifie les métadonnées d'une série et renvoie son détail. Chaque champ donné est édité
/// (verrouillé, `null` compris) ; `unlock: [...]` rend des champs aux sources.
async fn patch_series(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<Edited>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    if !app.db.edit_series(id, patch).await? {
        return Err(ApiError::not_found("série"));
    }
    app.db
        .series_detail(id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn series(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    app.db
        .series_detail(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn volume(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::VolumeDetail> {
    app.db
        .volume_detail(&volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

/// Modifie les métadonnées d'un tome, comme [`patch_series`].
impl From<MetaError> for ApiError {
    fn from(e: MetaError) -> Self {
        let message = e.to_string();
        match e {
            MetaError::Disabled => {
                ApiError::new(StatusCode::CONFLICT, "metadata_disabled", message)
            }
            MetaError::UnknownSource => ApiError::not_found("source"),
            MetaError::AlreadyRunning => {
                ApiError::new(StatusCode::CONFLICT, "already_running", message)
            }
            MetaError::Fetch(f) => {
                let status = match f {
                    FetchError::NotFound => StatusCode::NOT_FOUND,
                    FetchError::RateLimited(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::BAD_GATEWAY,
                };
                ApiError::new(status, f.code(), message)
            }
            MetaError::Db(e) => e.into(),
        }
    }
}

/// Source nommée dans l'adresse (`anilist`, `mangadex`).
fn source_id(name: &str) -> Result<SourceId, ApiError> {
    serde_json::from_value(json!(name)).map_err(|_| ApiError::not_found("source"))
}

/// Détail d'une série, ou 404.
async fn series_or_404(app: &App, id: i64) -> Result<shiori_db::SeriesDetail, ApiError> {
    app.db
        .series_detail(id)
        .await?
        .ok_or_else(|| ApiError::not_found("série"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MetadataSearch {
    source: SourceId,
    q: String,
}

/// Fiches d'une source pour identifier une série à la main (fenêtre « Identifier »).
async fn search_metadata(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    query: Result<Query<MetadataSearch>, QueryRejection>,
) -> ApiResult<Vec<WebMetadata>> {
    let id = int_id(&id)?;
    let Query(q) = query.map_err(invalid_query)?;
    let text = q.q.trim().to_owned();
    if text.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "q : un titre à chercher",
        ));
    }
    series_or_404(&app, id).await?;
    Ok(Json(app.metadata.search(q.source, text).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewLink {
    external_id: String,
}

/// Lie une série à la fiche choisie et renvoie son détail.
async fn link_source(
    State(app): State<Arc<App>>,
    Path((id, source)): Path<(String, String)>,
    body: Result<Json<NewLink>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    let Json(link) = body.map_err(invalid_body)?;
    series_or_404(&app, id).await?;
    app.metadata.link(id, source, link.external_id).await?;
    Ok(Json(series_or_404(&app, id).await?))
}

/// Délie une source (ses données ne comptent plus) et renvoie le détail de la série.
async fn unlink_source(
    State(app): State<Arc<App>>,
    Path((id, source)): Path<(String, String)>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    series_or_404(&app, id).await?;
    app.db.unlink_source(id, source).await?;
    Ok(Json(series_or_404(&app, id).await?))
}

/// Relit les fiches liées d'une série et renvoie son détail.
async fn refresh_metadata(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    series_or_404(&app, id).await?;
    app.metadata.refresh(id).await?;
    Ok(Json(series_or_404(&app, id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UseCover {
    source: SourceId,
}

/// Prend la couverture de la fiche liée chez une source et renvoie le détail de la série.
async fn use_cover(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<UseCover>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(body) = body.map_err(invalid_body)?;
    series_or_404(&app, id).await?;
    app.metadata.use_cover(id, body.source).await?;
    Ok(Json(series_or_404(&app, id).await?))
}

/// Vignette d'une fiche, relayée ; gardée un jour par le navigateur.
async fn thumbnail(
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
async fn cover(
    State(app): State<Arc<App>>,
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

#[derive(Serialize)]
struct MetadataOverview {
    online: bool,
    /// Ordre appliqué des sources.
    sources: Vec<SourceId>,
    identify: IdentifyStatus,
    /// Sources à confirmer ou en échec.
    issues: Vec<shiori_db::SourceIssue>,
}

async fn metadata_status(State(app): State<Arc<App>>) -> ApiResult<MetadataOverview> {
    let settings = app.db.settings().await?;
    Ok(Json(MetadataOverview {
        online: settings.metadata_online,
        sources: settings.source_order(),
        identify: app.metadata.status(),
        issues: app.db.source_issues().await?,
    }))
}

/// Lance l'identification des séries pas encore liées (202) ; 409 si le réseau est désactivé
/// ou si une identification tourne déjà.
async fn identify(State(app): State<Arc<App>>) -> Result<StatusCode, ApiError> {
    if !app.db.settings().await?.metadata_online {
        return Err(MetaError::Disabled.into());
    }
    app.metadata.identify()?;
    Ok(StatusCode::ACCEPTED)
}

async fn patch_volume(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<Edited>, JsonRejection>,
) -> ApiResult<shiori_db::VolumeDetail> {
    let id = volume_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    if !app.db.edit_volume(&id, patch).await? {
        return Err(ApiError::not_found("tome"));
    }
    app.db
        .volume_detail(&id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

/// Progression d'un tome, ou 204 s'il n'a jamais été ouvert.
async fn progress(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let volume = volume_id(&id)?;
    if let Some(progress) = app.db.progress(&volume).await? {
        return Ok(Json(progress).into_response());
    }
    match app.db.volume_exists(&volume).await? {
        true => Ok(StatusCode::NO_CONTENT.into_response()),
        false => Err(ApiError::not_found("tome")),
    }
}

#[derive(Deserialize)]
struct NewProgress {
    page_id: String,
}

async fn save_progress(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<NewProgress>, JsonRejection>,
) -> ApiResult<shiori_db::Progress> {
    let volume = volume_id(&id)?;
    let Json(new) = body.map_err(invalid_body)?;
    let page: PageId = new
        .page_id
        .parse()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", format!("{e}")))?;
    match app.db.save_progress(&volume, &page).await? {
        SaveProgress::Saved(progress) => Ok(Json(progress)),
        SaveProgress::UnknownVolume => Err(ApiError::not_found("tome")),
        SaveProgress::PageNotInVolume => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_not_in_volume",
            "cette page n'appartient pas à ce tome",
        )),
    }
}

/// « Marquer comme non lu ».
async fn clear_progress(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.clear_progress(&volume_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("tome")),
    }
}

/// Champ absent = inchangé, `null` = revenir au défaut de la bibliothèque.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SettingsPatch {
    #[serde(default, deserialize_with = "present")]
    reading_mode: Option<Option<ReadingMode>>,
    #[serde(default, deserialize_with = "present")]
    page_layout: Option<Option<PageLayout>>,
    #[serde(default, deserialize_with = "present")]
    page_turn: Option<Option<PageTurn>>,
}

/// Distingue un champ présent à `null` (`Some(None)`) d'un champ absent (`None`, par défaut).
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

async fn series_settings(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<SettingsPatch>, JsonRejection>,
) -> ApiResult<shiori_db::ReadingSettings> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    let change = SettingsChange {
        reading_mode: patch.reading_mode,
        page_layout: patch.page_layout,
        page_turn: patch.page_turn,
    };
    app.db
        .change_series_settings(id, change)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn pages(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::PageInfo>> {
    app.db
        .pages_of(&volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

/// Une page ne change jamais sous un même ID (principe 2) : cache permanent.
pub(crate) const IMMUTABLE: &str = "public, max-age=31536000, immutable";

#[derive(Deserialize)]
struct ImageQuery {
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
async fn page_image(
    State(app): State<Arc<App>>,
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
        app.db.page_source(&id).await?.ok_or_else(|| {
            ApiError::new(StatusCode::NOT_FOUND, "page_not_found", "page inconnue")
        })?;
    if page.status == PageStatus::Broken {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_broken",
            "page illisible dans l'archive (voir les problèmes du tome)",
        ));
    }
    // Pas de décodeur AVIF pur Rust (Q2), et on n'agrandit jamais : l'originale est servie.
    let resize = wanted.filter(|&(w, _)| {
        page.media_type.as_deref() != Some("image/avif") && page.width.is_none_or(|pw| w < pw)
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
    let original = original.await?;
    let _permit = app.renders.acquire().await.expect("sémaphore jamais fermé");
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

impl From<StartError> for ApiError {
    fn from(e: StartError) -> Self {
        let (status, code) = match &e {
            StartError::AlreadyRunning => (StatusCode::CONFLICT, "scan_already_running"),
            StartError::UnknownLibrary => (StatusCode::NOT_FOUND, "not_found"),
            StartError::OutsideLibrary(_) => (StatusCode::BAD_REQUEST, "path_outside_library"),
            StartError::NotFound(_) => (StatusCode::NOT_FOUND, "path_not_found"),
            StartError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        if status.is_server_error() {
            tracing::error!("lancement d'un scan : {e}");
            return Self::new(status, code, "erreur interne");
        }
        Self::new(status, code, e.to_string())
    }
}

fn invalid_query(e: QueryRejection) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_query", e.body_text())
}

/// Taille de page des listes : 50 par défaut, 500 au plus.
fn limit(requested: Option<u32>) -> u32 {
    requested.unwrap_or(50).clamp(1, 500)
}

#[derive(Deserialize)]
struct NewScan {
    library_id: i64,
    /// Dossier ou fichier sous la racine ; absent : toute la bibliothèque.
    path: Option<PathBuf>,
    #[serde(default)]
    deep: bool,
    #[serde(default)]
    rehash: bool,
}

async fn start_scan(
    State(app): State<Arc<App>>,
    body: Result<Json<NewScan>, JsonRejection>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let request = ScanRequest {
        library_id: new.library_id,
        path: new.path,
        deep: new.deep,
        rehash: new.rehash,
    };
    let run = app.scanner.start(app.db.clone(), request).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "run": run }))))
}

/// Progression de la passe en cours, ou 204 si rien ne tourne.
async fn current_scan(State(app): State<Arc<App>>) -> Response {
    match app.scanner.progress().await {
        Some(progress) => Json(progress).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

async fn cancel_scan(
    State(app): State<Arc<App>>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    match app.scanner.cancel().await {
        true => Ok((StatusCode::ACCEPTED, Json(json!({ "cancelling": true })))),
        false => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no_scan_running",
            "aucun scan en cours",
        )),
    }
}

#[derive(Deserialize)]
struct ScansQuery {
    status: Option<RunStatus>,
    library_id: Option<i64>,
    before: Option<i64>,
    limit: Option<u32>,
}

async fn scans(
    State(app): State<Arc<App>>,
    query: Result<Query<ScansQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::RunSummary>> {
    let Query(q) = query.map_err(invalid_query)?;
    let filter = RunFilter {
        status: q.status,
        library_id: q.library_id,
        before: q.before,
        limit: limit(q.limit),
    };
    Ok(Json(app.db.runs(&filter).await?))
}

async fn scan(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::RunSummary> {
    app.db
        .run(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("passe de scan"))
}

#[derive(Deserialize)]
struct LogsQuery {
    /// Gravité minimale : `info` (tout), `warning`, `error`.
    level: Option<Severity>,
    after: Option<i64>,
    limit: Option<u32>,
}

async fn scan_logs(
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    query: Result<Query<LogsQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::LogLine>> {
    let id = int_id(&id)?;
    let Query(q) = query.map_err(invalid_query)?;
    if app.db.run(id).await?.is_none() {
        return Err(ApiError::not_found("passe de scan"));
    }
    let min = q.level.unwrap_or(Severity::Info);
    Ok(Json(
        app.db.run_logs(id, min, q.after, limit(q.limit)).await?,
    ))
}
