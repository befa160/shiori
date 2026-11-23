//! Scans et santé de la bibliothèque (`/api/admin/scans`, `/api/admin/health`).

use std::path::PathBuf;
use std::sync::Arc;

use axum::Json;
use axum::extract::rejection::{JsonRejection, QueryRejection};
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Deserialize;
use serde_json::json;
use shiori_core::Severity;
use shiori_db::{RunFilter, RunStatus};

use crate::job::{ScanRequest, StartError, Trigger};

use super::auth::Admin;
use super::{ApiError, ApiResult, App, int_id, invalid_body, invalid_query, limit};

impl From<StartError> for ApiError {
    fn from(e: StartError) -> Self {
        let (status, code) = match &e {
            StartError::AlreadyRunning => (StatusCode::CONFLICT, "scan_already_running"),
            StartError::ShuttingDown => (StatusCode::SERVICE_UNAVAILABLE, "shutting_down"),
            StartError::UnknownLibrary => (StatusCode::NOT_FOUND, "not_found"),
            StartError::OutsideLibrary(_) => (StatusCode::BAD_REQUEST, "path_outside_library"),
            StartError::NotFound(_) => (StatusCode::NOT_FOUND, "path_not_found"),
            StartError::Db(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };
        if status.is_server_error() {
            tracing::error!("lancement d'un scan : {e}");
            return Self::new(status, code, "erreur interne");
        }
        Self::new(status, code, e.to_string())
    }
}

#[derive(Deserialize)]
pub(super) struct NewScan {
    library_id: i64,
    /// Dossier ou fichier sous la racine ; absent : toute la bibliothèque.
    path: Option<PathBuf>,
    #[serde(default)]
    deep: bool,
    #[serde(default)]
    rehash: bool,
}

pub(super) async fn start_scan(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Result<Json<NewScan>, JsonRejection>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    let Json(new) = body.map_err(invalid_body)?;
    let request = ScanRequest {
        library_id: new.library_id,
        path: new.path,
        deep: new.deep,
        rehash: new.rehash,
        trigger: Trigger::Manual,
    };
    let run = app.scanner.start(app.db.clone(), request).await?;
    Ok((StatusCode::ACCEPTED, Json(json!({ "run": run }))))
}

/// Progression de la passe en cours, ou 204 si rien ne tourne.
pub(super) async fn current_scan(_: Admin, State(app): State<Arc<App>>) -> Response {
    match app.scanner.progress().await {
        Some(progress) => Json(progress).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

pub(super) async fn cancel_scan(
    _: Admin,
    State(app): State<Arc<App>>,
) -> Result<(StatusCode, Json<serde_json::Value>), ApiError> {
    match app.scanner.cancel().await {
        true => Ok((StatusCode::ACCEPTED, Json(json!({ "cancelling": true })))),
        false => Err(ApiError::new(
            StatusCode::NOT_FOUND,
            "no_scan_running",
            "aucun scan en cours",
        )),
    }
}

/// Santé des bibliothèques : états des tomes, fichiers illisibles, codes présents.
pub(super) async fn health_summary(
    _: Admin,
    State(app): State<Arc<App>>,
) -> ApiResult<shiori_db::Health> {
    Ok(Json(app.db.health().await?))
}

#[derive(Deserialize)]
pub(super) struct RepairQuery {
    library_id: Option<i64>,
    code: Option<shiori_core::IssueCode>,
    /// Chemins après celui-ci (pagination).
    after: Option<String>,
    limit: Option<u32>,
}

pub(super) async fn files_to_repair(
    _: Admin,
    State(app): State<Arc<App>>,
    query: Result<Query<RepairQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::FileHealth>> {
    let Query(q) = query.map_err(invalid_query)?;
    let filter = shiori_db::HealthFilter {
        library_id: q.library_id,
        code: q.code,
        after: q.after,
        limit: limit(q.limit),
    };
    Ok(Json(app.db.files_to_repair(&filter).await?))
}

#[derive(Deserialize)]
pub(super) struct ScansQuery {
    status: Option<RunStatus>,
    library_id: Option<i64>,
    before: Option<i64>,
    limit: Option<u32>,
}

pub(super) async fn scans(
    _: Admin,
    State(app): State<Arc<App>>,
    query: Result<Query<ScansQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::RunSummary>> {
    let Query(q) = query.map_err(invalid_query)?;
    let filter = RunFilter {
        status: q.status,
        library_id: q.library_id,
        before: q.before,
        limit: limit(q.limit),
    };
    Ok(Json(app.db.runs(&filter).await?))
}

pub(super) async fn scan(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
) -> ApiResult<shiori_db::RunSummary> {
    app.db
        .run(int_id(&id)?)
        .await?
        .map(Json)
        .ok_or_else(|| ApiError::not_found("passe de scan"))
}

#[derive(Deserialize)]
pub(super) struct LogsQuery {
    /// Gravité minimale : `info` (tout), `warning`, `error`.
    level: Option<Severity>,
    after: Option<i64>,
    limit: Option<u32>,
}

pub(super) async fn scan_logs(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(id): Path<String>,
    query: Result<Query<LogsQuery>, QueryRejection>,
) -> ApiResult<Vec<shiori_db::LogLine>> {
    let id = int_id(&id)?;
    let Query(q) = query.map_err(invalid_query)?;
    if app.db.run(id).await?.is_none() {
        return Err(ApiError::not_found("passe de scan"));
    }
    let min = q.level.unwrap_or(Severity::Info);
    Ok(Json(
        app.db.run_logs(id, min, q.after, limit(q.limit)).await?,
    ))
}
