use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}
fn body(b: Option<Json<Value>>) -> Value {
    b.map(|x| x.0).unwrap_or_else(|| json!({}))
}
#[derive(Deserialize)]
struct LimitQ {
    limit: Option<i64>,
}
#[derive(Deserialize)]
struct CommitsQ {
    branch: Option<String>,
    page: Option<i64>,
    limit: Option<i64>,
}
#[derive(Deserialize)]
struct RefQ {
    r#ref: Option<String>,
}
#[derive(Deserialize)]
struct LogsQ {
    logs: Option<bool>,
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/requirements", get(requirements))
        .route("/install", post(install))
        .route("/uninstall", post(uninstall))
        .route("/start", post(start))
        .route("/stop", post(stop))
        .route("/restart", post(restart))
        .route("/version", get(version))
        .route("/webhooks", get(webhooks).post(create_webhook))
        .route(
            "/webhooks/{id}",
            get(webhook).put(update_webhook).delete(delete_webhook),
        )
        .route("/webhooks/{id}/toggle", post(toggle_webhook))
        .route("/webhooks/{id}/test", post(test_webhook))
        .route("/webhooks/{id}/logs", get(webhook_logs))
        .route("/repos", get(repos))
        .route("/repos/{owner}/{repo}", get(repo))
        .route("/repos/{owner}/{repo}/stats", get(stats))
        .route("/repos/{owner}/{repo}/branches", get(branches))
        .route("/repos/{owner}/{repo}/branches/{branch}", get(branch))
        .route("/repos/{owner}/{repo}/commits", get(commits))
        .route("/repos/{owner}/{repo}/commits/{sha}", get(commit))
        .route("/repos/{owner}/{repo}/contents", get(contents_root))
        .route("/repos/{owner}/{repo}/contents/{*path}", get(contents_path))
        .route("/repos/{owner}/{repo}/readme", get(readme))
        .route("/deployments/app/{id}", get(app_deployments))
        .route("/deployments/app/{id}/deploy", post(trigger_deploy))
        .route("/deployments/app/{id}/rollback", post(rollback))
        .route("/deployments/webhook/{id}", get(webhook_deployments))
        .route("/deployments/{id}", get(deployment))
}

async fn status(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::status(&s.db).await.map_err(internal)?))
}
async fn requirements(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::requirements().await.map_err(internal)?))
}
async fn install(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::install(&s.db, &body(b)).await.map_err(internal)?,
    ))
}
async fn uninstall(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::uninstall(&s.db, &body(b)).await.map_err(internal)?,
    ))
}
async fn start(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::start(&s.db).await.map_err(internal)?))
}
async fn stop(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::stop(&s.db).await.map_err(internal)?))
}
async fn restart(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::restart(&s.db).await.map_err(internal)?))
}
async fn version(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::version(&s.db).await.map_err(internal)?))
}
async fn webhooks(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::webhooks(&s.db).await.map_err(internal)?))
}
async fn webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_git::webhook(&s.db, &id).await.map_err(internal)?))
}
async fn create_webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::create_webhook(&s.db, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn update_webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::update_webhook(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn delete_webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::delete_webhook(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn toggle_webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::toggle_webhook(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn test_webhook(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::test_webhook(&s.db, &id).await.map_err(internal)?,
    ))
}
async fn webhook_logs(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<LimitQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::webhook_logs(&s.db, &id, q.limit.unwrap_or(50))
            .await
            .map_err(internal)?,
    ))
}
async fn repos(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Query(q): Query<LimitQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::repos(&s.db, q.limit.unwrap_or(50))
            .await
            .map_err(internal)?,
    ))
}
async fn repo(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::repo(&s.db, &owner, &repo).await.map_err(internal)?,
    ))
}
async fn stats(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::stats(&s.db, &owner, &repo)
            .await
            .map_err(internal)?,
    ))
}
async fn branches(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::branches(&s.db, &owner, &repo)
            .await
            .map_err(internal)?,
    ))
}
async fn branch(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo, branch)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::branch(&s.db, &owner, &repo, &branch)
            .await
            .map_err(internal)?,
    ))
}
async fn commits(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
    Query(q): Query<CommitsQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::commits(
            &s.db,
            &owner,
            &repo,
            q.branch.as_deref(),
            q.page.unwrap_or(1),
            q.limit.unwrap_or(30),
        )
        .await
        .map_err(internal)?,
    ))
}
async fn commit(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo, sha)): Path<(String, String, String)>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::commit(&s.db, &owner, &repo, &sha)
            .await
            .map_err(internal)?,
    ))
}
async fn contents_root(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
    Query(q): Query<RefQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::contents(&s.db, &owner, &repo, "", q.r#ref.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn contents_path(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo, path)): Path<(String, String, String)>,
    Query(q): Query<RefQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::contents(&s.db, &owner, &repo, &path, q.r#ref.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn readme(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path((owner, repo)): Path<(String, String)>,
    Query(q): Query<RefQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::readme(&s.db, &owner, &repo, q.r#ref.as_deref())
            .await
            .map_err(internal)?,
    ))
}
async fn app_deployments(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<LimitQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::app_deployments(&s.db, &id, q.limit.unwrap_or(20))
            .await
            .map_err(internal)?,
    ))
}
async fn webhook_deployments(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<LimitQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::webhook_deployments(&s.db, &id, q.limit.unwrap_or(20))
            .await
            .map_err(internal)?,
    ))
}
async fn deployment(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<LogsQ>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::deployment(&s.db, &id, q.logs.unwrap_or(false))
            .await
            .map_err(internal)?,
    ))
}
async fn trigger_deploy(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::trigger_deploy(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
async fn rollback(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
    Path(id): Path<String>,
    b: Option<Json<Value>>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_git::rollback(&s.db, &id, &body(b))
            .await
            .map_err(internal)?,
    ))
}
