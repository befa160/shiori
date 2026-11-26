//! Marque-pages (1.2c) : ceux du compte connecté, dans un tome ou une série.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use shiori_core::PageId;
use shiori_db::{AddBookmark, Bookmark};

use super::auth::Me;
use super::{ApiError, ApiResult, App, int_id, invalid_body, volume_id};

pub(super) async fn of_volume(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<Vec<Bookmark>> {
    app.db
        .bookmarks_of(&me, &volume_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("tome"))
}

pub(super) async fn of_series(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<Vec<Bookmark>> {
    let id = int_id(&id)?;
    if !app.db.series_visible(&me, id).await? {
        return Err(ApiError::not_found("série"));
    }
    Ok(Json(app.db.series_bookmarks(&me, id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewBookmark {
    page_id: String,
    #[serde(default)]
    note: String,
}

/// 201 et le marque-page ; 200 et celui qui existait si la page était déjà marquée.
pub(super) async fn add(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<NewBookmark>, JsonRejection>,
) -> Result<Response, ApiError> {
    let volume = volume_id(&id)?;
    let Json(new) = body.map_err(invalid_body)?;
    let page: PageId = new
        .page_id
        .parse()
        .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", format!("{e}")))?;
    let note = note(&new.note)?;
    match app.db.add_bookmark(&me, &volume, &page, &note).await? {
        AddBookmark::Added(b) => Ok((StatusCode::CREATED, Json(b)).into_response()),
        AddBookmark::Existing(b) => Ok(Json(b).into_response()),
        AddBookmark::UnknownVolume => Err(ApiError::not_found("tome")),
        AddBookmark::PageNotInVolume => Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "page_not_in_volume",
            "cette page n'appartient pas à ce tome",
        )),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NotePatch {
    note: String,
}

pub(super) async fn set_note(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<NotePatch>, JsonRejection>,
) -> ApiResult<Bookmark> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    let note = note(&patch.note)?;
    app.db
        .set_bookmark_note(&me, id, &note)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("marque-page"))
}

pub(super) async fn remove(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.delete_bookmark(&me, int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("marque-page")),
    }
}

fn note(raw: &str) -> Result<String, ApiError> {
    shiori_db::clean_note(raw).ok_or_else(|| {
        ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_body",
            format!("note : {} caractères au plus", shiori_db::MAX_NOTE),
        )
    })
}
