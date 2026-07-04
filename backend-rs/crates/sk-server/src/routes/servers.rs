//! Fleet/server route family.
//!
//! Local server operations use real host adapters where safe. Remote/agent
//! operations persist a command in `sk_fleet_commands` and return a typed
//! `AGENT_OFFLINE` state until an agent heartbeat consumes it.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::Method;
use axum::routing::{any, delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    b.map(|x| x.0).unwrap_or_else(|| json!({}))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list_servers).post(create_server))
        .route("/agent/version", get(agent_version))
        .route("/available", get(available))
        .route("/register", post(register_server))
        .route("/groups", get(groups).post(create_group))
        .route("/groups/{id}", put(update_group).delete(delete_group))
        .route("/fleet/health", get(fleet_health))
        .route("/fleet/versions", get(versions).post(add_version))
        .route("/fleet/upgrade", post(upgrade_fleet))
        .route("/fleet/rollout", post(start_rollout))
        .route("/fleet/rollouts", get(rollouts))
        .route("/fleet/rollouts/{id}", get(get_rollout))
        .route("/fleet/rollouts/{id}/cancel", post(cancel_rollout))
        .route("/fleet/discovery", get(discovery).post(start_discovery))
        .route("/fleet/approve/{id}", post(approve_discovery))
        .route("/fleet/reject/{id}", post(reject_discovery))
        .route("/fleet/commands/queued", get(queued_commands))
        .route("/fleet/commands/{id}/retry", post(retry_command))
        .route("/fleet/diagnostics/{id}", get(diagnostics))
        .route("/metrics/compare", get(metrics_compare))
        .route("/metrics/retention", get(metrics_retention))
        .route("/metrics/cleanup", post(metrics_cleanup))
        .route("/proxy/overview", get(proxy_overview))
        .route("/security/alerts", get(all_security_alerts))
        .route("/security/alerts/counts", get(security_alert_counts))
        .route(
            "/security/alerts/{id}/acknowledge",
            post(ack_security_alert),
        )
        .route(
            "/security/alerts/{id}/resolve",
            post(resolve_security_alert),
        )
        .route("/terminal/sessions", get(list_sessions))
        .route("/terminal/{sid}/input", post(terminal_input))
        .route("/terminal/{sid}/resize", post(terminal_resize))
        .route("/terminal/{sid}", delete(terminal_close))
        .route(
            "/{server_id}",
            get(get_server).put(update_server).delete(delete_server),
        )
        .route("/{server_id}/workspace", put(set_workspace))
        .route("/{server_id}/regenerate-token", post(regenerate_token))
        .route(
            "/{server_id}/allowed-ips",
            get(allowed_ips).put(set_allowed_ips),
        )
        .route("/{server_id}/security/alerts", get(server_security_alerts))
        .route("/{server_id}/terminal", post(terminal_create))
        .route("/{server_id}/{*path}", any(server_gateway))
}

async fn list_servers(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::servers_list(&s.db).await.map_err(internal)?))
}
async fn create_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_server_record(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn get_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::get_server_record(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn update_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::update_server_record(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::delete_server_record(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn set_workspace(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_fleet::set_workspace(&s.db, &id, b.get("workspace_id").and_then(Value::as_str))
            .await
            .map_err(internal)?,
    ))
}
async fn regenerate_token(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_fleet::regenerate_token(&s.db, &id, b.get("expires_in").and_then(Value::as_i64))
            .await
            .map_err(internal)?,
    ))
}
async fn register_server(
    State(s): State<SharedState>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::register_server(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn available(AuthUser(_): AuthUser) -> Json<Value> {
    Json(
        json!([{"id":"local","name":"Local (this server)","status":"online","is_local":true,"capabilities":{"terminal":true,"docker":true,"files":true,"system":true}}]),
    )
}
async fn agent_version(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::agent_version().await.map_err(internal)?))
}
async fn groups(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::groups(&s.db).await.map_err(internal)?))
}
async fn create_group(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_group(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_group(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::update_group(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_group(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::delete_group(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn fleet_health(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::fleet_health(&s.db).await.map_err(internal)?))
}
async fn versions(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::versions(&s.db).await.map_err(internal)?))
}
async fn add_version(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::add_version(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn upgrade_fleet(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::upgrade_fleet(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn start_rollout(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::start_rollout(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
#[derive(Deserialize)]
struct FleetQuery {
    status: Option<String>,
    server_id: Option<String>,
    duration: Option<i64>,
    ids: Option<String>,
    metric: Option<String>,
    period: Option<String>,
}
async fn rollouts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::rollouts(&s.db, q.status.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn get_rollout(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::get_rollout(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn cancel_rollout(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::cancel_rollout(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn discovery(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::discovery(&s.db).await.map_err(internal)?))
}
async fn start_discovery(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::start_discovery(&s.db, q.duration.unwrap_or(10))
            .await
            .map_err(internal)?,
    ))
}
async fn approve_discovery(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::approve_discovery(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn reject_discovery(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::reject_discovery(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn queued_commands(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::queued_commands(&s.db, q.server_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn retry_command(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::retry_command(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn diagnostics(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::diagnostics(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn metrics_compare(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::metrics_compare(
            &s.db,
            q.ids.as_deref(),
            q.metric.as_deref(),
            q.period.as_deref(),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn metrics_retention(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::metrics_retention().await.map_err(internal)?))
}
async fn metrics_cleanup(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::metrics_cleanup().await.map_err(internal)?))
}
async fn proxy_overview(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::proxy_overview().await.map_err(internal)?))
}
async fn all_security_alerts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::security_alerts(&s.db, q.server_id.as_deref(), q.status.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn server_security_alerts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::security_alerts(&s.db, Some(&id), None)
            .await
            .map_err(internal)?,
    ))
}
async fn security_alert_counts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<FleetQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::security_counts(&s.db, q.server_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn ack_security_alert(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::set_alert_status(&s.db, &id, "acknowledged")
            .await
            .map_err(internal)?,
    ))
}
async fn resolve_security_alert(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::set_alert_status(&s.db, &id, "resolved")
            .await
            .map_err(internal)?,
    ))
}
async fn allowed_ips(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::allowed_ips(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn set_allowed_ips(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_fleet::set_allowed_ips(
            &s.db,
            &id,
            b.get("allowed_ips").cloned().unwrap_or_else(|| json!([])),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn server_gateway(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    method: Method,
    Path((server_id, path)): Path<(String, String)>,
    Query(q): Query<HashMap<String, String>>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::server_route(&s.db, method.as_str(), &server_id, &path, json!(q), body(b))
            .await
            .map_err(internal)?,
    ))
}

fn require_developer(user: &sk_models::user::User) -> ApiResult<()> {
    if !matches!(user.role(), "admin" | "developer") {
        return Err(ApiError::forbidden("Developer access required"));
    }
    Ok(())
}
#[derive(Deserialize, Default)]
struct CreateBody {
    cols: Option<u16>,
    rows: Option<u16>,
}
async fn terminal_create(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(server_id): Path<String>,
    body: Option<Json<CreateBody>>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    let b = body.map(|b| b.0).unwrap_or_default();
    if server_id != "local" {
        return Ok(Json(
            json!({"success":false,"error":"Agent not connected","code":"AGENT_OFFLINE"}),
        ));
    }
    Ok(Json(state.terminal.create_session(
        u.id,
        b.cols.unwrap_or(80),
        b.rows.unwrap_or(24),
        state.term_events.clone(),
    )))
}
#[derive(Deserialize)]
struct InputBody {
    data: Option<String>,
}
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
async fn terminal_close(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(sid): Path<String>,
) -> ApiResult<Json<Value>> {
    require_developer(&u)?;
    Ok(Json(state.terminal.close(&sid, u.id)))
}
async fn list_sessions(State(state): State<SharedState>, AuthUser(u): AuthUser) -> Json<Value> {
    let sessions = state.terminal.user_sessions(u.id);
    Json(json!({ "count": sessions.len(), "sessions": sessions }))
}
