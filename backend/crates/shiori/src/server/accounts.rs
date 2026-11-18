//! Compte connecté (`/api/me`) et gestion des comptes par l'administrateur
//! (`/api/admin/users`), phase 7, lot 7.4b.

use std::sync::Arc;

use axum::extract::rejection::JsonRejection;
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::{Json, response::IntoResponse, response::Response};
use serde::{Deserialize, Serialize};
use shiori_db::{
    Device, Language, PageTurn, Preferences, Role, Session, Theme, User, UserChange,
    temporary_password,
};

use super::auth::Admin;
use super::{ApiError, ApiResult, App, int_id, invalid_body, present};

#[derive(Serialize)]
pub(crate) struct MeView {
    user: User,
    /// `null` = défaut de l'instance.
    preferences: Preferences,
    /// Double authentification active, et codes de secours restants.
    totp: bool,
    recovery_codes_left: i64,
}

async fn me_view(app: &App, user: User, preferences: Preferences) -> Result<MeView, ApiError> {
    Ok(MeView {
        totp: app.db.totp_enabled(user.id).await?,
        recovery_codes_left: app.db.recovery_codes_left(user.id).await?,
        preferences,
        user,
    })
}

pub(crate) async fn me(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
) -> ApiResult<MeView> {
    let preferences = app.db.preferences(session.user.id).await?;
    Ok(Json(me_view(&app, session.user, preferences).await?))
}

/// Champ absent = inchangé, `null` = revenir au défaut de l'instance.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PreferencesPatch {
    #[serde(default, deserialize_with = "present")]
    language: Option<Option<Language>>,
    #[serde(default, deserialize_with = "present")]
    theme: Option<Option<Theme>>,
    #[serde(default, deserialize_with = "present")]
    page_turn: Option<Option<PageTurn>>,
}

pub(crate) async fn save_me(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    body: Result<Json<PreferencesPatch>, JsonRejection>,
) -> ApiResult<MeView> {
    let Json(patch) = body.map_err(invalid_body)?;
    let current = app.db.preferences(session.user.id).await?;
    let preferences = Preferences {
        language: patch.language.unwrap_or(current.language),
        theme: patch.theme.unwrap_or(current.theme),
        page_turn: patch.page_turn.unwrap_or(current.page_turn),
    };
    let preferences = app.db.set_preferences(session.user.id, preferences).await?;
    Ok(Json(me_view(&app, session.user, preferences).await?))
}

#[derive(Serialize)]
pub(crate) struct DeviceView {
    #[serde(flatten)]
    device: Device,
    /// L'appareil d'où vient la requête.
    current: bool,
}

pub(crate) async fn devices(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
) -> ApiResult<Vec<DeviceView>> {
    let devices = app.db.devices(session.user.id).await?;
    Ok(Json(
        devices
            .into_iter()
            .map(|device| DeviceView {
                current: device.id == session.id,
                device,
            })
            .collect(),
    ))
}

pub(crate) async fn revoke_device(
    State(app): State<Arc<App>>,
    Extension(session): Extension<Session>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.revoke_device(session.user.id, int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("appareil")),
    }
}

#[derive(Serialize)]
pub(crate) struct UserView {
    #[serde(flatten)]
    user: User,
    /// Bibliothèques ouvertes (un administrateur les voit toutes, sans ligne ici).
    libraries: Vec<i64>,
    /// Double authentification active.
    totp: bool,
}

async fn view(app: &App, user: User) -> Result<UserView, ApiError> {
    Ok(UserView {
        libraries: app.db.user_libraries(user.id).await?,
        totp: app.db.totp_enabled(user.id).await?,
        user,
    })
}

async fn user_or_404(app: &App, raw: &str) -> Result<User, ApiError> {
    app.db
        .user_by_id(int_id(raw)?)
        .await?
        .ok_or_else(|| ApiError::not_found("compte"))
}

pub(crate) async fn users(_: Admin, State(app): State<Arc<App>>) -> ApiResult<Vec<UserView>> {
    let mut out = Vec::new();
    for user in app.db.users().await? {
        out.push(view(&app, user).await?);
    }
    Ok(Json(out))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct NewUser {
    username: String,
    #[serde(default = "reader")]
    role: Role,
    /// Bibliothèques ouvertes à un lecteur ; absentes : toutes celles qui existent (Q49).
    libraries: Option<Vec<i64>>,
}

fn reader() -> Role {
    Role::Reader
}

/// Compte créé avec un mot de passe provisoire (Q45), montré une seule fois.
#[derive(Serialize)]
pub(crate) struct Created {
    user: UserView,
    password: String,
}

pub(crate) async fn create_user(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewUser>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let password = temporary_password();
    let mut user = app
        .db
        .create_user(&new.username, &password, new.role)
        .await?;
    if let Some(libraries) = new.libraries {
        let change = UserChange {
            libraries: Some(libraries),
            ..UserChange::default()
        };
        user = app.db.update_user(user.id, change).await?.unwrap_or(user);
    }
    let created = Created {
        user: view(&app, user).await?,
        password,
    };
    Ok((StatusCode::CREATED, Json(created)).into_response())
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UserPatch {
    role: Option<Role>,
    disabled: Option<bool>,
    libraries: Option<Vec<i64>>,
}

pub(crate) async fn update_user(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<UserPatch>, JsonRejection>,
) -> ApiResult<UserView> {
    let Json(patch) = body.map_err(invalid_body)?;
    let change = UserChange {
        role: patch.role,
        disabled: patch.disabled,
        libraries: patch.libraries,
    };
    let user = app
        .db
        .update_user(int_id(&id)?, change)
        .await?
        .ok_or_else(|| ApiError::not_found("compte"))?;
    Ok(Json(view(&app, user).await?))
}

pub(crate) async fn delete_user(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.delete_user(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("compte")),
    }
}

#[derive(Serialize)]
pub(crate) struct NewPassword {
    password: String,
}

/// Nouveau mot de passe provisoire (mot de passe oublié) ; ferme les sessions du compte.
pub(crate) async fn reset_password(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<NewPassword> {
    let user = user_or_404(&app, &id).await?;
    let password = temporary_password();
    app.db
        .set_password(&user.username, &password, true, None)
        .await?;
    Ok(Json(NewPassword { password }))
}

/// Déconnecte tous les appareils d'un compte (appareil perdu).
pub(crate) async fn revoke_sessions(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let user = user_or_404(&app, &id).await?;
    app.db.revoke_devices(user.id).await?;
    Ok(StatusCode::NO_CONTENT)
}
