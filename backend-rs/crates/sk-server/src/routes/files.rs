//! Port of `app/api/files.py` (local filesystem half; s3/* comes with the
//! storage-provider port in P4).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::body::Body;
use axum::extract::{Multipart, Query};
use axum::http::{header, StatusCode};
use axum::response::Response;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use tokio::task::spawn_blocking;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/browse", get(browse))
        .route("/info", get(info))
        .route("/read", get(read))
        .route("/write", post(write))
        .route("/create", post(create))
        .route("/mkdir", post(mkdir))
        .route("/delete", delete(delete_path))
        .route("/rename", post(rename))
        .route("/copy", post(copy))
        .route("/move", post(move_path))
        .route("/chmod", post(chmod))
        .route("/search", get(search))
        .route("/disk-usage", get(disk_usage))
        .route("/disk-mounts", get(disk_mounts))
        .route("/analyze", get(analyze))
        .route("/download", get(download))
        .route("/upload", post(upload))
}

/// Flask maps 'denied' errors to 403, everything else to 400.
fn respond(result: Value, ok: StatusCode) -> (StatusCode, Json<Value>) {
    let success = result
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let status = if success {
        ok
    } else if result
        .get("error")
        .and_then(|e| e.as_str())
        .map(|e| e.to_lowercase().contains("denied"))
        .unwrap_or(false)
    {
        StatusCode::FORBIDDEN
    } else {
        StatusCode::BAD_REQUEST
    };
    (status, Json(result))
}

async fn blocking<F: FnOnce() -> Value + Send + 'static>(f: F) -> ApiResult<Value> {
    spawn_blocking(f)
        .await
        .map_err(|e| anyhow::Error::from(e).into())
}

#[derive(Deserialize)]
struct BrowseQuery {
    path: Option<String>,
    show_hidden: Option<String>,
}

async fn browse(
    AuthUser(_u): AuthUser,
    Query(q): Query<BrowseQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q.path.unwrap_or_else(|| "/home".into());
    let hidden = q
        .show_hidden
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    Ok(respond(
        blocking(move || sk_files::list_directory(&path, hidden)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct PathQuery {
    path: Option<String>,
}

async fn info(
    AuthUser(_u): AuthUser,
    Query(q): Query<PathQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    if !sk_files::is_path_allowed(&path) {
        return Err(ApiError::forbidden("Access denied"));
    }
    let result = blocking(move || {
        sk_files::file_info(&path)
            .map(|f| serde_json::json!({ "success": true, "file": f }))
            .unwrap_or_else(|| serde_json::json!({ "error": "File not found" }))
    })
    .await?;
    if result.get("success").is_some() {
        Ok((StatusCode::OK, Json(result)))
    } else {
        Ok((StatusCode::NOT_FOUND, Json(result)))
    }
}

async fn read(
    AuthUser(_u): AuthUser,
    Query(q): Query<PathQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    Ok(respond(
        blocking(move || sk_files::read_file(&path)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct WriteBody {
    path: Option<String>,
    content: Option<String>,
    create_backup: Option<bool>,
}

async fn write(
    AuthUser(_u): AuthUser,
    Json(b): Json<WriteBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = b
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    let content = b
        .content
        .ok_or_else(|| ApiError::bad_request("Content is required"))?;
    let backup = b.create_backup.unwrap_or(true);
    Ok(respond(
        blocking(move || sk_files::write_file(&path, &content, backup)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct CreateBody {
    path: Option<String>,
    #[serde(default)]
    content: String,
}

async fn create(
    AuthUser(_u): AuthUser,
    Json(b): Json<CreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = b
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    Ok(respond(
        blocking(move || sk_files::create_file(&path, &b.content)).await?,
        StatusCode::CREATED,
    ))
}

async fn mkdir(
    AuthUser(_u): AuthUser,
    Json(b): Json<PathBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = b
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    Ok(respond(
        blocking(move || sk_files::create_directory(&path)).await?,
        StatusCode::CREATED,
    ))
}

#[derive(Deserialize)]
struct PathBody {
    path: Option<String>,
}

async fn delete_path(
    AuthUser(_u): AuthUser,
    Query(q): Query<PathQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    Ok(respond(
        blocking(move || sk_files::delete(&path)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct RenameBody {
    path: Option<String>,
    new_name: Option<String>,
}

async fn rename(
    AuthUser(_u): AuthUser,
    Json(b): Json<RenameBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (Some(path), Some(new_name)) = (b.path, b.new_name) else {
        return Err(ApiError::bad_request("Path and new_name are required"));
    };
    Ok(respond(
        blocking(move || sk_files::rename(&path, &new_name)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct SrcDestBody {
    src: Option<String>,
    dest: Option<String>,
}

async fn copy(
    AuthUser(_u): AuthUser,
    Json(b): Json<SrcDestBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (Some(src), Some(dest)) = (b.src, b.dest) else {
        return Err(ApiError::bad_request(
            "Source and destination paths are required",
        ));
    };
    Ok(respond(
        blocking(move || sk_files::copy(&src, &dest)).await?,
        StatusCode::OK,
    ))
}

async fn move_path(
    AuthUser(_u): AuthUser,
    Json(b): Json<SrcDestBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (Some(src), Some(dest)) = (b.src, b.dest) else {
        return Err(ApiError::bad_request(
            "Source and destination paths are required",
        ));
    };
    Ok(respond(
        blocking(move || sk_files::move_path(&src, &dest)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct ChmodBody {
    path: Option<String>,
    mode: Option<String>,
}

async fn chmod(
    AuthUser(_u): AuthUser,
    Json(b): Json<ChmodBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (Some(path), Some(mode)) = (b.path, b.mode) else {
        return Err(ApiError::bad_request("Path and mode are required"));
    };
    Ok(respond(
        blocking(move || sk_files::chmod(&path, &mode)).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct SearchQuery {
    directory: Option<String>,
    pattern: Option<String>,
    max_results: Option<usize>,
}

async fn search(
    AuthUser(_u): AuthUser,
    Query(q): Query<SearchQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let pattern = q
        .pattern
        .ok_or_else(|| ApiError::bad_request("Search pattern is required"))?;
    let dir = q.directory.unwrap_or_else(|| "/home".into());
    let max = q.max_results.unwrap_or(100);
    Ok(respond(
        blocking(move || sk_files::search(&dir, &pattern, max)).await?,
        StatusCode::OK,
    ))
}

async fn disk_usage(
    AuthUser(_u): AuthUser,
    Query(q): Query<PathQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q.path.unwrap_or_else(|| "/".into());
    Ok(respond(
        blocking(move || sk_files::disk_usage(&path)).await?,
        StatusCode::OK,
    ))
}

async fn disk_mounts(AuthUser(_u): AuthUser) -> ApiResult<(StatusCode, Json<Value>)> {
    Ok(respond(
        blocking(sk_files::disk_mounts).await?,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct AnalyzeQuery {
    path: Option<String>,
    depth: Option<u32>,
    limit: Option<usize>,
}

async fn analyze(
    AuthUser(_u): AuthUser,
    Query(q): Query<AnalyzeQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = q.path.unwrap_or_else(|| "/home".into());
    let depth = q.depth.unwrap_or(2);
    let limit = q.limit.unwrap_or(20);
    Ok(respond(
        blocking(move || sk_files::analyze(&path, depth, limit)).await?,
        StatusCode::OK,
    ))
}

/// GET /files/download — streamed attachment (Flask `send_file`).
async fn download(AuthUser(_u): AuthUser, Query(q): Query<PathQuery>) -> ApiResult<Response> {
    let path = q
        .path
        .ok_or_else(|| ApiError::bad_request("Path is required"))?;
    if !sk_files::is_path_allowed(&path) {
        return Err(ApiError::forbidden("Access denied"));
    }
    let p = std::path::Path::new(&path);
    if !p.exists() {
        return Err(ApiError::not_found("File not found"));
    }
    if p.is_dir() {
        return Err(ApiError::bad_request("Cannot download directory"));
    }

    let file = tokio::fs::File::open(&path)
        .await
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let name = p
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "download".into());
    let stream = tokio_util::io::ReaderStream::new(file);

    Response::builder()
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{name}\""),
        )
        .body(Body::from_stream(stream))
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))
}

/// POST /files/upload — multipart with `file` + `destination` fields.
async fn upload(
    AuthUser(_u): AuthUser,
    mut multipart: Multipart,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let mut file_bytes: Option<(String, Vec<u8>)> = None;
    let mut destination: Option<String> = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?
    {
        match field.name() {
            Some("file") => {
                let filename = field.file_name().unwrap_or("").to_string();
                let data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::bad_request(e.to_string()))?;
                file_bytes = Some((filename, data.to_vec()));
            }
            Some("destination") => {
                destination = field.text().await.ok();
            }
            _ => {}
        }
    }

    let (filename, data) = file_bytes.ok_or_else(|| ApiError::bad_request("No file provided"))?;
    let destination =
        destination.ok_or_else(|| ApiError::bad_request("Destination path is required"))?;

    if filename.is_empty() {
        return Err(ApiError::bad_request("No file selected"));
    }
    if !sk_files::is_path_allowed(&destination) {
        return Err(ApiError::forbidden("Access denied"));
    }
    if data.len() as u64 > sk_files::MAX_UPLOAD_SIZE {
        return Err(ApiError::bad_request(
            "File too large. Maximum size is 100.0 MB",
        ));
    }

    let result = spawn_blocking(move || {
        let dest = std::path::Path::new(&destination);
        let full_path = if dest.is_dir() {
            dest.join(&filename)
        } else {
            dest.to_path_buf()
        };
        let full_str = full_path.to_string_lossy().into_owned();

        if !sk_files::is_path_allowed(&full_str) {
            return (
                StatusCode::FORBIDDEN,
                serde_json::json!({ "error": "Access denied" }),
            );
        }
        if let Some(parent) = full_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        match std::fs::write(&full_path, &data) {
            Ok(_) => (
                StatusCode::CREATED,
                serde_json::json!({ "success": true, "path": full_str, "size": data.len() }),
            ),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => (
                StatusCode::FORBIDDEN,
                serde_json::json!({ "error": "Permission denied" }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                serde_json::json!({ "error": e.to_string() }),
            ),
        }
    })
    .await
    .map_err(anyhow::Error::from)?;

    Ok((result.0, Json(result.1)))
}
