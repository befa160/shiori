//! Métadonnées en ligne d'une série (`/api/series/{id}/metadata`, `/api/admin/metadata`) :
//! recherche, liens aux sources, rafraîchissement, couverture, identification.

use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use shiori_meta::{FetchError, SourceId, WebMetadata};

use crate::meta::{IdentifyStatus, MetaError};

use super::auth::{Admin, Me};
use super::series::series_or_404;
use super::{ApiError, ApiResult, App, int_id, invalid_body, invalid_query, source_id};

/// Modifie les métadonnées d'un tome, comme [`patch_series`].
impl From<MetaError> for ApiError {
    fn from(e: MetaError) -> Self {
        let message = e.to_string();
        match e {
            MetaError::Disabled => {
                ApiError::new(StatusCode::CONFLICT, "metadata_disabled", message)
            }
            MetaError::UnknownSource => ApiError::not_found("source"),
            MetaError::AlreadyRunning => {
                ApiError::new(StatusCode::CONFLICT, "already_running", message)
            }
            MetaError::Fetch(f) => {
                let status = match f {
                    FetchError::NotFound => StatusCode::NOT_FOUND,
                    FetchError::RateLimited(_) => StatusCode::SERVICE_UNAVAILABLE,
                    _ => StatusCode::BAD_GATEWAY,
                };
                ApiError::new(status, f.code(), message)
            }
            MetaError::Db(e) => e.into(),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MetadataSearch {
    source: SourceId,
    q: String,
}

/// Fiches d'une source pour identifier une série à la main (fenêtre « Identifier »).
pub(super) async fn search_metadata(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    query: Result<Query<MetadataSearch>, QueryRejection>,
) -> ApiResult<Vec<WebMetadata>> {
    let id = int_id(&id)?;
    let Query(q) = query.map_err(invalid_query)?;
    let text = q.q.trim().to_owned();
    if text.is_empty() {
        return Err(ApiError::new(
            StatusCode::BAD_REQUEST,
            "invalid_query",
            "q : un titre à chercher",
        ));
    }
    series_or_404(&app, &me, id).await?;
    let lang = shiori_db::Settings::code(me.language);
    Ok(Json(app.metadata.search(q.source, text, lang).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct NewLink {
    external_id: String,
}

/// Lie une série à la fiche choisie et renvoie son détail.
pub(super) async fn link_source(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, source)): Path<(String, String)>,
    body: Result<Json<NewLink>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    let Json(link) = body.map_err(invalid_body)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.link(id, source, link.external_id).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

/// Délie une source (ses données ne comptent plus) et renvoie le détail de la série.
pub(super) async fn unlink_source(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path((id, source)): Path<(String, String)>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let (id, source) = (int_id(&id)?, source_id(&source)?);
    series_or_404(&app, &me, id).await?;
    app.db.unlink_source(id, source).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

/// Relit les fiches liées d'une série et renvoie son détail.
pub(super) async fn refresh_metadata(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.refresh(id).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct UseCover {
    source: SourceId,
}

/// Prend la couverture de la fiche liée chez une source et renvoie le détail de la série.
pub(super) async fn use_cover(
    _: Admin,
    State(app): State<Arc<App>>,
    Me(me): Me,
    Path(id): Path<String>,
    body: Result<Json<UseCover>, JsonRejection>,
) -> ApiResult<shiori_db::SeriesDetail> {
    let id = int_id(&id)?;
    let Json(body) = body.map_err(invalid_body)?;
    series_or_404(&app, &me, id).await?;
    app.metadata.use_cover(id, body.source).await?;
    Ok(Json(series_or_404(&app, &me, id).await?))
}

#[derive(Serialize)]
pub(super) struct MetadataOverview {
    online: bool,
    /// Ordre appliqué des sources.
    sources: Vec<SourceId>,
    identify: IdentifyStatus,
    /// Sources à confirmer ou en échec.
    issues: Vec<shiori_db::SourceIssue>,
}

pub(super) async fn metadata_status(
    _: Admin,
    State(app): State<Arc<App>>,
) -> ApiResult<MetadataOverview> {
    let settings = app.db.settings().await?;
    Ok(Json(MetadataOverview {
        online: settings.metadata_online,
        sources: settings.source_order(),
        identify: app.metadata.status(),
        issues: app.db.source_issues().await?,
    }))
}

/// Lance l'identification des séries pas encore liées (202) ; 409 si le réseau est désactivé
/// ou si une identification tourne déjà.
pub(super) async fn identify(
    _: Admin,
    State(app): State<Arc<App>>,
) -> Result<StatusCode, ApiError> {
    if !app.db.settings().await?.metadata_online {
        return Err(MetaError::Disabled.into());
    }
    app.metadata.identify()?;
    Ok(StatusCode::ACCEPTED)
}
