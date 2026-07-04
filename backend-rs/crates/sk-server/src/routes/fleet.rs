use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
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

pub fn pairing_router() -> Router<SharedState> {
    Router::new()
        .route("/lookup", post(pair_lookup))
        .route("/claim", post(pair_claim))
}
pub fn tunnels_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(tunnels).post(create_tunnel))
        .route("/{id}", get(get_tunnel).delete(delete_tunnel))
        .route("/{id}/services", get(tunnel_services).post(publish_service))
        .route("/{id}/services/{service_id}", delete(unpublish_service))
}
pub fn templates_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(templates).post(create_template))
        .route("/library", get(template_library))
        .route("/library/{key}", post(create_from_library))
        .route("/compliance", get(template_compliance))
        .route("/server/{server_id}", get(server_assignments))
        .route("/assignments/{id}", delete(unassign_template))
        .route("/assignments/{id}/check", post(check_assignment))
        .route("/assignments/{id}/remediate", post(remediate_assignment))
        .route(
            "/{id}",
            get(get_template)
                .put(update_template)
                .delete(delete_template),
        )
        .route("/{id}/assign", post(assign_template))
        .route("/{id}/bulk-assign", post(bulk_assign_template))
        .route("/{id}/assignments", get(template_assignments))
}
pub fn monitor_router() -> Router<SharedState> {
    Router::new()
        .route("/heatmap", get(heatmap))
        .route("/comparison", get(comparison))
        .route("/alerts", get(alerts))
        .route("/alerts/{id}/acknowledge", post(ack_alert))
        .route("/alerts/{id}/resolve", post(resolve_alert))
        .route("/thresholds", get(thresholds).post(create_threshold))
        .route("/thresholds/{id}", delete(delete_threshold))
        .route("/anomalies", get(anomalies))
        .route("/forecast/{server_id}", get(forecast))
        .route("/search", get(search))
}

async fn pair_lookup(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::lookup_pairing(&s.db, b.get("code").and_then(Value::as_str).unwrap_or(""))
            .await
            .map_err(internal)?,
    ))
}
async fn pair_claim(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::claim_pairing(&s.db, &b).await.map_err(internal)?,
    ))
}
pub async fn tunnels(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::tunnels(&s.db).await.map_err(internal)?))
}
pub async fn create_tunnel(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_tunnel(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn get_tunnel(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::get_tunnel(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn delete_tunnel(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::delete_tunnel(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn tunnel_services(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::tunnel_services(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn publish_service(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::publish_service(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn unpublish_service(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((id, service_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::unpublish_service(&s.db, &id, &service_id)
            .await
            .map_err(internal)?,
    ))
}

pub async fn templates(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::templates(&s.db, q.get("category").map(String::as_str))
            .await
            .map_err(internal)?,
    ))
}
async fn template_library(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_fleet::template_library().await.map_err(internal)?))
}
async fn create_from_library(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_template_from_library(&s.db, &key)
            .await
            .map_err(internal)?,
    ))
}
async fn get_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::get_template(&s.db, &id).await.map_err(internal)?,
    ))
}
pub async fn create_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_template(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::update_template(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::delete_template(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn assign_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_fleet::assign_template(
            &s.db,
            &id,
            b.get("server_id")
                .and_then(Value::as_str)
                .unwrap_or("local"),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn bulk_assign_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_fleet::bulk_assign_template(
            &s.db,
            &id,
            b.get("server_ids").cloned().unwrap_or_else(|| json!([])),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn template_assignments(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::template_assignments(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn server_assignments(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(server_id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::server_template_assignments(&s.db, &server_id)
            .await
            .map_err(internal)?,
    ))
}
async fn unassign_template(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::unassign_template(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn check_assignment(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::check_assignment(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn remediate_assignment(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::remediate_assignment(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn template_compliance(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::template_compliance(&s.db)
            .await
            .map_err(internal)?,
    ))
}

#[derive(Deserialize)]
struct Limit {
    status: Option<String>,
    limit: Option<i64>,
    server_id: Option<String>,
    group_id: Option<String>,
    ids: Option<String>,
    metric: Option<String>,
    period: Option<String>,
    q: Option<String>,
    r#type: Option<String>,
}
async fn heatmap(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::heatmap(&s.db, q.group_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn comparison(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::comparison(
            &s.db,
            q.ids.as_deref(),
            q.metric.as_deref(),
            q.period.as_deref(),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn alerts(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::alerts(&s.db, q.status.as_deref(), q.limit.unwrap_or(100))
            .await
            .map_err(internal)?,
    ))
}
async fn ack_alert(
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
async fn resolve_alert(
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
async fn thresholds(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::thresholds(&s.db, q.server_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn create_threshold(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::create_threshold(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_threshold(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::delete_threshold(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn anomalies(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::anomalies(&s.db, q.server_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn forecast(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(server_id): Path<String>,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::forecast(&s.db, &server_id, q.metric.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn search(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Limit>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_fleet::search(&s.db, q.q.as_deref(), q.r#type.as_deref())
            .await
            .map_err(internal)?,
    ))
}
