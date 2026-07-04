//! Port of the terminal + available-servers slice of `app/api/servers.py`.
//!
//! DIVERGENCE (improvement): terminals work on `server_id == 'local'` via a
//! real PTY (upstream requires a remote agent). Remote-agent terminals return
//! an agent-offline error until the fleet gateway is ported (P5).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        // Single-node fleet: this box only. Empty until the fleet gateway lands.
        .route("/", get(async |AuthUser(_u): AuthUser| Json(json!([]))))
        .route(
            "/groups",
            get(async |AuthUser(_u): AuthUser| Json(json!([]))),
        )
        .route(
            "/fleet/health",
            get(async |AuthUser(_u): AuthUser| {
                Json(json!({ "healthy": 0, "total": 0, "servers": [] }))
            }),
        )
        .route("/available", get(available))
        .route("/terminal/sessions", get(list_sessions))
        .route("/terminal/{sid}/input", post(terminal_input))
        .route("/terminal/{sid}/resize", post(terminal_resize))
        .route("/terminal/{sid}", delete(terminal_close))
        .route("/{server_id}/terminal", post(terminal_create))
}

/// `@developer_required` — admin or developer.
fn require_developer(user: &sk_models::user::User) -> ApiResult<()> {
    if !matches!(user.role(), "admin" | "developer") {
        return Err(ApiError::forbidden("Developer access required"));
    }
    Ok(())
}

/// GET /servers/available — `RemoteDockerService.get_available_servers()`.
/// Local server always present; remote agents come with the fleet port (P5).
async fn available(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!([{
        "id": "local",
        "name": "Local (this server)",
        "status": "online",
        "is_local": true,
        // DIVERGENCE: upstream's local entry has no capabilities because the
        // Flask panel host has no PTY endpoint. Ours does (sk-terminal), so
        // advertising it makes the Terminal tab list the local shell with
        // zero frontend changes.
        "capabilities": { "terminal": true }
    }]))
}

#[derive(Deserialize, Default)]
struct CreateBody {
    cols: Option<u16>,
    rows: Option<u16>,
}

/// POST /servers/{server_id}/terminal
async fn terminal_create(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(server_id): Path<String>,
    body: Option<Json<CreateBody>>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    let b = body.map(|b| b.0).unwrap_or_default();

    if server_id != "local" {
        // Fleet gateway not ported yet — match the Flask agent-offline shape.
        return Ok(Json(json!({
            "success": false,
            "error": "Agent not connected",
            "code": "AGENT_OFFLINE"
        })));
    }

    let result = state.terminal.create_session(
        u.id,
        b.cols.unwrap_or(80),
        b.rows.unwrap_or(24),
        state.term_events.clone(),
    );
    Ok(Json(result))
}

#[derive(Deserialize)]
struct InputBody {
    data: Option<String>,
}

/// POST /servers/terminal/{sid}/input — `data` is base64.
async fn terminal_input(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(sid): Path<String>,
    Json(body): Json<InputBody>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    let data = body
        .data
        .ok_or_else(|| ApiError::bad_request("data is required"))?;
    Ok(Json(state.terminal.send_input(&sid, u.id, &data)))
}

#[derive(Deserialize, Default)]
struct ResizeBody {
    cols: Option<u16>,
    rows: Option<u16>,
}

/// POST /servers/terminal/{sid}/resize
async fn terminal_resize(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(sid): Path<String>,
    body: Option<Json<ResizeBody>>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    let b = body.map(|b| b.0).unwrap_or_default();
    Ok(Json(state.terminal.resize(
        &sid,
        u.id,
        b.cols.unwrap_or(80),
        b.rows.unwrap_or(24),
    )))
}

/// DELETE /servers/terminal/{sid}
async fn terminal_close(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(sid): Path<String>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    Ok(Json(state.terminal.close(&sid, u.id)))
}

/// GET /servers/terminal/sessions
async fn list_sessions(State(state): State<SharedState>, AuthUser(u): AuthUser) -> Json<Value> {
    let sessions = state.terminal.user_sessions(u.id);
    Json(json!({ "count": sessions.len(), "sessions": sessions }))
}
