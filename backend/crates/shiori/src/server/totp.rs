//! Double authentification du compte connecté (`/api/me/totp`) et sa réinitialisation par
//! l'administrateur (phase 7, lot 7.5).

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use qrcode::QrCode;
use qrcode::render::svg;
use serde::{Deserialize, Serialize};
use shiori_db::{Session, TotpError};

use super::auth::{Admin, check_own_code, check_own_password};
use super::{ApiError, ApiResult, App, int_id, invalid_body};

impl From<TotpError> for ApiError {
    fn from(e: TotpError) -> Self {
        match e {
            TotpError::AlreadyEnabled => {
                Self::new(StatusCode::CONFLICT, "totp_enabled", e.to_string())
            }
            TotpError::NotStarted => {
                Self::new(StatusCode::CONFLICT, "totp_not_started", e.to_string())
            }
            TotpError::WrongCode => {
                Self::new(StatusCode::BAD_REQUEST, "invalid_code", e.to_string())
            }
            TotpError::Db(e) => e.into(),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct EnrollmentView {
    /// Base32, pour une saisie à la main.
    secret: String,
    uri: String,
    /// QR de `uri`, en SVG autonome (affiché comme une image, jamais inséré dans la page).
    qr_svg: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Start {
    password: String,
}

/// Commence l'activation : secret et QR, à confirmer par un premier code. Le mot de passe est
/// exigé : une session restée ouverte ne doit pas permettre d'enfermer le titulaire dehors avec
/// le téléphone d'un autre.
pub(crate) async fn start(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<Start>, JsonRejection>,
) -> ApiResult<EnrollmentView> {
    let Json(Start { password }) = body.map_err(invalid_body)?;
    check_own_password(&app, &session.user, &password).await?;
    let enrollment = app
        .db
        .start_totp(session.user.id, &session.user.username)
        .await?;
    let qr_svg = QrCode::new(enrollment.uri.as_bytes())
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", e.to_string()))?
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .quiet_zone(true)
        .build();
    Ok(Json(EnrollmentView {
        secret: enrollment.secret,
        uri: enrollment.uri,
        qr_svg,
    }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Code {
    code: String,
}

#[derive(Serialize)]
pub(crate) struct RecoveryCodes {
    /// Montrés une seule fois.
    recovery_codes: Vec<String>,
}

pub(crate) async fn confirm(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<Code>, JsonRejection>,
) -> ApiResult<RecoveryCodes> {
    let Json(Code { code }) = body.map_err(invalid_body)?;
    let recovery_codes = app.db.confirm_totp(session.user.id, &code).await?;
    Ok(Json(RecoveryCodes { recovery_codes }))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Disable {
    password: String,
    code: String,
}

/// Retire sa double authentification : mot de passe et code exigés (une session restée ouverte
/// ne suffit pas).
pub(crate) async fn disable(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<Disable>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(request) = body.map_err(invalid_body)?;
    if !app.db.totp_enabled(session.user.id).await? {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "totp_disabled",
            "la double authentification n'est pas active",
        ));
    }
    // Code d'abord : un bon mot de passe remet le compteur à zéro, vérifié avant il laisserait
    // essayer tous les codes sans pause.
    check_own_code(&app, &session.user, &request.code).await?;
    check_own_password(&app, &session.user, &request.password).await?;
    app.db.disable_totp(session.user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Téléphone perdu : l'administrateur retire la double authentification d'un compte.
pub(crate) async fn reset(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let id = int_id(&id)?;
    if app.db.user_by_id(id).await?.is_none() {
        return Err(ApiError::not_found("compte"));
    }
    app.db.disable_totp(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
