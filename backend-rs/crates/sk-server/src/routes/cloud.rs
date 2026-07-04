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
    b.map(|x| x.0).unwrap_or_else(|| json!({}))
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/providers", get(providers).post(create_provider))
        .route("/providers/{id}", delete(delete_provider))
        .route("/providers/{id}/options", get(provider_options))
        .route("/servers", get(servers).post(create_server))
        .route("/servers/{id}", get(get_server).delete(delete_server))
        .route("/servers/{id}/resize", post(resize_server))
        .route(
            "/servers/{id}/snapshots",
            get(snapshots).post(create_snapshot),
        )
        .route("/snapshots/{id}", delete(delete_snapshot))
        .route("/costs", get(costs))
}
#[derive(Deserialize)]
struct ServerQuery {
    provider_id: Option<String>,
}
async fn providers(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloud::providers(&s.db).await.map_err(internal)?))
}
async fn create_provider(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::create_provider(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_provider(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::delete_provider(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn provider_options(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::provider_options(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn servers(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<ServerQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::servers(&s.db, q.provider_id.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn create_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::create_server(&s.db, &body(b))
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
        sk_cloud::get_server(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn delete_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::delete_server(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn resize_server(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_cloud::resize_server(
            &s.db,
            &id,
            b.get("size").and_then(Value::as_str).unwrap_or(""),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn snapshots(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::snapshots(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn create_snapshot(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    let b = body(b);
    Ok(Json(
        sk_cloud::create_snapshot(
            &s.db,
            &id,
            b.get("name")
                .and_then(Value::as_str)
                .unwrap_or("serverkit-snapshot"),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn delete_snapshot(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_cloud::delete_snapshot(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn costs(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_cloud::costs(&s.db).await.map_err(internal)?))
}
