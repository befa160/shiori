//! Collections partagées (`/api/collections`, 1.3d) : lues par tout compte (ses séries visibles
//! seulement), gérées par l'administrateur.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use shiori_db::{Collection, CollectionError, Viewer};

use super::auth::{Admin, Me};
use super::{ApiError, ApiResult, App, int_id, invalid_body};

impl From<CollectionError> for ApiError {
    fn from(e: CollectionError) -> Self {
        match e {
            CollectionError::InvalidName => {
                ApiError::new(StatusCode::BAD_REQUEST, "invalid_name", e.to_string())
            }
            CollectionError::NameTaken(_) => {
                ApiError::new(StatusCode::CONFLICT, "name_taken", e.to_string())
            }
            CollectionError::Db(e) => e.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Named {
    name: String,
}

pub(super) async fn list(State(app): State<Arc<App>>, Me(me): Me) -> ApiResult<Vec<Collection>> {
    Ok(Json(app.db.collections(&me).await?))
}

/// La collection telle que ce compte (administrateur) la voit.
async fn one(app: &App, me: &Viewer, id: i64) -> Result<Collection, ApiError> {
    app.db
        .collections(me)
        .await?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| ApiError::not_found("collection"))
}

/// 201 et la collection, vide.
pub(super) async fn create(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    body: Result<Json<Named>, JsonRejection>,
) -> Result<Response, ApiError> {
    let Json(body) = body.map_err(invalid_body)?;
    let id = app.db.create_collection(&body.name).await?;
    Ok((StatusCode::CREATED, Json(one(&app, &me, id).await?)).into_response())
}

pub(super) async fn rename(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<Named>, JsonRejection>,
) -> ApiResult<Collection> {
    let id = int_id(&id)?;
    let Json(body) = body.map_err(invalid_body)?;
    if !app.db.rename_collection(id, &body.name).await? {
        return Err(ApiError::not_found("collection"));
    }
    Ok(Json(one(&app, &me, id).await?))
}

pub(super) async fn delete(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.delete_collection(int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("collection")),
    }
}

/// Ajoute une série ; renvoie la collection.
pub(super) async fn add_series(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, series)): Path<(String, String)>,
) -> ApiResult<Collection> {
    let id = int_id(&id)?;
    if !app.db.add_to_collection(id, int_id(&series)?).await? {
        return Err(ApiError::not_found("collection ou série"));
    }
    Ok(Json(one(&app, &me, id).await?))
}

/// Retire une série ; renvoie la collection.
pub(super) async fn remove_series(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, series)): Path<(String, String)>,
) -> ApiResult<Collection> {
    let id = int_id(&id)?;
    if !app.db.remove_from_collection(id, int_id(&series)?).await? {
        return Err(ApiError::not_found("collection"));
    }
    Ok(Json(one(&app, &me, id).await?))
}
