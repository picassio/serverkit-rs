//! Port of `app/api/docker.py` (containers, images, networks, volumes).
//! Compose endpoints are P2 (they belong with the app/deploy port).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};
use sk_docker::ContainerSpec;

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/info", get(info))
        .route("/disk-usage", get(disk_usage))
        .route("/containers", get(list_containers).post(create_container))
        .route("/containers/run", post(run_container))
        .route("/containers/stats", post(containers_stats))
        .route(
            "/containers/{id}",
            get(get_container).delete(remove_container),
        )
        .route("/containers/{id}/start", post(start_container))
        .route("/containers/{id}/stop", post(stop_container))
        .route("/containers/{id}/restart", post(restart_container))
        .route("/containers/{id}/logs", get(container_logs))
        .route("/containers/{id}/stats", get(container_stats))
        .route("/containers/{id}/exec", post(exec_container))
        .route("/images", get(list_images))
        .route("/images/pull", post(pull_image))
        .route("/images/tag", post(tag_image))
        .route("/images/{id}", delete(remove_image))
        .route("/networks", get(list_networks).post(create_network))
        .route("/networks/{id}", delete(remove_network))
        .route("/volumes", get(list_volumes).post(create_volume))
        .route("/volumes/{name}", delete(remove_volume))
}

/// `@admin_required` equivalent.
fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

/// `_reject_if_protected` — lifecycle actions on ServerKit's own containers
/// would take the panel offline.
async fn reject_if_protected(container_id: &str) -> ApiResult<()> {
    if sk_docker::is_protected_container(container_id).await {
        return Err(ApiError::forbidden(
            "This is a ServerKit system container and cannot be controlled from here. \
             Managing it would take the panel offline.",
        ));
    }
    Ok(())
}

fn result_status(result: &Value, ok: StatusCode) -> StatusCode {
    if result
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        ok
    } else {
        StatusCode::BAD_REQUEST
    }
}

// ==================== STATUS ====================

async fn status(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_docker::status().await)
}

async fn info(AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    match sk_docker::info().await {
        Some(info) => Ok(Json(json!({ "info": info }))),
        None => Err(ApiError::bad_request("Could not get Docker info")),
    }
}

async fn disk_usage(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "usage": sk_docker::disk_usage().await }))
}

// ==================== CONTAINERS ====================

#[derive(Deserialize)]
struct ListQuery {
    all: Option<String>,
}

async fn list_containers(AuthUser(_u): AuthUser, Query(q): Query<ListQuery>) -> Json<Value> {
    let all = q
        .all
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(true);
    Json(json!({ "containers": sk_docker::list_containers(all).await }))
}

async fn get_container(AuthUser(_u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    match sk_docker::inspect_container(&id).await {
        Some(c) => Ok(Json(json!({ "container": c }))),
        None => Err(ApiError::not_found("Container not found")),
    }
}

#[derive(Deserialize)]
struct ContainerBody {
    image: Option<String>,
    name: Option<String>,
    #[serde(default)]
    ports: Vec<String>,
    #[serde(default)]
    volumes: Vec<String>,
    #[serde(default)]
    env: Map<String, Value>,
    network: Option<String>,
    restart_policy: Option<String>,
    command: Option<String>,
}

impl ContainerBody {
    fn into_spec(self) -> ApiResult<ContainerSpec> {
        let image = self
            .image
            .ok_or_else(|| ApiError::bad_request("image is required"))?;
        Ok(ContainerSpec {
            image,
            name: self.name,
            ports: self.ports,
            volumes: self.volumes,
            env: self.env,
            network: self.network,
            restart_policy: Some(
                self.restart_policy
                    .unwrap_or_else(|| "unless-stopped".into()),
            ),
            command: self.command,
        })
    }
}

async fn create_container(
    AuthUser(u): AuthUser,
    Json(body): Json<ContainerBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_docker::create_container(&body.into_spec()?).await;
    Ok((result_status(&result, StatusCode::CREATED), Json(result)))
}

async fn run_container(
    AuthUser(u): AuthUser,
    Json(body): Json<ContainerBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_docker::run_container(&body.into_spec()?).await;
    Ok((result_status(&result, StatusCode::CREATED), Json(result)))
}

async fn start_container(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_docker::start_container(&id).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

#[derive(Deserialize, Default)]
struct TimeoutBody {
    timeout: Option<i64>,
}

async fn stop_container(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<TimeoutBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    reject_if_protected(&id).await?;
    let timeout = body.and_then(|b| b.0.timeout).unwrap_or(10);
    let result = sk_docker::stop_container(&id, timeout).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

async fn restart_container(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<TimeoutBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    reject_if_protected(&id).await?;
    let timeout = body.and_then(|b| b.0.timeout).unwrap_or(10);
    let result = sk_docker::restart_container(&id, timeout).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

#[derive(Deserialize, Default)]
struct RemoveBody {
    force: Option<bool>,
    volumes: Option<bool>,
}

async fn remove_container(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<RemoveBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    reject_if_protected(&id).await?;
    let b = body.map(|b| b.0).unwrap_or_default();
    let result =
        sk_docker::remove_container(&id, b.force.unwrap_or(false), b.volumes.unwrap_or(false))
            .await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

#[derive(Deserialize)]
struct LogsQuery {
    tail: Option<i64>,
    since: Option<String>,
}

async fn container_logs(
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<LogsQuery>,
) -> Json<Value> {
    Json(sk_docker::container_logs(&id, q.tail.unwrap_or(100), q.since.as_deref()).await)
}

async fn container_stats(AuthUser(_u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    match sk_docker::container_stats(&id).await {
        Some(stats) => Ok(Json(json!({ "stats": stats }))),
        None => Err(ApiError::bad_request("Could not get stats")),
    }
}

#[derive(Deserialize, Default)]
struct StatsBody {
    ids: Option<Vec<String>>,
    container_ids: Option<Vec<String>>,
}

async fn containers_stats(AuthUser(_u): AuthUser, body: Option<Json<StatsBody>>) -> Json<Value> {
    let b = body.map(|b| b.0).unwrap_or_default();
    let ids = b.ids.or(b.container_ids).unwrap_or_default();
    Json(json!({ "stats": sk_docker::containers_stats(&ids).await }))
}

#[derive(Deserialize)]
struct ExecBody {
    command: Option<String>,
}

async fn exec_container(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(body): Json<ExecBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let command = body
        .command
        .ok_or_else(|| ApiError::bad_request("command is required"))?;
    Ok(Json(sk_docker::exec_command(&id, &command).await))
}

// ==================== IMAGES ====================

async fn list_images(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "images": sk_docker::list_images().await }))
}

#[derive(Deserialize)]
struct PullBody {
    image: Option<String>,
    tag: Option<String>,
}

async fn pull_image(
    AuthUser(u): AuthUser,
    Json(body): Json<PullBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let image = body
        .image
        .ok_or_else(|| ApiError::bad_request("image is required"))?;
    let result = sk_docker::pull_image(&image, body.tag.as_deref().unwrap_or("latest")).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

#[derive(Deserialize)]
struct TagBody {
    source: Option<String>,
    target: Option<String>,
}

async fn tag_image(
    AuthUser(u): AuthUser,
    Json(body): Json<TagBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let (Some(source), Some(target)) = (body.source, body.target) else {
        return Err(ApiError::bad_request("source and target are required"));
    };
    let result = sk_docker::tag_image(&source, &target).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

#[derive(Deserialize, Default)]
struct ForceBody {
    force: Option<bool>,
}

async fn remove_image(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<ForceBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let force = body.and_then(|b| b.0.force).unwrap_or(false);
    let result = sk_docker::remove_image(&id, force).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

// ==================== NETWORKS ====================

async fn list_networks(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "networks": sk_docker::list_networks().await }))
}

#[derive(Deserialize)]
struct NetworkBody {
    name: Option<String>,
    driver: Option<String>,
}

async fn create_network(
    AuthUser(u): AuthUser,
    Json(body): Json<NetworkBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let name = body
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    let result = sk_docker::create_network(&name, body.driver.as_deref().unwrap_or("bridge")).await;
    Ok((result_status(&result, StatusCode::CREATED), Json(result)))
}

async fn remove_network(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_docker::remove_network(&id).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}

// ==================== VOLUMES ====================

async fn list_volumes(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "volumes": sk_docker::list_volumes().await }))
}

async fn create_volume(
    AuthUser(u): AuthUser,
    Json(body): Json<NetworkBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let name = body
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    let result = sk_docker::create_volume(&name, body.driver.as_deref().unwrap_or("local")).await;
    Ok((result_status(&result, StatusCode::CREATED), Json(result)))
}

async fn remove_volume(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    body: Option<Json<ForceBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let force = body.and_then(|b| b.0.force).unwrap_or(false);
    let result = sk_docker::remove_volume(&name, force).await;
    Ok((result_status(&result, StatusCode::OK), Json(result)))
}
