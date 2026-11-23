//! Réglages de l'instance (`/api/settings`) : lus par tous, changés par l'administrateur.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::StatusCode;
use serde::Deserialize;
use shiori_db::{Language, PageTurn, Theme};
use shiori_meta::SourceId;

use super::auth::Admin;
use super::{ApiError, ApiResult, App, invalid_body, present};

pub(super) async fn settings(State(app): State<Arc<App>>) -> ApiResult<shiori_db::Settings> {
    Ok(Json(app.db.settings().await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewSettings {
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

pub(super) async fn save_settings(
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
