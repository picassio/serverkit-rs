use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    match b.map(|x| x.0).unwrap_or_else(|| json!({})) {
        Value::String(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({"value":s})),
        v => v,
    }
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list).post(create))
        .route("/validate", post(validate))
        .route("/executions/{id}", get(execution))
        .route("/executions/{id}/logs", get(logs))
        .route("/{id}", get(get_wf).put(update).delete(delete_wf))
        .route("/{id}/deploy", post(deploy))
        .route("/{id}/execute", post(execute))
        .route("/{id}/executions", get(executions))
}
pub async fn list(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_workflows::list(&s.db).await.map_err(internal)?))
}
pub async fn create(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::create(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn validate(AuthUser(_): AuthUser, b: Option<Json<Value>>) -> Json<Value> {
    Json(sk_workflows::validate_def(&body(b)))
}
async fn get_wf(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_workflows::get(&s.db, &id).await.map_err(internal)?))
}
async fn update(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::update(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_wf(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::delete(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn deploy(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::deploy(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn execute(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::execute(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn executions(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::executions(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn execution(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::execution(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn logs(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_workflows::logs(&s.db, &id).await.map_err(internal)?,
    ))
}
