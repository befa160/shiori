//! Séries (`/api/series`, `/api/continue`) : listes, fiche, édition, lu / non lu, réglages de lecture.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Deserialize;
use shiori_db::{EditError, Edited, PageLayout, PageTurn, ReadingMode, SettingsChange, Viewer};

use super::auth::{Admin, Me};
use super::{ApiError, ApiResult, App, int_id, invalid_body, invalid_query, present};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SeriesQuery {
    library_id: Option<i64>,
}

/// Toutes les séries, ou celles d'une bibliothèque (`?library_id=`).
pub(super) async fn all_series(
    State(app): State<Arc<App>>,
    Me(me): Me,
    query: Result<Query<SeriesQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::SeriesSummary>> {
    let Query(q) = query.map_err(invalid_query)?;
    match q.library_id {
        None => Ok(Json(app.db.all_series(&me).await?)),
        Some(id) => app
            .db
            .series_of(&me, id)
            .await?
            .map(Json)
            .ok_or_else(|| ApiError::not_found("bibliothèque")),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContinueQuery {
    limit: Option<u32>,
}

/// « Continuer la lecture » : 20 entrées par défaut, 100 au plus.
pub(super) async fn continue_reading(
    State(app): State<Arc<App>>,
    Me(me): Me,
    query: Result<Query<ContinueQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::ContinueItem>> {
    let Query(q) = query.map_err(invalid_query)?;
    let limit = q.limit.unwrap_or(20).clamp(1, 100) as usize;
    Ok(Json(app.db.continue_reading(&me, limit).await?))
}

/// Nouveautés des séries suivies (1.3a), voir [`shiori_db::Db::new_volumes`].
pub(super) async fn new_volumes(
    State(app): State<Arc<App>>,
    Me(me): Me,
) -> ApiResult<Vec<shiori_db::NewVolume>> {
    Ok(Json(app.db.new_volumes(&me).await?))
}

/// « Tout marquer comme vu ».
pub(super) async fn mark_new_seen(
    State(app): State<Arc<App>>,
    Me(me): Me,
) -> Result<StatusCode, ApiError> {
    app.db.mark_new_seen(&me).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn follow(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.follow(&me, int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("série")),
    }
}

pub(super) async fn unfollow(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    match app.db.unfollow(&me, int_id(&id)?).await? {
        true => Ok(StatusCode::NO_CONTENT),
        false => Err(ApiError::not_found("série")),
    }
}

impl From<EditError> for ApiError {
    fn from(e: EditError) -> Self {
        match e {
            EditError::Invalid(msg) => ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", msg),
            EditError::Db(e) => e.into(),
        }
    }
}

/// Modifie les métadonnées d'une série et renvoie son détail. Chaque champ donné est édité
/// (verrouillé, `null` compris) ; `unlock: [...]` rend des champs aux sources.
pub(super) async fn patch_series(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<Edited>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    if !app.db.edit_series(id, patch).await? {
        return Err(ApiError::not_found("série"));
    }
    app.db
        .series_detail(&me, id)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

pub(super) async fn series(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    app.db
        .series_detail(&me, int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}

/// Détail d'une série, ou 404.
pub(super) async fn series_or_404(
    app: &App,
    me: &Viewer,
    id: i64,
) -> Result<shiori_db::SeriesDetail, ApiError> {
    app.db
        .series_detail(me, id)
        .await?
        .ok_or_else(|| ApiError::not_found("série"))
}

/// « Marquer la série comme lue » pour ce compte ; renvoie son détail.
pub(super) async fn mark_series_read(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    mark_series(&app, &me, &id, true).await
}

/// « Marquer la série comme non lue » : la progression de tous ses tomes est effacée.
pub(super) async fn mark_series_unread(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    mark_series(&app, &me, &id, false).await
}

pub(super) async fn mark_series(
    app: &App,
    me: &Viewer,
    id: &str,
    read: bool,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(id)?;
    series_or_404(app, me, id).await?;
    app.db.mark_series(me, id, read).await?;
    Ok(Json(series_or_404(app, me, id).await?))
}

/// Champ absent = inchangé, `null` = revenir au défaut de la bibliothèque.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SettingsPatch {
    #[serde(default, deserialize_with = "present")]
    reading_mode: Option<Option<ReadingMode>>,
    #[serde(default, deserialize_with = "present")]
    page_layout: Option<Option<PageLayout>>,
    #[serde(default, deserialize_with = "present")]
    page_turn: Option<Option<PageTurn>>,
}

pub(super) async fn series_settings(
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<SettingsPatch>, JsonRejection>,
) -> ApiResult<shiori_db::ReadingSettings> {
    let id = int_id(&id)?;
    let Json(patch) = body.map_err(invalid_body)?;
    let change = SettingsChange {
        reading_mode: patch.reading_mode,
        page_layout: patch.page_layout,
        page_turn: patch.page_turn,
    };
    app.db
        .change_series_settings(&me, id, change)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("série"))
}
