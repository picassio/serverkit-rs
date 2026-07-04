//! Port of the MySQL half of `app/api/databases.py` + `app/api/cron.py`.
//! Root passwords arrive via the `X-DB-Password` header (never query params).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn databases_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(db_status))
        .route("/backups", get(list_backups))
        .route("/backups/{filename}", delete(delete_backup))
        .route("/docker", get(docker_containers))
        .route("/docker/{container}/databases", get(docker_databases))
        .route("/docker/{container}/{database}/tables", get(docker_tables))
        .route("/docker/{container}/{database}/query", post(docker_query))
        .route("/mysql", get(list_mysql).post(create_mysql))
        .route("/mysql/users", get(mysql_users).post(create_mysql_user))
        .route("/mysql/users/{username}", delete(drop_mysql_user))
        .route(
            "/mysql/users/{username}/privileges",
            get(mysql_user_privileges),
        )
        .route("/mysql/users/{username}/grant", post(mysql_grant))
        .route("/mysql/users/{username}/revoke", post(mysql_revoke))
        .route("/mysql/{name}", delete(drop_mysql))
        .route("/mysql/{name}/tables", get(mysql_tables))
        .route("/mysql/{name}/backup", post(backup_mysql))
        .route("/mysql/{name}/restore", post(restore_mysql))
        .route("/mysql/{name}/query", post(mysql_query))
}

pub fn cron_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(cron_status))
        .route("/presets", get(cron_presets))
        .route("/jobs", get(cron_jobs).post(cron_create))
        .route(
            "/jobs/{id}",
            axum::routing::put(cron_update).delete(cron_delete),
        )
        .route("/jobs/{id}/toggle", post(cron_toggle))
        .route("/jobs/{id}/run", post(cron_run))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

fn db_password(headers: &HeaderMap) -> Option<String> {
    headers
        .get("X-DB-Password")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
}

fn by_success(result: Value, ok: StatusCode) -> (StatusCode, Json<Value>) {
    let success = result
        .get("success")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    (
        if success { ok } else { StatusCode::BAD_REQUEST },
        Json(result),
    )
}

// ==================== DATABASES ====================

async fn db_status(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_db::status().await)
}

async fn list_mysql(AuthUser(_u): AuthUser, headers: HeaderMap) -> Json<Value> {
    let pw = db_password(&headers);
    Json(json!({ "databases": sk_db::list_databases(pw.as_deref()).await }))
}

#[derive(Deserialize)]
struct CreateDbBody {
    name: Option<String>,
    charset: Option<String>,
    collation: Option<String>,
    root_password: Option<String>,
    create_user: Option<bool>,
    user_password: Option<String>,
    host: Option<String>,
}

async fn create_mysql(
    AuthUser(u): AuthUser,
    Json(b): Json<CreateDbBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let name = b
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    let charset = b.charset.unwrap_or_else(|| "utf8mb4".into());
    let collation = b.collation.unwrap_or_else(|| "utf8mb4_unicode_ci".into());
    let host = b.host.unwrap_or_else(|| "localhost".into());
    let root_pw = b.root_password.as_deref();

    let mut result = sk_db::create_database(&name, &charset, &collation, root_pw).await;

    if result["success"].as_bool().unwrap_or(false) && b.create_user.unwrap_or(false) {
        let password = b
            .user_password
            .unwrap_or_else(|| sk_db::generate_password(16));
        sk_db::create_user(&name, &password, &host, root_pw).await;
        sk_db::grant(&name, &name, "ALL", &host, root_pw).await;
        result["user"] = json!(name);
        result["password"] = json!(password);
    }
    // TODO(P3): _persist_provisioned — provisioned_databases table write

    Ok(by_success(result, StatusCode::CREATED))
}

#[derive(Deserialize, Default)]
struct RootPwBody {
    root_password: Option<String>,
}

async fn drop_mysql(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    body: Option<Json<RootPwBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let pw = body.and_then(|b| b.0.root_password);
    Ok(by_success(
        sk_db::drop_database(&name, pw.as_deref()).await,
        StatusCode::OK,
    ))
}

async fn mysql_tables(
    AuthUser(_u): AuthUser,
    Path(name): Path<String>,
    headers: HeaderMap,
) -> Json<Value> {
    let pw = db_password(&headers);
    Json(json!({ "tables": sk_db::tables(&name, pw.as_deref()).await }))
}

async fn backup_mysql(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    body: Option<Json<RootPwBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let pw = body.and_then(|b| b.0.root_password);
    Ok(by_success(
        sk_db::backup(&name, pw.as_deref()).await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct RestoreBody {
    backup_path: Option<String>,
    root_password: Option<String>,
}

async fn restore_mysql(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Json(b): Json<RestoreBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let path = b
        .backup_path
        .ok_or_else(|| ApiError::bad_request("backup_path is required"))?;
    Ok(by_success(
        sk_db::restore(&name, &path, b.root_password.as_deref()).await,
        StatusCode::OK,
    ))
}

async fn mysql_users(AuthUser(_u): AuthUser, headers: HeaderMap) -> Json<Value> {
    let pw = db_password(&headers);
    Json(json!({ "users": sk_db::list_users(pw.as_deref()).await }))
}

#[derive(Deserialize)]
struct CreateUserBody {
    username: Option<String>,
    password: Option<String>,
    host: Option<String>,
    root_password: Option<String>,
}

async fn create_mysql_user(
    AuthUser(u): AuthUser,
    Json(b): Json<CreateUserBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let (Some(username), Some(password)) = (b.username, b.password) else {
        return Err(ApiError::bad_request("username and password are required"));
    };
    let host = b.host.unwrap_or_else(|| "localhost".into());
    Ok(by_success(
        sk_db::create_user(&username, &password, &host, b.root_password.as_deref()).await,
        StatusCode::CREATED,
    ))
}

#[derive(Deserialize, Default)]
struct HostBody {
    host: Option<String>,
    root_password: Option<String>,
}

async fn drop_mysql_user(
    AuthUser(u): AuthUser,
    Path(username): Path<String>,
    body: Option<Json<HostBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let b = body.map(|b| b.0).unwrap_or_default();
    let host = b.host.unwrap_or_else(|| "localhost".into());
    Ok(by_success(
        sk_db::drop_user(&username, &host, b.root_password.as_deref()).await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct HostQuery {
    host: Option<String>,
}

async fn mysql_user_privileges(
    AuthUser(_u): AuthUser,
    Path(username): Path<String>,
    Query(q): Query<HostQuery>,
    headers: HeaderMap,
) -> Json<Value> {
    let pw = db_password(&headers);
    let host = q.host.unwrap_or_else(|| "localhost".into());
    Json(json!({
        "privileges": sk_db::user_privileges(&username, &host, pw.as_deref()).await
    }))
}

#[derive(Deserialize)]
struct GrantBody {
    database: Option<String>,
    privileges: Option<String>,
    host: Option<String>,
    root_password: Option<String>,
}

async fn mysql_grant(
    AuthUser(u): AuthUser,
    Path(username): Path<String>,
    Json(b): Json<GrantBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let database = b
        .database
        .ok_or_else(|| ApiError::bad_request("database is required"))?;
    Ok(by_success(
        sk_db::grant(
            &username,
            &database,
            b.privileges.as_deref().unwrap_or("ALL"),
            b.host.as_deref().unwrap_or("localhost"),
            b.root_password.as_deref(),
        )
        .await,
        StatusCode::OK,
    ))
}

async fn mysql_revoke(
    AuthUser(u): AuthUser,
    Path(username): Path<String>,
    Json(b): Json<GrantBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let database = b
        .database
        .ok_or_else(|| ApiError::bad_request("database is required"))?;
    Ok(by_success(
        sk_db::revoke(
            &username,
            &database,
            b.privileges.as_deref().unwrap_or("ALL"),
            b.host.as_deref().unwrap_or("localhost"),
            b.root_password.as_deref(),
        )
        .await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct QueryBody {
    query: Option<String>,
    readonly: Option<bool>,
}

/// SQL console — readonly by default; write mode is admin-only.
async fn mysql_query(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    headers: HeaderMap,
    Json(b): Json<QueryBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let query = b
        .query
        .ok_or_else(|| ApiError::bad_request("query is required"))?;
    let readonly = b.readonly.unwrap_or(true);
    if !readonly && !u.is_admin() {
        return Err(ApiError::forbidden(
            "Admin access required to execute write queries",
        ));
    }
    let pw = db_password(&headers);
    let result = sk_db::execute_query(&name, &query, readonly, pw.as_deref(), 30, 1000).await;
    Ok(by_success(result, StatusCode::OK))
}

// ==================== DOCKER DATABASES ====================

async fn docker_containers(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "containers": sk_db::docker::list_containers().await }))
}

#[derive(Deserialize)]
struct DockerUserQuery {
    user: Option<String>,
}

async fn docker_databases(
    AuthUser(_u): AuthUser,
    Path(container): Path<String>,
    Query(q): Query<DockerUserQuery>,
    headers: HeaderMap,
) -> Json<Value> {
    let pw = db_password(&headers);
    let user = q.user.unwrap_or_else(|| "root".into());
    Json(json!({
        "databases": sk_db::docker::list_databases(&container, &user, pw.as_deref()).await
    }))
}

async fn docker_tables(
    AuthUser(_u): AuthUser,
    Path((container, database)): Path<(String, String)>,
    Query(q): Query<DockerUserQuery>,
    headers: HeaderMap,
) -> Json<Value> {
    let pw = db_password(&headers);
    let user = q.user.unwrap_or_else(|| "root".into());
    let result = sk_db::docker::tables(&container, &database, &user, pw.as_deref()).await;
    Json(json!({
        "tables": result["tables"],
        "connected": result["connected"],
        "error": result["error"],
    }))
}

#[derive(Deserialize)]
struct DockerQueryBody {
    query: Option<String>,
    readonly: Option<bool>,
    user: Option<String>,
    password: Option<String>,
}

async fn docker_query(
    AuthUser(u): AuthUser,
    Path((container, database)): Path<(String, String)>,
    headers: HeaderMap,
    Json(b): Json<DockerQueryBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let query = b
        .query
        .ok_or_else(|| ApiError::bad_request("query is required"))?;
    let readonly = b.readonly.unwrap_or(true);
    if !readonly && !u.is_admin() {
        return Err(ApiError::forbidden(
            "Admin access required to execute write queries",
        ));
    }
    let password = db_password(&headers).or(b.password);
    let user = b.user.unwrap_or_else(|| "root".into());
    let result = sk_db::docker::execute_query(
        &container,
        &database,
        &query,
        &user,
        password.as_deref(),
        readonly,
        30,
        1000,
    )
    .await;
    Ok(by_success(result, StatusCode::OK))
}

#[derive(Deserialize)]
struct BackupsQuery {
    #[serde(rename = "type")]
    db_type: Option<String>,
}

async fn list_backups(
    AuthUser(_u): AuthUser,
    Query(q): Query<BackupsQuery>,
) -> ApiResult<Json<Value>> {
    let backups = tokio::task::spawn_blocking(move || sk_db::list_backups(q.db_type.as_deref()))
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "backups": backups })))
}

async fn delete_backup(
    AuthUser(u): AuthUser,
    Path(filename): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(sk_db::delete_backup(&filename), StatusCode::OK))
}

// ==================== CRON ====================

async fn cron_status(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_ops::cron::status().await)
}

async fn cron_presets(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_ops::cron::presets())
}

async fn cron_jobs(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_ops::cron::list_jobs().await)
}

#[derive(Deserialize)]
struct CronBody {
    schedule: Option<String>,
    command: Option<String>,
    name: Option<String>,
    description: Option<String>,
}

async fn cron_create(
    AuthUser(u): AuthUser,
    Json(b): Json<CronBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let schedule = b
        .schedule
        .ok_or_else(|| ApiError::bad_request("Schedule is required"))?;
    let command = b
        .command
        .ok_or_else(|| ApiError::bad_request("Command is required"))?;
    Ok(by_success(
        sk_ops::cron::add_job(
            &schedule,
            &command,
            b.name.as_deref(),
            b.description.as_deref(),
        )
        .await,
        StatusCode::CREATED,
    ))
}

async fn cron_update(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<CronBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_ops::cron::update_job(
            &id,
            b.name.as_deref(),
            b.command.as_deref(),
            b.schedule.as_deref(),
            b.description.as_deref(),
        )
        .await,
        StatusCode::OK,
    ))
}

async fn cron_delete(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_ops::cron::remove_job(&id).await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize, Default)]
struct ToggleBody {
    enabled: Option<bool>,
}

async fn cron_toggle(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    body: Option<Json<ToggleBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let enabled = body.and_then(|b| b.0.enabled).unwrap_or(true);
    Ok(by_success(
        sk_ops::cron::toggle_job(&id, enabled).await,
        StatusCode::OK,
    ))
}

async fn cron_run(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_ops::cron::run_job_now(&id).await,
        StatusCode::OK,
    ))
}
