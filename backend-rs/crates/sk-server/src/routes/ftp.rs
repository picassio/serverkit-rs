use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    match b.map(|x| x.0).unwrap_or_else(|| json!({})) {
        Value::String(s) => serde_json::from_str(&s).unwrap_or_else(|_| json!({"value": s})),
        v => v,
    }
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/config", get(config).post(set_config))
        .route("/users", get(users).post(create_user))
        .route("/users/{username}", delete(delete_user))
        .route("/users/{username}/password", post(change_password))
        .route("/users/{username}/toggle", post(toggle_user))
        .route("/connections", get(connections))
        .route("/connections/{pid}", delete(disconnect))
        .route("/logs", get(logs))
        .route("/install", post(install))
        .route("/service/{action}", post(service))
        .route("/test", post(test))
}

#[derive(Deserialize)]
struct Q {
    service: Option<String>,
    lines: Option<i64>,
    delete_home: Option<bool>,
}
async fn status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::status(&s.db).await.map_err(internal)?))
}
async fn config(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<Q>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::config(&s.db, q.service.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn set_config(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::set_config(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn users(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::users(&s.db).await.map_err(internal)?))
}
async fn create_user(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::create_user(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_user(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(username): Path<String>,
    Query(q): Query<Q>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::delete_user(&s.db, &username, q.delete_home.unwrap_or(false))
            .await
            .map_err(internal)?,
    ))
}
async fn change_password(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(username): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::change_password(&s.db, &username, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn toggle_user(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(username): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::toggle_user(&s.db, &username, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn connections(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::connections().await.map_err(internal)?))
}
async fn disconnect(AuthUser(_): AuthUser, Path(pid): Path<String>) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::disconnect(&pid).await.map_err(internal)?))
}
async fn logs(AuthUser(_): AuthUser, Query(q): Query<Q>) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::logs(q.lines.unwrap_or(100))
            .await
            .map_err(internal)?,
    ))
}
async fn install(AuthUser(_): AuthUser, b: Option<Json<Value>>) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::install(&body(b)).await.map_err(internal)?))
}
async fn service(
    AuthUser(_): AuthUser,
    Path(action): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_ftp::service(&action, &body(b)).await.map_err(internal)?,
    ))
}
async fn test(AuthUser(_): AuthUser, b: Option<Json<Value>>) -> ApiResult<Json<Value>> {
    Ok(Json(sk_ftp::test(&body(b)).await.map_err(internal)?))
}
