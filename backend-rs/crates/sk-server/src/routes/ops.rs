//! Ports of `app/api/logs.py` and `app/api/processes.py`.
//! All endpoints are admin-only (parity with `@admin_required`).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn logs_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_logs))
        .route("/read", get(read_log))
        .route("/search", get(search_log))
        .route("/journal", get(journal))
        .route("/clear", post(clear_log))
}

pub fn processes_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_processes))
        .route("/services", get(services_status))
        .route("/services/{name}", post(control_service))
        .route("/services/{name}/logs", get(service_logs))
        .route("/{pid}", get(process_details).delete(kill_process))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

fn status_of(v: &Value) -> StatusCode {
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(true) {
        StatusCode::OK
    } else {
        StatusCode::BAD_REQUEST
    }
}

// ==================== LOGS ====================

async fn list_logs(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({ "logs": sk_ops::logs::log_files() })))
}

#[derive(Deserialize)]
struct ReadQuery {
    path: Option<String>,
    lines: Option<i64>,
    from_end: Option<String>,
}

async fn read_log(
    AuthUser(u): AuthUser,
    Query(q): Query<ReadQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let path = q
        .path
        .ok_or_else(|| ApiError::bad_request("path parameter is required"))?;
    let from_end = q
        .from_end
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    let result = sk_ops::logs::read_log(&path, q.lines.unwrap_or(100), from_end).await;
    Ok((status_of(&result), Json(result)))
}

#[derive(Deserialize)]
struct SearchQuery {
    path: Option<String>,
    pattern: Option<String>,
    lines: Option<i64>,
}

async fn search_log(
    AuthUser(u): AuthUser,
    Query(q): Query<SearchQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let (Some(path), Some(pattern)) = (q.path, q.pattern) else {
        return Err(ApiError::bad_request(
            "path and pattern parameters are required",
        ));
    };
    let result = sk_ops::logs::search_log(&path, &pattern, q.lines.unwrap_or(100)).await;
    Ok((status_of(&result), Json(result)))
}

#[derive(Deserialize)]
struct JournalQuery {
    unit: Option<String>,
    lines: Option<i64>,
    since: Option<String>,
    priority: Option<String>,
}

async fn journal(
    AuthUser(u): AuthUser,
    Query(q): Query<JournalQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_ops::logs::journal_logs(
        q.unit.as_deref().filter(|s| !s.is_empty()),
        q.lines.unwrap_or(100),
        q.since.as_deref(),
        q.priority.as_deref(),
    )
    .await;
    Ok((status_of(&result), Json(result)))
}

#[derive(Deserialize)]
struct ClearBody {
    path: Option<String>,
}

async fn clear_log(
    AuthUser(u): AuthUser,
    Json(body): Json<ClearBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let path = body
        .path
        .ok_or_else(|| ApiError::bad_request("path is required"))?;
    let result = sk_ops::logs::clear_log(&path).await;
    Ok((status_of(&result), Json(result)))
}

// ==================== PROCESSES ====================

#[derive(Deserialize)]
struct ProcQuery {
    limit: Option<usize>,
    sort: Option<String>,
}

async fn list_processes(
    AuthUser(u): AuthUser,
    Query(q): Query<ProcQuery>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let procs =
        sk_system::processes::list(q.limit.unwrap_or(50), q.sort.as_deref().unwrap_or("cpu")).await;
    Ok(Json(json!({ "processes": procs })))
}

async fn process_details(AuthUser(u): AuthUser, Path(pid): Path<u32>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    match sk_system::processes::details(pid).await {
        Some(p) => Ok(Json(json!({ "process": p }))),
        None => Err(ApiError::not_found("Process not found")),
    }
}

#[derive(Deserialize)]
struct KillQuery {
    force: Option<String>,
}

async fn kill_process(
    AuthUser(u): AuthUser,
    Path(pid): Path<u32>,
    Query(q): Query<KillQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let force = q
        .force
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let result = sk_system::processes::kill(pid, force).await;
    Ok((status_of(&result), Json(result)))
}

async fn services_status(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let running = sk_system::processes::running_names().await;
    Ok(Json(
        json!({ "services": sk_ops::services::services_status(&running) }),
    ))
}

#[derive(Deserialize)]
struct ServiceBody {
    action: Option<String>,
}

async fn control_service(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Json(body): Json<ServiceBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let action = body.action.ok_or_else(|| {
        ApiError::bad_request("action is required (start, stop, restart, reload)")
    })?;
    let result = sk_ops::services::control_service(&name, &action).await;
    Ok((status_of(&result), Json(result)))
}

#[derive(Deserialize)]
struct ServiceLogsQuery {
    lines: Option<i64>,
}

/// GET /processes/services/{name}/logs — journalctl for one unit.
async fn service_logs(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Query(q): Query<ServiceLogsQuery>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_ops::logs::journal_logs(Some(&name), q.lines.unwrap_or(100), None, None).await;
    Ok((status_of(&result), Json(result)))
}
