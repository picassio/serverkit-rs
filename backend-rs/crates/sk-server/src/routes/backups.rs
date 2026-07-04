use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::routing::{delete, get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/", get(list).delete(delete_backup))
        .route("/stats", get(stats))
        .route("/config", get(config).put(set_config))
        .route("/cost-rates", get(rates).put(set_rates))
        .route("/cost-summary", get(cost_summary))
        .route("/application", post(backup_application))
        .route("/database", post(backup_database))
        .route("/files", post(backup_files))
        .route("/restore/application", post(restore_application))
        .route("/restore/database", post(restore_database))
        .route("/cleanup", post(cleanup))
        .route("/schedules", get(schedules).post(add_schedule))
        .route(
            "/schedules/{id}",
            put(update_schedule).delete(delete_schedule),
        )
        .route("/storage", get(storage).put(set_storage))
        .route("/storage/test", post(test_storage))
        .route("/upload", post(upload))
        .route("/verify", post(verify))
        .route("/remote", get(remote_list))
        .route("/remote/download", post(remote_download))
        .route("/{id}", delete(delete_backup_by_id))
}

#[derive(Deserialize)]
struct ListQuery {
    #[serde(rename = "type")]
    kind: Option<String>,
}
async fn list(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::list(&s.db, q.kind.as_deref()).await?))
}
async fn stats(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::stats(&s.db).await?))
}
async fn config(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::config(&s.db).await?))
}
async fn set_config(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::set_config(&s.db, &b).await?))
}
async fn rates(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::rates(&s.db).await?))
}
async fn set_rates(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::set_rates(&s.db, &b).await?))
}
async fn cost_summary(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::cost_summary(&s.db).await?))
}
async fn storage(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::storage(&s.db).await?))
}
async fn set_storage(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::set_storage(&s.db, &b).await?))
}
async fn test_storage(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::test_storage(&s.db, Some(&b)).await?))
}
async fn backup_application(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::backup_application(&s.db, &b).await?))
}
async fn backup_database(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::backup_database(&s.db, &b).await?))
}
async fn backup_files(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::backup_files(&s.db, &b).await?))
}
async fn restore_application(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::restore_application(&s.db, &b).await?))
}
async fn restore_database(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::restore_database(&s.db, &b).await?))
}
async fn cleanup(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::cleanup(&s.db, &b).await?))
}
async fn schedules(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_backups::schedules(&s.db).await?))
}
async fn add_schedule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::add_schedule(&s.db, &b).await?))
}
async fn update_schedule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::update_schedule(&s.db, &id, &b).await?))
}
async fn delete_schedule(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::delete_schedule(&s.db, &id).await?))
}
async fn upload(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::upload(&s.db, &b).await?))
}
async fn verify(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::verify(&s.db, &b).await?))
}
#[derive(Deserialize)]
struct RemoteQuery {
    prefix: Option<String>,
}
async fn remote_list(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<RemoteQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_backups::remote_list(&s.db, q.prefix.as_deref()).await?,
    ))
}
async fn remote_download(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::remote_download(&s.db, &b).await?))
}
async fn delete_backup(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let key = b
        .get("backup_path")
        .or_else(|| b.get("path"))
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("backup_path is required"))?;
    Ok(Json(sk_backups::delete(&s.db, key).await?))
}
async fn delete_backup_by_id(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_backups::delete(&s.db, &id).await?))
}
