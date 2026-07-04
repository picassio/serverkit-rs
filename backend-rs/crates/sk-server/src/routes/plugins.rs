use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Multipart, Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
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
#[derive(Deserialize)]
struct StatusQ {
    status: Option<String>,
}
pub fn plugin_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list))
        .route("/install", post(install_url))
        .route("/install-local", post(install_local))
        .route("/install-upload", post(install_upload))
        .route("/contributions", get(contrib))
        .route("/builtin", get(builtin))
        .route("/builtin/{slug}/install", post(install_builtin))
        .route("/updates", get(updates))
        .route("/{id}", get(get_plugin).delete(uninstall))
        .route("/{id}/enable", post(enable))
        .route("/{id}/disable", post(disable))
        .route("/{id}/config", get(config).put(update_config))
        .route("/{id}/update", post(update_plugin))
}
pub fn marketplace_router() -> Router<SharedState> {
    Router::new()
        .route("/registry", get(registry))
        .route("/registry/{slug}/install", post(install_registry))
}
pub fn agent_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(agent_list).post(agent_create))
        .route("/spec", get(agent_spec))
        .route("/server/{id}", get(server_plugins))
        .route("/installs/{id}/enable", post(install_enable))
        .route("/installs/{id}/disable", post(install_disable))
        .route("/installs/{id}", axum::routing::delete(install_delete))
        .route("/installs/{id}/config", axum::routing::put(install_config))
        .route(
            "/{id}",
            get(agent_get).put(agent_update).delete(agent_delete),
        )
        .route("/{id}/install", post(agent_install))
        .route("/{id}/bulk-install", post(agent_bulk))
        .route("/{id}/installations", get(agent_installs))
}
async fn list(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<StatusQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::plugins(&s.db, q.status.as_deref())
            .await
            .map_err(internal)?,
    ))
}

pub async fn plugin_root_alias(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::plugins(&s.db, None).await.map_err(internal)?,
    ))
}
async fn get_plugin(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::plugin(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn install_url(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::install_url(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn install_local(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::install_local(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn install_upload(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    mut m: Multipart,
) -> ApiResult<Json<Value>> {
    let mut name = "upload.zip".to_string();
    let mut bytes = 0usize;
    while let Some(f) = m
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?
    {
        if let Some(n) = f.file_name() {
            name = n.to_string()
        }
        let data = f
            .bytes()
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        bytes += data.len();
    }
    Ok(Json(
        sk_plugins::install_upload(&s.db, &name, bytes)
            .await
            .map_err(internal)?,
    ))
}
async fn uninstall(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::uninstall(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn enable(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::set_status(&s.db, &id, "active")
            .await
            .map_err(internal)?,
    ))
}
async fn disable(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::set_status(&s.db, &id, "disabled")
            .await
            .map_err(internal)?,
    ))
}
async fn contrib(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::contributions(&s.db).await.map_err(internal)?,
    ))
}
async fn builtin(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::builtin_extensions(&s.db)
            .await
            .map_err(internal)?,
    ))
}
async fn install_builtin(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::install_builtin(&s.db, &slug)
            .await
            .map_err(internal)?,
    ))
}
async fn config(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::config(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn update_config(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::update_config(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn updates(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_plugins::updates(&s.db).await.map_err(internal)?))
}
async fn update_plugin(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::update_plugin(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn registry(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::marketplace(&s.db).await.map_err(internal)?,
    ))
}
async fn install_registry(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(slug): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::install_builtin(&s.db, &slug)
            .await
            .map_err(internal)?,
    ))
}
async fn agent_list(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<StatusQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::agent_plugins(&s.db, q.status.as_deref())
            .await
            .map_err(internal)?,
    ))
}

pub async fn agent_root_alias(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::agent_plugins(&s.db, None)
            .await
            .map_err(internal)?,
    ))
}

async fn agent_create(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::create_agent(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}

pub async fn agent_create_root_alias(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::create_agent(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn agent_get(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_plugins::agent(&s.db, &id).await.map_err(internal)?))
}
async fn agent_update(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::update_agent(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn agent_delete(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::delete_agent(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn agent_install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::install_agent(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn agent_bulk(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::bulk_install_agent(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn agent_installs(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::installations(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn server_plugins(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::server_plugins(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn install_enable(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::set_install_status(&s.db, &id, "enabled")
            .await
            .map_err(internal)?,
    ))
}
async fn install_disable(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::set_install_status(&s.db, &id, "disabled")
            .await
            .map_err(internal)?,
    ))
}
async fn install_delete(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::delete_install(&s.db, &id)
            .await
            .map_err(internal)?,
    ))
}
async fn install_config(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_plugins::update_install_config(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn agent_spec(AuthUser(_): AuthUser) -> Json<Value> {
    Json(sk_plugins::agent_spec())
}
