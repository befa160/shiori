//! Bibliothèques (`/api/libraries`) : liste, création, changement, suppression, séries.

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use shiori_db::LibraryKind;

use super::auth::{Admin, Me};
use super::{ApiError, ApiResult, App, int_id, invalid_body};

pub(super) async fn libraries(
    State(app): State<Arc<App>>,
    Me(me): Me,
) -> ApiResult<Vec<shiori_db::LibrarySummary>> {
    Ok(Json(app.db.library_summaries(&me).await?))
}

#[derive(Deserialize)]
pub(super) struct NewLibrary {
    name: String,
    root: PathBuf,
    /// Donne le sens de lecture par défaut des séries (Q12) ; `manga` si absent.
    #[serde(default)]
    kind: LibraryKind,
    /// Ouverte aux lecteurs existants ; sinon visible de l'administrateur seul (Q49).
    #[serde(default)]
    share: bool,
}

#[derive(Serialize)]
pub(super) struct CreatedLibrary {
    id: i64,
    name: String,
    root: String,
    kind: LibraryKind,
}

pub(super) async fn create_library(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewLibrary>, JsonRejection>,
) -> Result<(StatusCode, Json<CreatedLibrary>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let library = app
        .db
        .create_library(&new.name, &new.root, new.kind)
        .await?;
    if new.share {
        app.db.share_library(library.id).await?;
    }
    Ok((
        StatusCode::CREATED,
        Json(CreatedLibrary {
            id: library.id,
            name: library.name,
            root: library.root,
            kind: library.kind,
        }),
    ))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct LibraryChange {
    watch: bool,
}

pub(super) async fn patch_library(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    body: Result<Json<LibraryChange>, JsonRejection>,
) -> Result<StatusCode, ApiError> {
    let Json(change) = body.map_err(invalid_body)?;
    match app.db.set_library_watch(int_id(&id)?, change.watch).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("bibliothèque")),
    }
}

pub(super) async fn delete_library(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.delete_library(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("bibliothèque")),
    }
}

pub(super) async fn library_series(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    app.db
        .series_of(&me, int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("bibliothèque"))
}
