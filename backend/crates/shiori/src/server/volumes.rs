//! Tomes (`/api/volumes`) : fiche, édition, progression, pages.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use shiori_core::PageId;
use shiori_db::{Edited, SaveProgress};

use super::auth::{Admin, Me};
use super::{ApiError, ApiResult, App, invalid_body, volume_id};

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
