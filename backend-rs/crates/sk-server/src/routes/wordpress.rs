use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    b.map(|x| x.0).unwrap_or_else(|| json!({}))
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/standalone/status", get(status))
        .route("/standalone/requirements", get(requirements))
        .route("/standalone/install", post(install))
        .route("/standalone/uninstall", post(uninstall))
        .route("/standalone/start", post(start))
        .route("/standalone/stop", post(stop))
        .route("/standalone/restart", post(restart))
}
async fn status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_wordpress::status(&s.db).await.map_err(internal)?))
}
async fn requirements(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_wordpress::requirements().await.map_err(internal)?))
}
async fn install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_wordpress::install(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn uninstall(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_wordpress::uninstall(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn start(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_wordpress::start(&s.db).await.map_err(internal)?))
}
async fn stop(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_wordpress::stop(&s.db).await.map_err(internal)?))
}
async fn restart(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_wordpress::restart(&s.db).await.map_err(internal)?))
}
