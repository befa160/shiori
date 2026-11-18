//! Sauvegardes de la base (`/api/admin/backups`, phase 7, lot 7.8) : liste, sauvegarde à la
//! demande, téléchargement, téléversement et restauration, Shiori allumé.

use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use http_body_util::BodyExt;
use serde::Serialize;
use shiori_db::RestoreError;
use tokio::io::AsyncWriteExt;
use tower::ServiceExt;
use tower_http::services::ServeFile;

use super::auth::Admin;
use super::{ApiError, ApiResult, App};
use crate::backup::BackupFile;

#[derive(Serialize)]
pub(crate) struct Listing {
    dir: String,
    /// `null` : pas de sauvegarde périodique.
    every_hours: Option<u64>,
    keep: usize,
    files: Vec<BackupFile>,
}

pub(crate) async fn list(_: Admin, State(app): State<Arc<App>>) -> Json<Listing> {
    let b = &app.backups;
    Json(Listing {
        dir: super::absolute(&b.dir),
        every_hours: b.every.map(|d| d.as_secs() / 3600),
        keep: b.keep,
        files: b.list(),
    })
}

pub(crate) async fn create(_: Admin, State(app): State<Arc<App>>) -> Result<Response, ApiError> {
    let file = app.backups.create(&app.db).await?;
    Ok((StatusCode::CREATED, Json(file)).into_response())
}

fn named(app: &App, name: &str) -> Result<std::path::PathBuf, ApiError> {
    app.backups
        .path(name)
        .ok_or_else(|| ApiError::not_found("sauvegarde"))
}

pub(crate) async fn download(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(name): Path<String>,
    request: Request,
) -> Result<Response, ApiError> {
    let path = named(&app, &name)?;
    let mut response = ServeFile::new(path)
        .oneshot(request)
        .await
        .unwrap_or_else(|never| match never {})
        .map(Body::new);
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/vnd.sqlite3"),
    );
    // Nom déjà validé : lettres, chiffres, `-`, `_` et `.`.
    if let Ok(value) = HeaderValue::from_str(&format!("attachment; filename=\"{name}\"")) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    headers.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

pub(crate) async fn remove(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let path = named(&app, &name)?;
    std::fs::remove_file(path).map_err(shiori_db::Error::Io)?;
    Ok(StatusCode::NO_CONTENT)
}

/// Fichier envoyé tel quel dans le corps (`application/octet-stream`), gardé sous
/// `shiori-importee-<date>.db` ; à restaurer ensuite par son nom.
pub(crate) async fn upload(
    _: Admin,
    State(app): State<Arc<App>>,
    body: Body,
) -> ApiResult<BackupFile> {
    let dir = &app.backups.dir;
    std::fs::create_dir_all(dir).map_err(shiori_db::Error::Io)?;
    let part = shiori_db::temp_file(dir);
    let written = receive(body, &part).await;
    if let Err(e) = written {
        let _ = std::fs::remove_file(&part);
        return Err(e);
    }
    let path = app.db.free_name(dir, "shiori-importee").await?;
    std::fs::rename(&part, &path).map_err(shiori_db::Error::Io)?;
    let name = path.file_name().unwrap_or_default().to_string_lossy();
    let file = app.backups.list().into_iter().find(|f| f.name == name);
    file.map(Json)
        .ok_or_else(|| ApiError::not_found("sauvegarde"))
}

/// Écrit le corps dans `part` ; refuse ce qui ne commence pas comme une base SQLite.
async fn receive(body: Body, part: &std::path::Path) -> Result<(), ApiError> {
    let io = |e: std::io::Error| ApiError::from(shiori_db::Error::Io(e));
    let mut out = tokio::fs::File::create(part).await.map_err(io)?;
    let mut body = body;
    let mut head: Vec<u8> = Vec::with_capacity(16);
    while let Some(frame) = body.frame().await {
        let frame = frame
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_body", e.to_string()))?;
        if let Ok(data) = frame.into_data() {
            if head.len() < 16 {
                head.extend(data.iter().take(16 - head.len()));
            }
            out.write_all(&data).await.map_err(io)?;
        }
    }
    out.flush().await.map_err(io)?;
    if head != b"SQLite format 3\0" {
        return Err(invalid(RestoreError::NotABackup));
    }
    Ok(())
}

/// Sauvegarde d'abord la base actuelle (`shiori-avant-restauration-<date>.db`), puis la
/// remplace par la sauvegarde `name`. Les sessions sont fermées : 204, puis reconnexion.
pub(crate) async fn restore(
    _: Admin,
    State(app): State<Arc<App>>,
    Path(name): Path<String>,
) -> Result<StatusCode, ApiError> {
    let path = named(&app, &name)?;
    // Un scan écrit pendant toute sa passe : il écrirait dans une base qui n'est plus la sienne.
    if app.scanner.progress().await.is_some() {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "scan_already_running",
            "un scan est en cours : attendez sa fin ou annulez-le",
        ));
    }
    let dir = &app.backups.dir;
    app.db.backup(dir, "shiori-avant-restauration").await?;
    app.db.restore(&path, dir).await.map_err(invalid)?;
    tracing::warn!("base restaurée depuis {name}");
    Ok(StatusCode::NO_CONTENT)
}

fn invalid(e: RestoreError) -> ApiError {
    let code = match e {
        RestoreError::Db(e) => return e.into(),
        RestoreError::NotABackup => "invalid_backup",
        RestoreError::TooNew => "backup_too_new",
    };
    ApiError::new(StatusCode::UNPROCESSABLE_ENTITY, code, e.to_string())
}
