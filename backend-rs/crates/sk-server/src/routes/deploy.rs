use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};
use std::collections::HashMap;

fn admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}
fn nf(m: &str) -> ApiError {
    ApiError::new(StatusCode::NOT_FOUND, m)
}

pub fn buildpacks_router() -> Router<SharedState> {
    Router::new()
        .route("/detect", post(bp_detect))
        .route("/generate", post(bp_generate))
}
pub fn source_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/admin/{provider}",
            get(source_config).put(put_source_config),
        )
        .route("/{provider}/status", get(source_status))
        .route("/{provider}/authorize", get(authorize))
        .route("/{provider}/callback", post(callback))
        .route("/{provider}", axum::routing::delete(disconnect))
        .route("/{provider}/repos", get(repos))
        .route("/{provider}/repos/{repo}/branches", get(branches))
        .route("/{provider}/repos/{repo}/manifest", get(manifest))
}
pub fn connections_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(connections))
        .route("/registries", get(registries).post(create_registry))
        .route(
            "/registries/{id}",
            axum::routing::put(update_registry).delete(delete_registry),
        )
        .route("/registries/{id}/test", post(test_registry))
}
pub fn deploy_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/apps/{id}/config",
            get(deploy_config)
                .post(set_deploy_config)
                .delete(delete_deploy_config),
        )
        .route("/apps/{id}/deploy", post(trigger_deploy))
        .route("/apps/{id}/pull", post(trigger_pull))
        .route("/apps/{id}/git-status", get(git_status))
        .route("/apps/{id}/commit", get(commit_info))
        .route("/apps/{id}/branches", get(app_branches))
        .route("/history", get(deploy_history))
        .route("/clone", post(clone_repo))
        .route("/branches", post(branches_from_url))
        .route("/webhook-logs", get(webhook_logs))
}
pub fn builds_router() -> Router<SharedState> {
    Router::new()
        .route(
            "/apps/{id}/build-config",
            get(build_config)
                .post(set_build_config)
                .delete(delete_build_config),
        )
        .route("/apps/{id}/detect", get(build_detect))
        .route("/apps/{id}/nixpacks-plan", get(nixpacks_plan))
        .route("/apps/{id}/build", post(trigger_build))
        .route("/apps/{id}/build-logs", get(build_logs))
        .route("/apps/{id}/build-logs/{ts}", get(build_log_detail))
        .route("/apps/{id}/clear-cache", post(ok))
        .route("/apps/{id}/deploy", post(trigger_build_deploy))
        .route("/apps/{id}/deployments", get(app_deployments))
        .route("/apps/{id}/rollback", post(trigger_rollback))
        .route("/apps/{id}/current-deployment", get(current_deployment))
        .route("/deployments/{id}", get(deployment_detail))
        .route("/deployments/{id}/diff", get(deployment_diff))
}
pub fn deployment_jobs_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(deployment_jobs))
        .route("/{id}", get(deployment_job))
        .route("/{id}/logs", get(deployment_job_logs))
}
pub fn migrations_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(migration_status))
        .route("/backup", post(migration_backup))
        .route("/apply", post(migration_apply))
        .route("/history", get(migration_history))
}

async fn bp_detect(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::detect_plan(&b)))
}
async fn bp_generate(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::generate_buildpack(
        &b["plan"],
        b.get("overrides"),
        b.get("name").and_then(Value::as_str),
    )))
}
async fn source_status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::source_status(&s.db, &provider).await?))
}
async fn source_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::source_config(&s.db, &provider).await?))
}
async fn put_source_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::put_source_config(&s.db, &provider, &b).await?,
    ))
}
async fn authorize(AuthUser(u): AuthUser, Path(provider): Path<String>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::authorize(&provider).await))
}
async fn callback(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::callback(&s.db, &provider, &b).await?))
}
async fn disconnect(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::disconnect(&s.db, &provider).await?))
}
async fn repos(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(provider): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::repos(&s.db, &provider).await?))
}
async fn branches(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::branches()))
}
async fn manifest(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::manifest()))
}
async fn connections(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::connections(&s.db).await?))
}
async fn registries(State(s): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::registries(&s.db).await?))
}
async fn create_registry(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::create_registry(&s.db, &b).await?))
}
async fn update_registry(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::update_registry(&s.db, &id, &b)
            .await?
            .ok_or_else(|| nf("registry not found"))?,
    ))
}
async fn delete_registry(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::delete_registry(&s.db, &id).await?))
}
async fn test_registry(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::test_registry()))
}
async fn deploy_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::deploy_config(&s.db, &id).await?))
}
async fn set_deploy_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::set_deploy_config(&s.db, &id, &b).await?))
}
async fn delete_deploy_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::delete_deploy_config(&s.db, &id).await?))
}
async fn trigger_deploy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(&s.db, &id, "git-deploy", &b).await?,
    ))
}
async fn trigger_pull(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(&s.db, &id, "git-pull", &b).await?,
    ))
}
async fn git_status(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::git_status()))
}
async fn commit_info(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::commit_info()))
}
async fn app_branches(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::branches()))
}
async fn deploy_history(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Query(q): Query<HashMap<String, String>>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::deployments(&s.db, q.get("app_id").map(String::as_str)).await?,
    ))
}
async fn clone_repo(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(
            &s.db,
            b.get("app_id")
                .and_then(Value::as_str)
                .unwrap_or("external"),
            "clone",
            &b,
        )
        .await?,
    ))
}
async fn branches_from_url(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::branches_from_url()))
}
async fn webhook_logs(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(json!({"logs":[]})))
}
async fn build_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::build_config(&s.db, &id).await?))
}
async fn set_build_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::set_build_config(&s.db, &id, &b).await?))
}
async fn delete_build_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::delete_build_config(&s.db, &id).await?))
}
async fn build_detect(AuthUser(u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::detect_plan(&json!({"name":id}))))
}
async fn nixpacks_plan(AuthUser(u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::detect_plan(
        &json!({"name":id,"builder":"nixpacks"}),
    )))
}
async fn trigger_build(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(&s.db, &id, "build", &b).await?,
    ))
}
async fn build_logs(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::build_logs()))
}
async fn build_log_detail(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(json!({"log":""})))
}
async fn trigger_build_deploy(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(&s.db, &id, "deploy", &b).await?,
    ))
}
async fn app_deployments(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::deployments(&s.db, Some(&id)).await?))
}
async fn trigger_rollback(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::create_deployment(&s.db, &id, "rollback", &b).await?,
    ))
}
async fn current_deployment(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    let d = sk_deploy::deployments(&s.db, Some(&id)).await?;
    Ok(Json(
        json!({"deployment":d["deployments"].as_array().and_then(|a|a.first()).cloned()}),
    ))
}
async fn deployment_detail(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::deployment_detail(&s.db, &id)
            .await?
            .ok_or_else(|| nf("deployment not found"))?,
    ))
}
async fn deployment_diff(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::diff()))
}
async fn deployment_jobs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::deployment_jobs(&s.db).await?))
}
async fn deployment_job(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(
        sk_deploy::deployment_detail(&s.db, &id)
            .await?
            .ok_or_else(|| nf("deployment job not found"))?,
    ))
}
async fn deployment_job_logs(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::deployment_logs(&s.db, &id).await?))
}
async fn migration_status(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::migration_status(&s.db).await?))
}
async fn migration_backup(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::migration_record(&s.db, "backup").await?))
}
async fn migration_apply(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::migration_record(&s.db, "apply").await?))
}
async fn migration_history(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(sk_deploy::migration_history(&s.db).await?))
}
async fn ok(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    admin(&u)?;
    Ok(Json(json!({"success":true})))
}
