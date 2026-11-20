//! `shiori serve` : API HTTP JSON sous `/api`.
//!
//! Erreurs toujours au format `{ "code", "message" }`, codes stables pour l'UI. Pas
//! d'authentification en phase 2 : écoute sur 127.0.0.1 par défaut (Q10).

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{ConnectInfo, Path, Query, Request, State};
use axum::http::{Method, StatusCode, header};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post, put};
use axum::{Json, Router};
use clap::Args;
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use hyper_util::service::TowerToHyperService;
use serde::{Deserialize, Serialize};
use serde_json::json;
use shiori_core::{PageId, Severity, VolumeId};
use shiori_db::{
    Db, EditError, Edited, Language, LibraryError, LibraryKind, PageLayout, PageTurn, ReadingMode,
    RunFilter, RunStatus, SaveProgress, SettingsChange, Theme, Viewer,
};
use shiori_meta::{FetchError, Source, SourceId, WebMetadata};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tower::ServiceExt;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::backup::Backups;
use crate::cache::Images;
use crate::job::{ScanRequest, Scanner, StartError, Trigger};
use crate::meta::{IdentifyStatus, MetaError, Metadata, web_sources};

mod accounts;
mod auth;
mod backups;
mod images;
mod tokens;
mod totp;

use auth::{Admin, Me};
pub(crate) use images::{IMMUTABLE, etag_matches};
use images::{cover, page_image, thumbnail};

/// Délai pour recevoir les en-têtes d'une requête.
const HEADER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

#[derive(Args)]
pub struct ServeArgs {
    /// Adresse d'écoute ; 0.0.0.0 ouvre l'interface à tout le réseau (connexion exigée).
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
    /// Nom d'hôte accepté en plus de localhost, tel qu'écrit dans la barre d'adresse
    /// (`shiori.lan`, `192.168.1.10`), répétable ; en écoute réseau, en donner un active la
    /// parade au DNS rebinding (image Docker : `localhost`).
    #[arg(
        long = "allowed-host",
        value_name = "NOM",
        env = "SHIORI_ALLOWED_HOSTS",
        value_delimiter = ','
    )]
    pub allowed_hosts: Vec<String>,
    /// Adresse d'un reverse proxy dont l'en-tête `X-Forwarded-For` est cru, répétable ; sans
    /// elle, toute adresse locale ou privée l'est (un client du réseau local peut alors fausser
    /// le compte des essais de connexion par adresse).
    #[arg(
        long = "trusted-proxy",
        value_name = "IP",
        env = "SHIORI_TRUSTED_PROXIES",
        value_delimiter = ','
    )]
    pub trusted_proxies: Vec<std::net::IpAddr>,
    /// Dossier des sauvegardes de la base (défaut : `backups` à côté d'elle) ; un outil de
    /// sauvegarde (Backrest, restic) n'a qu'à copier ce dossier.
    #[arg(long, value_name = "DOSSIER", env = "SHIORI_BACKUP_DIR")]
    pub backup_dir: Option<PathBuf>,
    /// Une sauvegarde toutes les N heures, 0 pour aucune.
    #[arg(
        long,
        value_name = "HEURES",
        default_value_t = 24,
        env = "SHIORI_BACKUP_EVERY_HOURS"
    )]
    pub backup_every_hours: u64,
    /// Sauvegardes périodiques gardées (les plus récentes) ; celles d'avant une mise à jour ou
    /// une restauration ne sont jamais effacées.
    #[arg(
        long,
        value_name = "N",
        default_value_t = 7,
        env = "SHIORI_BACKUP_KEEP"
    )]
    pub backup_keep: usize,
    /// Ouvre l'interface dans le navigateur une fois prêt (double-clic sur le binaire). Si le
    /// port est déjà pris, par un Shiori déjà lancé, ouvre l'interface et s'arrête.
    #[arg(long)]
    pub open: bool,
}

/// État partagé par les requêtes.
pub struct App {
    pub db: Arc<Db>,
    pub images: Images,
    pub scanner: Scanner,
    pub metadata: Arc<Metadata>,
    /// Défaut : `backups` à côté de la base, sans sauvegarde périodique.
    pub backups: Backups,
    /// Décoder et encoder coûte du CPU et de la mémoire : nombre de rendus simultanés borné.
    renders: Semaphore,
    auth: auth::Guard,
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
        let backups = Backups {
            dir: crate::backup_dir(db.pool().connect_options().get_filename(), None),
            every: None,
            keep: 7,
        };
        Self {
            backups,
            db,
            images,
            scanner: Scanner::new(jobs, log_days).with_metadata(metadata.clone()),
            metadata,
            renders: Semaphore::new(cores.min(4)),
            auth: auth::Guard::new(),
        }
    }

    /// Reverse proxies dont l'en-tête `X-Forwarded-For` est cru (`--trusted-proxy`) ; sans eux,
    /// toute adresse locale ou privée.
    pub fn trust_proxies(&mut self, proxies: Vec<std::net::IpAddr>) {
        self.auth.trusted_proxies = proxies.into_iter().map(|ip| ip.to_canonical()).collect();
    }

    /// Jeton exigé pour créer le premier compte, tiré au hasard à chaque démarrage.
    pub fn setup_token(&self) -> &str {
        self.auth.setup_token()
    }
}

/// Ouvre la base, écoute, et sert jusqu'à `shutdown` (Ctrl-C dans le binaire).
pub async fn run(
    args: ServeArgs,
    quiet: bool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    // Port d'abord : un second lancement (`--open`) ne doit pas migrer la base d'un Shiori
    // déjà lancé, peut-être d'une autre version.
    let listener = match TcpListener::bind(args.listen).await {
        Ok(listener) => listener,
        // shortcut: le port pris est supposé l'être par un Shiori déjà lancé (second double-clic) ; interroger /api/health si un autre programme le prend un jour.
        Err(e) if args.open && e.kind() == std::io::ErrorKind::AddrInUse => {
            eprintln!("Shiori tourne déjà : ouverture de l'interface.");
            open_browser(args.listen.port());
            return Ok(());
        }
        Err(e) => return Err(e).with_context(|| format!("écoute sur {}", args.listen)),
    };
    let (db, backup_dir) = crate::open_db(args.db, args.backup_dir).await?;
    let cache = match args.cache_dir {
        Some(dir) => dir,
        None => crate::cache_dir()?,
    };
    let images = Images::open(
        cache.clone(),
        args.cache_max_mib.saturating_mul(1024 * 1024),
    )
    .with_context(|| format!("cache d'images {}", cache.display()))?;
    if !quiet {
        eprintln!("Shiori écoute sur http://{}", listener.local_addr()?);
        eprintln!(
            "Base : {}",
            absolute(db.pool().connect_options().get_filename())
        );
        eprintln!("Cache : {}", absolute(&cache));
        eprintln!("Sauvegardes : {}", absolute(&backup_dir));
    }
    // Une passe restée `running` vient d'un arrêt brutal : elle ne reprendra pas.
    db.interrupt_running_runs().await?;
    db.purge_runs(args.scan_log_days).await?;
    db.purge_covers().await?;
    let jobs = args
        .jobs
        .or_else(|| std::thread::available_parallelism().ok())
        .map_or(1, NonZeroUsize::get);
    let mut app = App::build(
        Arc::new(db),
        images,
        jobs,
        args.scan_log_days,
        web_sources(),
    );
    app.backups = Backups {
        dir: backup_dir,
        every: (args.backup_every_hours > 0)
            .then(|| std::time::Duration::from_secs(args.backup_every_hours * 3600)),
        keep: args.backup_keep,
    };
    app.trust_proxies(args.trusted_proxies);
    app.backups.clone().spawn(app.db.clone());
    // Toujours affiché, même avec `--quiet` : sans lui, personne ne peut créer le premier compte.
    if app.db.users().await?.is_empty() {
        eprintln!(
            "Aucun compte : ouvrez l'interface et saisissez le jeton d'installation {}",
            app.setup_token()
        );
    }
    if args.open {
        open_browser(listener.local_addr()?.port());
    }
    let app = Arc::new(app);
    crate::watch::spawn(app.clone(), crate::watch::Timing::default());
    serve(listener, app, args.allowed_hosts, shutdown).await
}

/// `localhost` : seul nom toujours accepté par la parade au DNS rebinding, quelle que soit
/// l'adresse d'écoute.
fn open_browser(port: u16) {
    let url = format!("http://localhost:{port}");
    if let Err(e) = webbrowser::open(&url) {
        eprintln!("navigateur introuvable ({e}) : ouvrez {url}");
    }
}

/// Chemin complet à montrer (une base donnée par un simple nom de fichier est relative).
fn absolute(path: &std::path::Path) -> String {
    std::path::absolute(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .display()
        .to_string()
}

pub async fn serve(
    listener: TcpListener,
    app: Arc<App>,
    allowed_hosts: Vec<String>,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    // `SHIORI_ALLOWED_HOSTS=` vide ou `a, b` : ni nom vide (il laisserait passer une requête
    // sans `Host`), ni espace.
    let allowed_hosts: Vec<String> = allowed_hosts
        .iter()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .collect();
    // Écoute locale : toujours protégée ; écoute réseau : seulement si des noms sont donnés.
    let local = listener.local_addr()?.ip().is_loopback();
    let hosts = (local || !allowed_hosts.is_empty()).then_some(allowed_hosts);
    let router = router(app.clone(), hosts);
    // Boucle d'hyper plutôt qu'`axum::serve`, qui ne pose aucun délai : un client qui envoie ses
    // en-têtes octet par octet garderait sa connexion ouverte indéfiniment.
    let graceful = GracefulShutdown::new();
    let mut shutdown = std::pin::pin!(shutdown);
    loop {
        let (stream, peer) = tokio::select! {
            accepted = listener.accept() => match accepted {
                Ok(accepted) => accepted,
                // Trop de fichiers ouverts, connexion avortée : on continue d'écouter.
                Err(e) => {
                    tracing::warn!("connexion refusée : {e}");
                    continue;
                }
            },
            () = &mut shutdown => break,
        };
        let service = TowerToHyperService::new(router.clone().map_request(
            move |mut request: axum::http::Request<Incoming>| {
                request.extensions_mut().insert(ConnectInfo(peer));
                request
            },
        ));
        let connection = http1::Builder::new()
            .timer(TokioTimer::new())
            .header_read_timeout(HEADER_TIMEOUT)
            .serve_connection(TokioIo::new(stream), service);
        let connection = graceful.watch(connection);
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                tracing::debug!("connexion de {peer} : {e}");
            }
        });
    }
    graceful.shutdown().await;
    // Le scan en cours est annulé et sa passe close `cancelled`, jamais laissée `running`.
    app.scanner.shutdown().await;
    // shortcut: une identification en cours finit sa requête (jusqu'à 10 s d'attente du débit + 15 s de délai réseau, fils bloquants non interruptibles) avant que le processus s'arrête ; ajouter un drapeau d'arrêt lu avant chaque appel si cette attente gêne.
    Ok(())
}

/// `hosts` : n'accepte que les requêtes adressées à `localhost` ou à l'un de ces noms (voir
/// [`allowed_host_only`]) ; `None` : aucun contrôle.
pub fn router(app: Arc<App>, hosts: Option<Vec<String>>) -> Router {
    let router = Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/state", get(auth::state))
        .route("/api/auth/setup", post(auth::setup))
        .route("/api/auth/login", post(auth::login))
        .route("/api/auth/totp", post(auth::second_factor))
        .route("/api/auth/logout", post(auth::logout))
        .route("/api/me", get(accounts::me).put(accounts::save_me))
        .route("/api/me/password", post(auth::change_password))
        .route("/api/me/sessions", get(accounts::devices))
        .route("/api/me/totp", post(totp::start).delete(totp::disable))
        .route("/api/me/totp/confirm", post(totp::confirm))
        .route("/api/me/tokens", get(tokens::list).post(tokens::create))
        .route("/api/me/tokens/{id}", delete(tokens::revoke))
        .route("/api/me/sessions/{id}", delete(accounts::revoke_device))
        .route(
            "/api/admin/users",
            get(accounts::users).post(accounts::create_user),
        )
        .route(
            "/api/admin/users/{id}",
            patch(accounts::update_user).delete(accounts::delete_user),
        )
        .route(
            "/api/admin/users/{id}/password",
            post(accounts::reset_password),
        )
        .route(
            "/api/admin/users/{id}/sessions",
            delete(accounts::revoke_sessions),
        )
        .route("/api/admin/users/{id}/totp", delete(totp::reset))
        .route("/api/settings", get(settings).put(save_settings))
        .route("/api/libraries", get(libraries).post(create_library))
        .route(
            "/api/libraries/{id}",
            patch(patch_library).delete(delete_library),
        )
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
        .route("/api/admin/storage", get(storage))
        .route(
            "/api/admin/backups",
            get(backups::list).post(backups::create),
        )
        .route(
            "/api/admin/backups/upload",
            // Une base avec ses couvertures dépasse vite la limite par défaut de 2 Mio.
            post(backups::upload).layer(DefaultBodyLimit::disable()),
        )
        .route(
            "/api/admin/backups/{name}",
            get(backups::download).delete(backups::remove),
        )
        .route("/api/admin/backups/{name}/restore", post(backups::restore))
        .route("/api/admin/metadata/identify", post(identify))
        .route("/api/volumes/{id}", get(volume).patch(patch_volume))
        .route(
            "/api/volumes/{id}/progress",
            get(progress).put(save_progress).delete(clear_progress),
        )
        .route("/api/volumes/{id}/pages", get(pages))
        .route("/api/pages/{id}/image", get(page_image))
        .route("/api/admin/health", get(health_summary))
        .route("/api/admin/health/files", get(files_to_repair))
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
        .layer(middleware::from_fn_with_state(
            app.clone(),
            auth::require_session,
        ))
        // Front (JS, CSS) et JSON compressés (brotli ou gzip, selon le navigateur) : environ trois
        // fois moins à transférer. Les images, déjà compressées, sont exclues par défaut.
        .layer(CompressionLayer::new())
        .layer(middleware::from_fn(same_site_writes_only))
        .layer(TraceLayer::new_for_http())
        .with_state(app);
    match hosts {
        Some(hosts) => {
            let hosts: Arc<[String]> = hosts.iter().map(|h| h.to_ascii_lowercase()).collect();
            router.layer(middleware::from_fn_with_state(hosts, allowed_host_only))
        }
        None => router,
    }
}

/// Parade au CSRF : un formulaire d'un autre site peut envoyer un POST sans corps JSON (annuler
/// le scan, identifier), que CORS ne bloque pas. Le navigateur dit d'où vient la requête.
async fn same_site_writes_only(request: Request, next: Next) -> Response {
    let read = matches!(*request.method(), Method::GET | Method::HEAD);
    let cross = request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v == "cross-site");
    match read || !cross {
        true => next.run(request).await,
        false => ApiError::new(
            StatusCode::FORBIDDEN,
            "cross_site",
            "requête d'un autre site refusée",
        )
        .into_response(),
    }
}

/// Parade au DNS rebinding : sans elle, une page web malveillante peut faire pointer son
/// propre domaine vers 127.0.0.1 et piloter l'API depuis le navigateur. Le `Host` doit donc
/// être local ou l'un des noms autorisés (`--allowed-host`). En écoute réseau sans nom
/// autorisé, aucune protection tant qu'il n'y a pas d'authentification.
async fn allowed_host_only(
    State(hosts): State<Arc<[String]>>,
    request: Request,
    next: Next,
) -> Response {
    let host = request
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let name = host
        .rsplit_once(':')
        .filter(|(_, port)| !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()))
        .map_or(host, |(name, _)| name)
        .to_ascii_lowercase();
    match matches!(name.as_str(), "localhost" | "127.0.0.1" | "[::1]") || hosts.contains(&name) {
        true => next.run(request).await,
        false => ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_host",
            "nom d'hôte non autorisé (localhost, ou un nom donné à --allowed-host ou SHIORI_ALLOWED_HOSTS)",
        )
        .into_response(),
    }
}

#[derive(Debug)]
pub struct ApiError {
    status: StatusCode,
    code: &'static str,
    message: String,
    /// `Retry-After`, en secondes (429).
    retry_after: Option<u64>,
}

impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    fn retry_after(self, seconds: u64) -> Self {
        Self {
            retry_after: Some(seconds),
            ..self
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
        let mut response = (self.status, no_store, body).into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(header::RETRY_AFTER, seconds.into());
        }
        response
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

/// Où sont la base et le cache (administration), pour les sauvegarder ou les retrouver.
async fn storage(_: Admin, State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    Json(json!({
        "database": absolute(app.db.pool().connect_options().get_filename()),
        "cache": absolute(app.images.dir()),
    }))
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
    /// Absent : inchangé ; jamais `null`.
    #[serde(default, deserialize_with = "present")]
    metadata_adult: Option<Option<bool>>,
    /// Absent : inchangé ; `null` : ordre d'après la langue ; sinon sources sans doublon.
    #[serde(default, deserialize_with = "present")]
    metadata_sources: Option<Option<Vec<SourceId>>>,
    /// Absent : inchangé ; jamais `null` (`system` pour suivre l'appareil).
    #[serde(default, deserialize_with = "present")]
    theme: Option<Option<Theme>>,
    /// Absent : inchangé ; 0 (jamais), 1, 6 ou 24 heures.
    full_scan_hours: Option<u32>,
}

async fn save_settings(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewSettings>, JsonRejection>,
) -> ApiResult<shiori_db::Settings> {
    let Json(new) = body.map_err(invalid_body)?;
    let bad = |msg: &str| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", msg);
    if new.page_turn == Some(None) {
        return Err(bad("page_turn ne peut pas être null"));
    }
    if new.theme == Some(None) {
        return Err(bad("theme ne peut pas être null"));
    }
    if new
        .full_scan_hours
        .is_some_and(|h| ![0, 1, 6, 24].contains(&h))
    {
        return Err(bad("full_scan_hours : 0, 1, 6 ou 24"));
    }
    if new.metadata_online == Some(None) || new.metadata_adult == Some(None) {
        return Err(bad(
            "metadata_online et metadata_adult ne peuvent pas être null",
        ));
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
        && new.metadata_adult.is_none()
        && new.metadata_sources.is_none()
        && new.theme.is_none()
        && new.full_scan_hours.is_none()
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
    if let Some(hours) = new.full_scan_hours {
        app.db.set_full_scan_hours(hours).await?;
    }
    if let Some(Some(theme)) = new.theme {
        app.db.set_theme(theme).await?;
    }
    if let Some(Some(online)) = new.metadata_online {
        app.db.set_metadata_online(online).await?;
    }
    if let Some(Some(adult)) = new.metadata_adult {
        app.db.set_metadata_adult(adult).await?;
    }
    if let Some(sources) = new.metadata_sources {
        app.db.set_metadata_sources(sources).await?;
    }
    Ok(Json(app.db.settings().await?))
}

async fn libraries(
    State(app): State<Arc<App>>,
    Me(me): Me,
) -> ApiResult<Vec<shiori_db::LibrarySummary>> {
    Ok(Json(app.db.library_summaries(&me).await?))
}

#[derive(Deserialize)]
struct NewLibrary {
    name: String,
    root: PathBuf,
    /// Donne le sens de lecture par défaut des séries (Q12) ; `manga` si absent.
    #[serde(default)]
    kind: LibraryKind,
    /// Ouverte aux lecteurs existants ; sinon visible de l'administrateur seul (Q49).
    #[serde(default)]
    share: bool,
}

#[derive(Serialize)]
struct CreatedLibrary {
    id: i64,
    name: String,
    root: String,
    kind: LibraryKind,
}

async fn create_library(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewLibrary>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedLibrary>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let library = app
        .db
        .create_library(&new.name, &new.root, new.kind)
        .await?;
    if new.share {
        app.db.share_library(library.id).await?;
    }
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryChange {
    watch: bool,
}

async fn patch_library(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<LibraryChange>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(change) = body.map_err(invalid_body)?;
    match app.db.set_library_watch(int_id(&id)?, change.watch).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("bibliothèque")),
    }
}

async fn delete_library(
    _: Admin,
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
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    app.db
        .series_of(&me, int_id(&id)?)
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
    Me(me): Me,
    query: Result<Query<SeriesQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    let Query(q) = query.map_err(invalid_query)?;
    match q.library_id {
        None => Ok(Json(app.db.all_series(&me).await?)),
        Some(id) => app
            .db
            .series_of(&me, id)
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
    Me(me): Me,
    query: Result<Query<ContinueQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::ContinueItem>> {
    let Query(q) = query.map_err(invalid_query)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100) as usize;
    Ok(Json(app.db.continue_reading(&me, limit).await?))
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
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<Edited>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    if !app.db.edit_series(id, patch).await? {
        return Err(ApiError::not_found("série"));
    }
    app.db
        .series_detail(&me, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn series(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    app.db
        .series_detail(&me, int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn volume(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::VolumeDetail> {
    app.db
        .volume_detail(&me, &volume_id(&id)?)
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
async fn series_or_404(
    app: &App,
    me: &Viewer,
    id: i64,
) -> Result<shiori_db::SeriesDetail, ApiError> {
    app.db
        .series_detail(me, id)
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
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
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
    series_or_404(&app, &me, id).await?;
    let lang = shiori_db::Settings::code(me.language);
    Ok(Json(app.metadata.search(q.source, text, lang).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct NewLink {
    external_id: String,
}

/// Lie une série à la fiche choisie et renvoie son détail.
async fn link_source(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, source)): Path<(String, String)>,
    body: Result<Json<NewLink>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    let Json(link) = body.map_err(invalid_body)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.link(id, source, link.external_id).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

/// Délie une source (ses données ne comptent plus) et renvoie le détail de la série.
async fn unlink_source(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, source)): Path<(String, String)>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    series_or_404(&app, &me, id).await?;
    app.db.unlink_source(id, source).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

/// Relit les fiches liées d'une série et renvoie son détail.
async fn refresh_metadata(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.refresh(id).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct UseCover {
    source: SourceId,
}

/// Prend la couverture de la fiche liée chez une source et renvoie le détail de la série.
async fn use_cover(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<UseCover>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(body) = body.map_err(invalid_body)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.use_cover(id, body.source).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
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

async fn metadata_status(_: Admin, State(app): State<Arc<App>>) -> ApiResult<MetadataOverview> {
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
async fn identify(_: Admin, State(app): State<Arc<App>>) -> Result<StatusCode, ApiError> {
    if !app.db.settings().await?.metadata_online {
        return Err(MetaError::Disabled.into());
    }
    app.metadata.identify()?;
    Ok(StatusCode::ACCEPTED)
}

async fn patch_volume(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<Edited>, JsonRejection>,
) -> ApiResult<shiori_db::VolumeDetail> {
    let id = volume_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    if !app.db.edit_volume(&id, patch).await? {
        return Err(ApiError::not_found("tome"));
    }
    app.db
        .volume_detail(&me, &id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

/// Progression d'un tome, ou 204 s'il n'a jamais été ouvert.
async fn progress(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let volume = volume_id(&id)?;
    if let Some(progress) = app.db.progress(&me, &volume).await? {
        return Ok(Json(progress).into_response());
    }
    match app.db.volume_visible(&me, &volume).await? {
        true => Ok(StatusCode::NO_CONTENT.into_response()),
        false => Err(ApiError::not_found("tome")),
    }
}

#[derive(Deserialize)]
struct NewProgress {
    page_id: String,
    /// Heure de lecture (ISO 8601), pour une page lue hors ligne et envoyée plus tard.
    read_at: Option<String>,
}

async fn save_progress(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<NewProgress>, JsonRejection>,
) -> ApiResult<shiori_db::Progress> {
    let volume = volume_id(&id)?;
    let Json(new) = body.map_err(invalid_body)?;
    let page: PageId = new
        .page_id
        .parse()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", format!("{e}")))?;
    let saved = app
        .db
        .save_progress_at(&me, &volume, &page, new.read_at.as_deref());
    match saved.await? {
        SaveProgress::Saved(progress) => Ok(Json(progress)),
        SaveProgress::InvalidTime => Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            "read_at : date ISO 8601 attendue",
        )),
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
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.clear_progress(&me, &volume_id(&id)?).await? {
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
    Me(me): Me,
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
        .change_series_settings(&me, id, change)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

async fn pages(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::PageInfo>> {
    app.db
        .pages_of(&me, &volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

impl From<StartError> for ApiError {
    fn from(e: StartError) -> Self {
        let (status, code) = match &e {
            StartError::AlreadyRunning => (StatusCode::CONFLICT, "scan_already_running"),
            StartError::ShuttingDown => (StatusCode::SERVICE_UNAVAILABLE, "shutting_down"),
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
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewScan>, JsonRejection>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let request = ScanRequest {
        library_id: new.library_id,
        path: new.path,
        deep: new.deep,
        rehash: new.rehash,
        trigger: Trigger::Manual,
    };
    let run = app.scanner.start(app.db.clone(), request).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "run": run }))))
}

/// Progression de la passe en cours, ou 204 si rien ne tourne.
async fn current_scan(_: Admin, State(app): State<Arc<App>>) -> Response {
    match app.scanner.progress().await {
        Some(progress) => Json(progress).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

async fn cancel_scan(
    _: Admin,
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

/// Santé des bibliothèques : états des tomes, fichiers illisibles, codes présents.
async fn health_summary(_: Admin, State(app): State<Arc<App>>) -> ApiResult<shiori_db::Health> {
    Ok(Json(app.db.health().await?))
}

#[derive(Deserialize)]
struct RepairQuery {
    library_id: Option<i64>,
    code: Option<shiori_core::IssueCode>,
    /// Chemins après celui-ci (pagination).
    after: Option<String>,
    limit: Option<u32>,
}

async fn files_to_repair(
    _: Admin,
    State(app): State<Arc<App>>,
    query: Result<Query<RepairQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::FileHealth>> {
    let Query(q) = query.map_err(invalid_query)?;
    let filter = shiori_db::HealthFilter {
        library_id: q.library_id,
        code: q.code,
        after: q.after,
        limit: limit(q.limit),
    };
    Ok(Json(app.db.files_to_repair(&filter).await?))
}

#[derive(Deserialize)]
struct ScansQuery {
    status: Option<RunStatus>,
    library_id: Option<i64>,
    before: Option<i64>,
    limit: Option<u32>,
}

async fn scans(
    _: Admin,
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
    _: Admin,
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
    _: Admin,
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
