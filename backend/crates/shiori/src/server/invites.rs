//! Liens d'invitation (version 1.4, lot 1.4a) : l'administrateur les crée, les liste et les
//! révoque (`/api/admin/invites`) ; la personne invitée, sans session, vérifie le lien et crée
//! son compte (`/api/auth/invite/{token}`). Aucune inscription libre.

use std::net::SocketAddr;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{ConnectInfo, Extension, Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use shiori_db::{Invite, Role, Session};

use super::auth::{Admin, Client, address_limit, client_address, rate_limited, sign_in};
use super::{ApiError, ApiResult, App, int_id, invalid_body};

/// Durées de validité proposées, en jours (Q78).
const DAYS: [u16; 3] = [1, 7, 30];

pub(crate) async fn list(_: Admin, State(app): State<Arc<App>>) -> ApiResult<Vec<Invite>> {
    Ok(Json(app.db.invites().await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewInvite {
    #[serde(default = "reader")]
    role: Role,
    /// Bibliothèques du futur lecteur ; absentes : toutes celles qui existent (Q49).
    libraries: Option<Vec<i64>>,
    #[serde(default = "week")]
    days: u16,
}

fn reader() -> Role {
    Role::Reader
}

fn week() -> u16 {
    7
}

#[derive(Serialize)]
pub(crate) struct Created {
    #[serde(flatten)]
    invite: Invite,
    /// Le jeton en clair, montré une seule fois (l'interface en fait `/invite/<jeton>`).
    token: String,
}

pub(crate) async fn create(
    _: Admin,
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<NewInvite>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    if !DAYS.contains(&new.days) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            "days : 1, 7 ou 30",
        ));
    }
    let created = app
        .db
        .create_invite(session.user.id, new.role, new.libraries, new.days)
        .await?;
    let body = Created {
        invite: created.invite,
        token: created.token,
    };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

pub(crate) async fn revoke(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.revoke_invite(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("invitation")),
    }
}

fn invalid() -> ApiError {
    ApiError::new(
        StatusCode::NOT_FOUND,
        "invite_invalid",
        "lien d'invitation expiré, déjà utilisé ou révoqué",
    )
}

#[derive(Serialize)]
pub(crate) struct Valid {
    role: Role,
}

/// Le lien est-il encore bon ? Le rôle prévu, rien d'autre (ni bibliothèques, ni auteur).
pub(crate) async fn check(
    State(app): State<Arc<App>>,
    Path(token): Path<String>,
) -> ApiResult<Valid> {
    match app.db.invite_role(&token).await? {
        Some(role) => Ok(Json(Valid { role })),
        None => Err(invalid()),
    }
}

#[derive(Deserialize)]
pub(crate) struct Accept {
    username: String,
    password: String,
    #[serde(default)]
    client: Client,
}

/// Crée le compte de l'invitation et ouvre sa session. Un lien qui ne vaut rien compte comme un
/// essai raté de l'adresse (limite de la connexion) ; le hachage passe par le même sémaphore.
pub(crate) async fn accept(
    State(app): State<Arc<App>>,
    peer: Option<Extension<ConnectInfo<SocketAddr>>>,
    headers: HeaderMap,
    Path(token): Path<String>,
    body: Result<Json<Accept>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(accept) = body.map_err(invalid_body)?;
    let peer = peer.map(|Extension(ConnectInfo(addr))| addr);
    let limits = address_limit(&client_address(peer, &headers, &app.auth.trusted_proxies));
    if let Some(wait) = app.auth.paused(&limits) {
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
            .accept_invite(&token, &accept.username, &accept.password)
            .await?
    };
    let Some(user) = user else {
        app.auth.failed(&limits);
        return Err(invalid());
    };
    sign_in(&app, &headers, user, accept.client).await
}
