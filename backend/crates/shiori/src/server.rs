//! `shiori serve` : API HTTP JSON sous `/api`.
//!
//! Erreurs toujours au format `{ "code", "message" }`, codes stables pour l'UI. Session exigée
//! sauf sur quelques routes (`auth::require_session`) ; écoute sur 127.0.0.1 par défaut (Q10).

use std::future::Future;
use std::net::SocketAddr;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use axum::extract::DefaultBodyLimit;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{ConnectInfo, Request, State};
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
use serde::Deserialize;
use serde_json::json;
use shiori_core::{PageId, VolumeId};
use shiori_db::{Db, LibraryError};
use shiori_meta::{Source, SourceId};
use tokio::net::TcpListener;
use tokio::sync::Semaphore;
use tower::ServiceExt;
use tower_http::compression::CompressionLayer;
use tower_http::trace::TraceLayer;

use crate::backup::Backups;
use crate::cache::Images;
use crate::job::Scanner;
use crate::meta::{Metadata, web_sources};

mod accounts;
mod auth;
mod backups;
mod images;
mod libraries;
mod metadata;
mod scans;
mod series;
mod settings;
mod tokens;
mod totp;
mod volumes;

use auth::Admin;
pub(crate) use images::{IMMUTABLE, etag_matches};
use images::{cover, page_image, thumbnail};
use libraries::{create_library, delete_library, libraries, library_series, patch_library};
use metadata::{
    identify, link_source, metadata_status, refresh_metadata, search_metadata, unlink_source,
    use_cover,
};
use scans::{
    cancel_scan, current_scan, files_to_repair, health_summary, scan, scan_logs, scans, start_scan,
};
use series::{
    all_series, continue_reading, mark_series_read, mark_series_unread, patch_series, series,
    series_settings,
};
use settings::{save_settings, settings};
use volumes::{
    clear_progress, pages, patch_volume, progress, reading_stats, save_progress, volume,
};

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
    /// Rendus en cours, demandés ou anticipés (page, largeur, format) : jamais deux fois à la fois.
    rendering: std::sync::Mutex<
        std::collections::HashMap<(PageId, u32, crate::render::Format), Arc<tokio::sync::Notify>>,
    >,
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
            rendering: Default::default(),
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
    db.purge_sessions().await?;
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
        .route("/api/me/stats", get(reading_stats))
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
        .route(
            "/api/series/{id}/read",
            put(mark_series_read).delete(mark_series_unread),
        )
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
/// le scan, restaurer), que CORS ne bloque pas. Le navigateur dit d'où vient la requête. Même
/// `same-site` est refusé : un autre sous-domaine ou un autre port reçoit le cookie `Lax`.
async fn same_site_writes_only(request: Request, next: Next) -> Response {
    let read = matches!(*request.method(), Method::GET | Method::HEAD);
    let cross = request
        .headers()
        .get("sec-fetch-site")
        .is_some_and(|v| v != "same-origin" && v != "none");
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
/// autorisé, pas de contrôle du `Host` : seule la session protège l'API.
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
    Json(json!({ "status": "ok", "version": crate::RELEASE }))
}

/// Où sont la base et le cache (administration), pour les sauvegarder ou les retrouver.
async fn storage(_: Admin, State(app): State<Arc<App>>) -> Json<serde_json::Value> {
    Json(json!({
        "database": absolute(app.db.pool().connect_options().get_filename()),
        "cache": absolute(app.images.dir()),
    }))
}

fn invalid_body(e: JsonRejection) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", e.body_text())
}

/// Source nommée dans l'adresse (`anilist`, `mangadex`).
fn source_id(name: &str) -> Result<SourceId, ApiError> {
    serde_json::from_value(json!(name)).map_err(|_| ApiError::not_found("source"))
}

/// Distingue un champ présent à `null` (`Some(None)`) d'un champ absent (`None`, par défaut).
fn present<'de, D, T>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(d).map(Some)
}

fn invalid_query(e: QueryRejection) -> ApiError {
    ApiError::new(StatusCode::BAD_REQUEST, "invalid_query", e.body_text())
}

/// Taille de page des listes : 50 par défaut, 500 au plus.
fn limit(requested: Option<u32>) -> u32 {
    requested.unwrap_or(50).clamp(1, 500)
}
