//! Jetons d'API du compte connecté (`/api/me/tokens`, phase 7, lot 7.6).

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use shiori_db::{ApiToken, Session};

use super::{ApiError, ApiResult, App, int_id, invalid_body};

pub(crate) async fn list(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
) -> ApiResult<Vec<ApiToken>> {
    Ok(Json(app.db.tokens(session.user.id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewToken {
    name: String,
    /// Durée de validité en jours ; absente : sans expiration.
    expires_in_days: Option<u16>,
}

#[derive(Serialize)]
pub(crate) struct Created {
    #[serde(flatten)]
    info: ApiToken,
    /// Le jeton en clair, montré une seule fois.
    token: String,
}

pub(crate) async fn create(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<NewToken>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let name = new.name.trim();
    if !(1..=64).contains(&name.chars().count()) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_token_name",
            "nom du jeton : 1 à 64 caractères",
        ));
    }
    if new.expires_in_days == Some(0) {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            "expires_in_days : un jour au moins",
        ));
    }
    let created = app
        .db
        .create_token(session.user.id, name, new.expires_in_days)
        .await?;
    let body = Created {
        info: created.info,
        token: created.token,
    };
    Ok((StatusCode::CREATED, Json(body)).into_response())
}

pub(crate) async fn revoke(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.revoke_token(session.user.id, int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("jeton")),
    }
}
