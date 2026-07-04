use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    match b.map(|x| x.0).unwrap_or_else(|| json!({})) {
        Value::String(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({"value": s})),
        other => other,
    }
}

pub fn status_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(pages).post(create_page))
        .route("/apps", get(apps_status))
        .route("/app/{id}", get(app_status))
        .route("/public/{slug}", get(public_page))
        .route("/badge/{slug}", get(badge))
        .route(
            "/components/{id}",
            put(update_component).delete(delete_component),
        )
        .route("/components/{id}/check", post(run_check))
        .route("/components/{id}/history", get(check_history))
        .route(
            "/incidents/{id}",
            put(update_incident).delete(delete_incident),
        )
        .route("/{id}", get(get_page).put(update_page).delete(delete_page))
        .route("/{id}/components", get(components).post(create_component))
        .route("/{id}/incidents", get(incidents).post(create_incident))
}

pub fn uptime_router() -> Router<SharedState> {
    Router::new()
        .route("/current", get(uptime_current))
        .route("/stats", get(uptime_stats))
        .route("/graph", get(uptime_graph))
        .route("/history", get(uptime_history))
        .route("/tracking/start", post(tracking_start))
        .route("/tracking/stop", post(tracking_stop))
        .route("/tracking/status", get(tracking_status))
}

pub async fn pages(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_status::pages(&s.db).await.map_err(internal)?))
}
pub async fn create_page(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::create_page(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn get_page(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::get_page(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn update_page(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::update_page(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_page(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::delete_page(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn public_page(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::public_page(&s.db, &slug)
            .await
            .map_err(internal)?,
    ))
}
async fn badge(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::badge(&s.db, &slug).await.map_err(internal)?,
    ))
}
async fn components(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::components(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn create_component(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::create_component(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_component(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::update_component(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_component(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::delete_component(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn run_check(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::run_check(&s.db, &id).await.map_err(internal)?,
    ))
}
#[derive(Deserialize)]
struct Hours {
    hours: Option<i64>,
    period: Option<String>,
}
async fn check_history(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<Hours>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::history(&s.db, &id, q.hours.unwrap_or(24))
            .await
            .map_err(internal)?,
    ))
}
async fn incidents(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::incidents(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn create_incident(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::create_incident(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_incident(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::update_incident(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_incident(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::delete_incident(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn apps_status(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_status::apps_status(&s.db).await.map_err(internal)?))
}
async fn app_status(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::app_status(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn uptime_current(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::uptime_current(&s.db).await.map_err(internal)?,
    ))
}
async fn uptime_stats(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::uptime_stats(&s.db).await.map_err(internal)?,
    ))
}
async fn uptime_graph(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Hours>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::uptime_graph(&s.db, q.period.as_deref().unwrap_or("24h"))
            .await
            .map_err(internal)?,
    ))
}
async fn uptime_history(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Hours>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::uptime_history(&s.db, q.hours.unwrap_or(24))
            .await
            .map_err(internal)?,
    ))
}
async fn tracking_start(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::tracking_start(&s.db).await.map_err(internal)?,
    ))
}
async fn tracking_stop(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::tracking_stop(&s.db).await.map_err(internal)?,
    ))
}
async fn tracking_status(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_status::tracking_status(&s.db).await.map_err(internal)?,
    ))
}
