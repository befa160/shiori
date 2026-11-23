//! Tomes (`/api/volumes`) : fiche, édition, progression, pages ; statistiques de lecture.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use shiori_core::PageId;
use shiori_db::{Edited, SaveProgress};

use super::auth::{Admin, Me};
use super::{ApiError, ApiResult, App, invalid_body, invalid_query, volume_id};

pub(super) async fn volume(
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

pub(super) async fn patch_volume(
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
pub(super) async fn progress(
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
pub(super) struct NewProgress {
    page_id: String,
    /// Heure de lecture (ISO 8601), pour une page lue hors ligne et envoyée plus tard.
    read_at: Option<String>,
    /// Minutes à l'est d'UTC de l'appareil : le jour compté par les statistiques (UTC sinon).
    utc_offset: Option<i32>,
    /// `false` : n'entre pas dans les statistiques (« Marquer comme lu »).
    stats: Option<bool>,
}

pub(super) async fn save_progress(
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
    let saved = app.db.save_progress_at(
        &me,
        &volume,
        &page,
        new.read_at.as_deref(),
        (new.stats != Some(false)).then(|| offset(new.utc_offset)),
    );
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
pub(super) async fn clear_progress(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.clear_progress(&me, &volume_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("tome")),
    }
}

pub(super) async fn pages(
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

/// Décalage horaire reçu d'un appareil, borné aux fuseaux qui existent (UTC-14 à UTC+14).
pub(super) fn offset(minutes: Option<i32>) -> i32 {
    minutes.unwrap_or(0).clamp(-14 * 60, 14 * 60)
}

#[derive(Deserialize)]
pub(super) struct StatsQuery {
    /// Jours de la période, aujourd'hui compris : 84 (12 semaines) par défaut, 366 au plus.
    days: Option<u32>,
    utc_offset: Option<i32>,
}

/// Statistiques de lecture du compte (`/api/me/stats`), jamais celles d'un autre (Q61).
pub(super) async fn reading_stats(
    State(app): State<Arc<App>>,
    Me(me): Me,
    query: Result<Query<StatsQuery>, QueryRejection>,
) -> ApiResult<shiori_db::ReadingStats> {
    let Query(q) = query.map_err(invalid_query)?;
    let days = q.days.unwrap_or(84).clamp(1, 366);
    Ok(Json(
        app.db
            .reading_stats(&me, days, offset(q.utc_offset))
            .await?,
    ))
}
