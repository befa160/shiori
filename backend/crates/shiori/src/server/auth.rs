//! Installation, connexion, déconnexion et garde de session (phase 7, lot 7.3).
//!
//! - Navigateur : cookie `shiori_session` `HttpOnly`, `SameSite=Lax`, `Secure` derrière un proxy
//!   HTTPS. Application native (`"client": "app"`) : le même jeton, renvoyé dans la réponse et
//!   présenté en `Authorization: Bearer`.
//! - Toute route `/api` exige une session, sauf [`PUBLIC`]. L'interface (hors `/api`) reste
//!   servie : elle affiche elle-même la page de connexion.
//! - Connexion : essais ratés comptés par nom et par adresse ; au-delà d'un seuil, pause qui
//!   double à chaque échec (429 `rate_limited` avec `Retry-After`), sans même calculer le hash.

use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, FromRequestParts, Request, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use shiori_db::{AccountError, Role, SESSION_SECONDS, Session, User, Viewer};
use tokio::sync::Semaphore;

use super::{ApiError, App, invalid_body};

/// Routes ouvertes sans session.
const PUBLIC: [&str; 5] = [
    "/api/health",
    "/api/auth/state",
    "/api/auth/setup",
    "/api/auth/login",
    "/api/auth/logout",
];
/// Routes permises avec un mot de passe provisoire : de quoi le changer.
const MUST_CHANGE_OPEN: [&str; 2] = ["/api/me", "/api/me/password"];
const COOKIE: &str = "shiori_session";
/// Essais ratés tolérés avant la première pause, pour un nom puis pour une adresse (plusieurs
/// personnes peuvent partager une adresse, derrière une box).
const FREE_PER_NAME: u32 = 5;
const FREE_PER_ADDRESS: u32 = 20;
const MAX_PAUSE: Duration = Duration::from_secs(15 * 60);

/// État de l'authentification, partagé par les requêtes.
pub(crate) struct Guard {
    /// Jeton d'installation (Q46) : affiché dans la console, exigé pour créer le premier compte.
    setup_token: String,
    failures: Mutex<HashMap<String, (u32, Instant)>>,
    /// Argon2 prend ~20 Mio et des dizaines de ms par essai : peu de calculs à la fois.
    hashing: Semaphore,
}

impl Guard {
    pub(crate) fn new() -> Self {
        Self {
            setup_token: shiori_db::temporary_password(),
            failures: Mutex::default(),
            hashing: Semaphore::new(2),
        }
    }

    pub(crate) fn setup_token(&self) -> &str {
        &self.setup_token
    }

    /// Temps restant avant le prochain essai permis pour l'une de ces clés, s'il y en a.
    fn paused(&self, keys: &[(String, u32)]) -> Option<Duration> {
        let failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        keys.iter()
            .filter_map(|(key, free)| {
                let &(count, last) = failures.get(key)?;
                let pause = count.checked_sub(*free).filter(|n| *n > 0)?;
                let pause = Duration::from_secs(1u64 << pause.min(10)).min(MAX_PAUSE);
                pause.checked_sub(last.elapsed())
            })
            .max()
    }

    fn failed(&self, keys: &[(String, u32)]) {
        let mut failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        // Des noms inventés par milliers ne doivent pas faire grossir la table sans fin.
        if failures.len() > 10_000 {
            failures.retain(|_, (_, last)| last.elapsed() < MAX_PAUSE);
        }
        for (key, _) in keys {
            let entry = failures.entry(key.clone()).or_insert((0, Instant::now()));
            *entry = (entry.0.saturating_add(1), Instant::now());
        }
    }

    fn succeeded(&self, name_key: &str) {
        let mut failures = self.failures.lock().unwrap_or_else(PoisonError::into_inner);
        failures.remove(name_key);
    }
}

/// Adresse du client : celle de la connexion, ou, si elle vient d'un proxy local (même machine
/// ou réseau privé), la dernière de `X-Forwarded-For`, ajoutée par ce proxy.
fn client_address(peer: Option<SocketAddr>, headers: &HeaderMap) -> String {
    let peer = peer.map(|p| p.ip());
    let local = |ip: IpAddr| match ip {
        IpAddr::V4(v4) => v4.is_loopback() || v4.is_private(),
        IpAddr::V6(v6) => v6.is_loopback() || v6.is_unique_local(),
    };
    let forwarded = || {
        headers
            .get("x-forwarded-for")?
            .to_str()
            .ok()?
            .rsplit(',')
            .next()?
            .trim()
            .parse::<IpAddr>()
            .ok()
    };
    match peer {
        Some(ip) if local(ip) => forwarded().unwrap_or(ip).to_string(),
        Some(ip) => ip.to_string(),
        None => "inconnue".into(),
    }
}

/// Jeton présenté : `Authorization: Bearer`, sinon le cookie. Le booléen dit s'il vient du cookie.
fn presented_token(headers: &HeaderMap) -> Option<(String, bool)> {
    let bearer = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "));
    if let Some(token) = bearer {
        return Some((token.trim().to_owned(), false));
    }
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| c.trim().strip_prefix(COOKIE)?.strip_prefix('='))
        .map(|token| (token.to_owned(), true))
}

fn cookie(headers: &HeaderMap, token: &str, max_age: u64) -> HeaderValue {
    // Derrière un reverse proxy HTTPS, le cookie ne doit jamais repartir en clair.
    let https = headers
        .get("x-forwarded-proto")
        .is_some_and(|v| v.as_bytes().eq_ignore_ascii_case(b"https"));
    let secure = if https { "; Secure" } else { "" };
    let value =
        format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}");
    HeaderValue::from_str(&value).expect("jeton hexadécimal")
}

/// Exige une session valide sur `/api` (sauf [`PUBLIC`]) et la donne aux routes ; renvoie le
/// cookie quand la session vient d'être prolongée, pour qu'il ne périme pas avant elle.
pub(crate) async fn require_session(
    State(app): State<Arc<App>>,
    mut request: Request,
    next: Next,
) -> Response {
    let path = request.uri().path().to_owned();
    if !path.starts_with("/api/") || PUBLIC.contains(&path.as_str()) {
        return next.run(request).await;
    }
    let session = match presented_token(request.headers()) {
        Some((token, from_cookie)) => match app.db.session_user(&token).await {
            Ok(Some(session)) => Some((session, token, from_cookie)),
            Ok(None) => None,
            Err(e) => return ApiError::from(e).into_response(),
        },
        None => None,
    };
    let Some((session, token, from_cookie)) = session else {
        return unauthenticated().into_response();
    };
    // Mot de passe provisoire (Q45) : rien d'autre tant qu'il n'est pas changé.
    if session.user.must_change_password && !MUST_CHANGE_OPEN.contains(&path.as_str()) {
        return ApiError::new(
            StatusCode::FORBIDDEN,
            "password_change_required",
            "mot de passe provisoire : le changer d'abord",
        )
        .into_response();
    }
    let renew = (session.renewed && from_cookie)
        .then(|| cookie(request.headers(), &token, SESSION_SECONDS));
    request.extensions_mut().insert(session);
    let mut response = next.run(request).await;
    if let Some(cookie) = renew {
        response.headers_mut().append(header::SET_COOKIE, cookie);
    }
    response
}

fn unauthenticated() -> ApiError {
    ApiError::new(
        StatusCode::UNAUTHORIZED,
        "unauthenticated",
        "connexion requise",
    )
}

/// Lecteur de la requête (compte de la session et sa langue), pour les routes dont la réponse
/// dépend du compte : progression, réglages, titres.
pub(crate) struct Me(pub(crate) Viewer);

impl FromRequestParts<Arc<App>> for Me {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, app: &Arc<App>) -> Result<Self, ApiError> {
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or_else(unauthenticated)?;
        Ok(Me(app.db.viewer(session.user.id).await?))
    }
}

/// Route réservée à l'administrateur : 403 `forbidden` pour un lecteur.
pub(crate) struct Admin;

impl FromRequestParts<Arc<App>> for Admin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _: &Arc<App>) -> Result<Self, ApiError> {
        let session = parts
            .extensions
            .get::<Session>()
            .ok_or_else(unauthenticated)?;
        match session.user.role {
            Role::Admin => Ok(Admin),
            Role::Reader => Err(ApiError::new(
                StatusCode::FORBIDDEN,
                "forbidden",
                "réservé à l'administrateur",
            )),
        }
    }
}

impl From<AccountError> for ApiError {
    fn from(e: AccountError) -> Self {
        let (status, code) = match &e {
            AccountError::InvalidName => (StatusCode::BAD_REQUEST, "invalid_username"),
            AccountError::WeakPassword => (StatusCode::BAD_REQUEST, "weak_password"),
            AccountError::NameTaken(_) => (StatusCode::CONFLICT, "name_taken"),
            AccountError::UnknownUser(_) => (StatusCode::NOT_FOUND, "not_found"),
            AccountError::LastAdmin => (StatusCode::CONFLICT, "last_admin"),
            AccountError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        if status.is_server_error() {
            tracing::error!("erreur de base : {e}");
            return Self::new(status, code, "erreur interne");
        }
        Self::new(status, code, e.to_string())
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Client {
    /// Cookie : le jeton n'est jamais lisible par le JavaScript de la page.
    #[default]
    Browser,
    /// Jeton dans la réponse, pour une application native (`Authorization: Bearer`).
    App,
}

#[derive(Deserialize)]
pub(crate) struct Credentials {
    username: String,
    password: String,
    #[serde(default)]
    client: Client,
}

#[derive(Deserialize)]
pub(crate) struct Setup {
    token: String,
    username: String,
    password: String,
    #[serde(default)]
    client: Client,
}

#[derive(Serialize)]
struct SignedIn {
    user: User,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct AuthState {
    /// Aucun compte : l'interface montre la page d'installation.
    setup_required: bool,
    user: Option<User>,
}

/// 429 `rate_limited`, avec `Retry-After` en secondes.
fn rate_limited(wait: Duration) -> ApiError {
    let seconds = wait.as_secs() + 1;
    ApiError::new(
        StatusCode::TOO_MANY_REQUESTS,
        "rate_limited",
        format!("trop d'essais : réessayer dans {seconds} s"),
    )
    .retry_after(seconds)
}

/// Ouvre une session et la remet au client : cookie, ou jeton dans la réponse.
async fn sign_in(
    app: &App,
    headers: &HeaderMap,
    user: User,
    client: Client,
) -> Result<Response, ApiError> {
    let label: String = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("appareil inconnu")
        .chars()
        .take(200)
        .collect();
    let session = app.db.create_session(user.id, &label).await?;
    Ok(match client {
        Client::App => Json(SignedIn {
            user,
            token: Some(session.token),
        })
        .into_response(),
        Client::Browser => {
            let cookie = cookie(headers, &session.token, SESSION_SECONDS);
            let body = Json(SignedIn { user, token: None });
            ([(header::SET_COOKIE, cookie)], body).into_response()
        }
    })
}

pub(crate) async fn state(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Json<AuthState>, ApiError> {
    let user = match presented_token(&headers) {
        Some((token, _)) => app.db.session_user(&token).await?.map(|s| s.user),
        None => None,
    };
    Ok(Json(AuthState {
        setup_required: app.db.users().await?.is_empty(),
        user,
    }))
}

/// Comparaison en temps constant : la durée ne dit pas combien de caractères sont justes.
fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0, |acc, (x, y)| acc | (x ^ y))
            == 0
}

pub(crate) async fn setup(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
    body: Result<Json<Setup>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(setup) = body.map_err(invalid_body)?;
    if !same_secret(setup.token.trim(), app.auth.setup_token()) {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "invalid_setup_token",
            "jeton d'installation incorrect (voir la console de Shiori)",
        ));
    }
    let user = app
        .db
        .create_first_admin(&setup.username, &setup.password)
        .await?
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::CONFLICT,
                "already_set_up",
                "un compte existe déjà : se connecter",
            )
        })?;
    sign_in(&app, &headers, user, setup.client).await
}

pub(crate) async fn login(
    State(app): State<Arc<App>>,
    peer: Option<axum::Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    body: Result<Json<Credentials>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(credentials) = body.map_err(invalid_body)?;
    let peer = peer.map(|axum::Extension(ConnectInfo(addr))| addr);
    let name_key = format!("nom:{}", credentials.username.to_lowercase());
    let keys = [
        (name_key.clone(), FREE_PER_NAME),
        (
            format!("adresse:{}", client_address(peer, &headers)),
            FREE_PER_ADDRESS,
        ),
    ];
    if let Some(wait) = app.auth.paused(&keys) {
        return Err(rate_limited(wait));
    }
    let user = {
        let _permit = app
            .auth
            .hashing
            .acquire()
            .await
            .expect("sémaphore jamais fermé");
        app.db
            .check_password(&credentials.username, &credentials.password)
            .await?
    };
    let Some(user) = user else {
        app.auth.failed(&keys);
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "invalid_credentials",
            "identifiants incorrects",
        ));
    };
    app.auth.succeeded(&name_key);
    sign_in(&app, &headers, user, credentials.client).await
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PasswordChange {
    current: String,
    new: String,
}

/// Change son mot de passe (obligatoire après un mot de passe provisoire) : l'ancien est
/// exigé, sous la même limite d'essais que la connexion (une session volée ne doit pas
/// permettre de le deviner). Les autres appareils sont déconnectés.
pub(crate) async fn change_password(
    State(app): State<Arc<App>>,
    axum::Extension(session): axum::Extension<Session>,
    body: Result<Json<PasswordChange>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(change) = body.map_err(invalid_body)?;
    let name = session.user.username.as_str();
    let name_key = format!("nom:{}", name.to_lowercase());
    let keys = [(name_key.clone(), FREE_PER_NAME)];
    if let Some(wait) = app.auth.paused(&keys) {
        return Err(rate_limited(wait));
    }
    let checked = {
        let _permit = app
            .auth
            .hashing
            .acquire()
            .await
            .expect("sémaphore jamais fermé");
        app.db.check_password(name, &change.current).await?
    };
    if checked.is_none() {
        app.auth.failed(&keys);
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "wrong_password",
            "mot de passe actuel incorrect",
        ));
    }
    app.auth.succeeded(&name_key);
    app.db
        .set_password(name, &change.new, false, Some(session.id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Ferme la session présentée et efface le cookie ; répond 204 même sans session.
pub(crate) async fn logout(
    State(app): State<Arc<App>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    if let Some((token, _)) = presented_token(&headers) {
        app.db.end_session(&token).await?;
    }
    let clear = cookie(&headers, "", 0);
    Ok((StatusCode::NO_CONTENT, [(header::SET_COOKIE, clear)]).into_response())
}

#[cfg(test)]
mod tests;
