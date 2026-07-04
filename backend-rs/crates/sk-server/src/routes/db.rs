//! Port of the MySQL half of `app/api/databases.py` + `app/api/cron.py`.
//! Root passwords arrive via the `X-DB-Password` header (never query params).

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Column, Row, ValueRef};

pub fn databases_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(db_status))
        .route("/generate-password", get(generate_password))
        .route("/backups", get(list_backups))
        .route("/backups/{filename}", delete(delete_backup))
        .route("/docker", get(docker_containers))
        .route("/docker/databases", get(docker_all_databases))
        .route("/docker/app/{app_id}", get(docker_app_databases))
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
        .route(
            "/mysql/{name}/tables/{table}/structure",
            get(mysql_table_structure),
        )
        .route("/mysql/{name}/backup", post(backup_mysql))
        .route("/mysql/{name}/restore", post(restore_mysql))
        .route("/mysql/{name}/query", post(mysql_query))
        .route("/postgresql", get(list_postgres).post(create_postgres))
        .route(
            "/postgresql/users",
            get(postgres_users).post(create_postgres_user),
        )
        .route("/postgresql/users/{username}", delete(drop_postgres_user))
        .route("/postgresql/users/{username}/grant", post(postgres_grant))
        .route("/postgresql/{name}", delete(drop_postgres))
        .route("/postgresql/{name}/tables", get(postgres_tables))
        .route(
            "/postgresql/{name}/tables/{table}/structure",
            get(postgres_table_structure),
        )
        .route("/postgresql/{name}/backup", post(backup_postgres))
        .route("/postgresql/{name}/restore", post(restore_postgres))
        .route("/postgresql/{name}/query", post(postgres_query))
        .route("/sqlite", get(sqlite_databases))
        .route("/sqlite/tables", get(sqlite_tables))
        .route(
            "/sqlite/tables/{table}/structure",
            get(sqlite_table_structure),
        )
        .route("/sqlite/query", post(sqlite_query))
        .route("/managed", get(managed_list).post(managed_create))
        .route("/managed/adopt", post(managed_adopt))
        .route("/managed/{id}", get(managed_get).delete(managed_delete))
        .route("/managed/{id}/connection-uri", post(managed_connection_uri))
        .route("/managed/{id}/protect", post(managed_protect))
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

#[derive(Deserialize)]
struct PasswordQuery {
    length: Option<usize>,
}

async fn generate_password(AuthUser(_u): AuthUser, Query(q): Query<PasswordQuery>) -> Json<Value> {
    let length = q.length.unwrap_or(16).clamp(8, 128);
    Json(json!({ "password": sk_db::generate_password(length), "length": length }))
}

async fn mysql_table_structure(
    AuthUser(_u): AuthUser,
    Path((database, table)): Path<(String, String)>,
    headers: HeaderMap,
) -> Json<Value> {
    if !sk_db::validate_identifier(&table, 64) {
        return Json(json!({ "success": false, "error": "Invalid table identifier" }));
    }
    let pw = db_password(&headers);
    let result = sk_db::execute_query(
        &database,
        &format!("DESCRIBE `{table}`"),
        true,
        pw.as_deref(),
        30,
        1000,
    )
    .await;
    Json(
        json!({ "success": result["success"], "columns": result["columns"], "rows": result["rows"], "structure": result["rows"], "error": result["error"] }),
    )
}

async fn docker_all_databases(AuthUser(_u): AuthUser) -> Json<Value> {
    let containers = sk_db::docker::list_containers().await;
    let mut databases = Vec::new();
    for c in &containers {
        if let Some(container) = c.get("name").and_then(Value::as_str) {
            for mut db in sk_db::docker::list_databases(container, "root", None).await {
                db["container"] = json!(container);
                db["container_id"] = c.get("id").cloned().unwrap_or(Value::Null);
                db["engine"] = json!("mysql");
                databases.push(db);
            }
        }
    }
    Json(json!({ "containers": containers, "databases": databases }))
}

async fn docker_app_databases(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(app_id): Path<String>,
) -> Json<Value> {
    let app = sk_apps::get(&s.db, &app_id).await.ok().flatten();
    let app_name = app
        .as_ref()
        .and_then(|a| a.get("name"))
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_lowercase();
    let containers = sk_db::docker::list_containers().await;
    let matched: Vec<Value> = containers
        .into_iter()
        .filter(|c| {
            let name = c
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("")
                .to_lowercase();
            !app_name.is_empty() && name.contains(&app_name.replace(' ', "-"))
                || name.contains(&app_id.to_lowercase())
        })
        .collect();
    let mut databases = Vec::new();
    for c in &matched {
        if let Some(container) = c.get("name").and_then(Value::as_str) {
            for mut db in sk_db::docker::list_databases(container, "root", None).await {
                db["container"] = json!(container);
                db["app_id"] = json!(app_id);
                databases.push(db);
            }
        }
    }
    Json(json!({ "app": app, "containers": matched, "databases": databases }))
}

async fn pg_cmd(args: &[&str], stdin: Option<&str>, timeout: u64) -> Value {
    let mut cmd = tokio::process::Command::new(args[0]);
    cmd.args(&args[1..]);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    if stdin.is_some() {
        cmd.stdin(std::process::Stdio::piped());
    }
    let run = async {
        let mut child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return json!({ "success": false, "error": e.to_string() }),
        };
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            use tokio::io::AsyncWriteExt;
            let _ = pipe.write_all(input.as_bytes()).await;
            drop(pipe);
        }
        match child.wait_with_output().await {
            Ok(o) => json!({
                "success": o.status.success(),
                "output": String::from_utf8_lossy(&o.stdout),
                "error": if o.status.success() { Value::Null } else { json!(String::from_utf8_lossy(&o.stderr).trim().to_string()) },
            }),
            Err(e) => json!({ "success": false, "error": e.to_string() }),
        }
    };
    match tokio::time::timeout(std::time::Duration::from_secs(timeout), run).await {
        Ok(v) => v,
        Err(_) => json!({ "success": false, "error": "Command timed out" }),
    }
}

fn parse_tabular(output: &str) -> Value {
    let lines: Vec<&str> = output.trim().lines().collect();
    if lines.is_empty() {
        return json!({ "columns": [], "rows": [], "row_count": 0 });
    }
    let columns: Vec<String> = lines[0].split('\t').map(str::to_string).collect();
    let rows: Vec<Vec<Value>> = lines[1..]
        .iter()
        .map(|l| {
            l.split('\t')
                .map(|v| if v.is_empty() { Value::Null } else { json!(v) })
                .collect()
        })
        .collect();
    json!({ "columns": columns, "row_count": rows.len(), "rows": rows })
}

async fn psql_query(database: Option<&str>, query: &str, readonly: bool) -> Value {
    if readonly && !sk_db::is_readonly_query(query) {
        return json!({ "success": false, "error": "Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed in readonly mode" });
    }
    let mut args = vec!["psql", "-U", "postgres", "-A", "-F", "\t"];
    if let Some(db) = database {
        args.extend(["-d", db]);
    }
    args.extend(["-c", query]);
    let start = std::time::Instant::now();
    let out = pg_cmd(&args, None, 30).await;
    if !out.get("success").and_then(Value::as_bool).unwrap_or(false) {
        return out;
    }
    let parsed = parse_tabular(out.get("output").and_then(Value::as_str).unwrap_or(""));
    json!({ "success": true, "columns": parsed["columns"], "rows": parsed["rows"], "row_count": parsed["row_count"], "execution_time": (start.elapsed().as_secs_f64() * 10000.0).round() / 10000.0, "truncated": false })
}

async fn list_postgres(AuthUser(_u): AuthUser) -> Json<Value> {
    let q = "SELECT datname AS name FROM pg_database WHERE datistemplate = false AND datname NOT IN ('postgres') ORDER BY datname";
    let result = psql_query(None, q, true).await;
    let databases: Vec<Value> = result
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            r.as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|name| json!({ "name": name, "type": "postgresql" }))
        })
        .collect();
    Json(
        json!({ "databases": databases, "connected": result["success"], "error": result["error"] }),
    )
}

#[derive(Deserialize)]
struct PgCreateBody {
    name: Option<String>,
}
async fn create_postgres(
    AuthUser(u): AuthUser,
    Json(b): Json<PgCreateBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let name = b
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    if !sk_db::validate_identifier(&name, 64) {
        return Err(ApiError::bad_request("invalid database name"));
    }
    let out = pg_cmd(&["createdb", "-U", "postgres", &name], None, 60).await;
    Ok(by_success(out, StatusCode::CREATED))
}
async fn drop_postgres(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    if !sk_db::validate_identifier(&name, 64) {
        return Err(ApiError::bad_request("invalid database name"));
    }
    Ok(by_success(
        pg_cmd(
            &["dropdb", "-U", "postgres", "--if-exists", &name],
            None,
            60,
        )
        .await,
        StatusCode::OK,
    ))
}
async fn postgres_tables(AuthUser(_u): AuthUser, Path(name): Path<String>) -> Json<Value> {
    let q = "SELECT tablename AS name FROM pg_tables WHERE schemaname='public' ORDER BY tablename";
    let result = psql_query(Some(&name), q, true).await;
    let tables: Vec<Value> = result
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            r.as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|t| json!({"name":t}))
        })
        .collect();
    Json(json!({"tables":tables,"connected":result["success"],"error":result["error"]}))
}
async fn postgres_table_structure(
    AuthUser(_u): AuthUser,
    Path((name, table)): Path<(String, String)>,
) -> Json<Value> {
    if !sk_db::validate_identifier(&table, 64) {
        return Json(json!({"success":false,"error":"invalid table"}));
    }
    let q = format!("SELECT column_name,data_type,is_nullable,column_default FROM information_schema.columns WHERE table_schema='public' AND table_name='{table}' ORDER BY ordinal_position");
    Json(psql_query(Some(&name), &q, true).await)
}
#[derive(Deserialize)]
struct PgUserBody {
    username: Option<String>,
    password: Option<String>,
}
async fn postgres_users(AuthUser(_u): AuthUser) -> Json<Value> {
    let result = psql_query(
        None,
        "SELECT usename AS username FROM pg_user ORDER BY usename",
        true,
    )
    .await;
    let users: Vec<Value> = result
        .get("rows")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|r| {
            r.as_array()
                .and_then(|a| a.first())
                .and_then(Value::as_str)
                .map(|u| json!({"user":u}))
        })
        .collect();
    Json(json!({"users":users,"connected":result["success"],"error":result["error"]}))
}
async fn create_postgres_user(
    AuthUser(u): AuthUser,
    Json(b): Json<PgUserBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let username = b
        .username
        .ok_or_else(|| ApiError::bad_request("username is required"))?;
    let password = b.password.unwrap_or_else(|| sk_db::generate_password(16));
    if !sk_db::validate_identifier(&username, 64) {
        return Err(ApiError::bad_request("invalid username"));
    }
    let q = format!(
        "CREATE USER {username} WITH PASSWORD '{}'",
        password.replace('\'', "''")
    );
    Ok(by_success(
        psql_query(None, &q, false).await,
        StatusCode::CREATED,
    ))
}
async fn drop_postgres_user(
    AuthUser(u): AuthUser,
    Path(username): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    if !sk_db::validate_identifier(&username, 64) {
        return Err(ApiError::bad_request("invalid username"));
    }
    Ok(by_success(
        psql_query(None, &format!("DROP USER IF EXISTS {username}"), false).await,
        StatusCode::OK,
    ))
}
#[derive(Deserialize)]
struct PgGrantBody {
    database: Option<String>,
    privileges: Option<String>,
}
async fn postgres_grant(
    AuthUser(u): AuthUser,
    Path(username): Path<String>,
    Json(b): Json<PgGrantBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let database = b
        .database
        .ok_or_else(|| ApiError::bad_request("database is required"))?;
    if !sk_db::validate_identifier(&username, 64) || !sk_db::validate_identifier(&database, 64) {
        return Err(ApiError::bad_request("invalid identifier"));
    }
    let privileges = b.privileges.unwrap_or_else(|| "ALL PRIVILEGES".into());
    let q = format!("GRANT {privileges} ON DATABASE {database} TO {username}");
    Ok(by_success(
        psql_query(None, &q, false).await,
        StatusCode::OK,
    ))
}
async fn backup_postgres(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    if !sk_db::validate_identifier(&name, 64) {
        return Err(ApiError::bad_request("invalid database name"));
    }
    let dir = std::env::var("SK_DB_BACKUP_DIR")
        .unwrap_or_else(|_| "/var/backups/serverkit/databases".into());
    let _ = std::fs::create_dir_all(&dir);
    let path = format!(
        "{dir}/postgresql_{name}_{}.sql",
        chrono::Local::now().format("%Y%m%d_%H%M%S")
    );
    let out = pg_cmd(
        &["pg_dump", "-U", "postgres", "-f", &path, &name],
        None,
        300,
    )
    .await;
    Ok(by_success(
        if out["success"].as_bool().unwrap_or(false) {
            json!({"success":true,"path":path})
        } else {
            out
        },
        StatusCode::OK,
    ))
}
async fn restore_postgres(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Json(b): Json<RestoreBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let path = b
        .backup_path
        .ok_or_else(|| ApiError::bad_request("backup_path is required"))?;
    if !sk_db::validate_identifier(&name, 64) {
        return Err(ApiError::bad_request("invalid database name"));
    }
    Ok(by_success(
        pg_cmd(
            &["psql", "-U", "postgres", "-d", &name, "-f", &path],
            None,
            300,
        )
        .await,
        StatusCode::OK,
    ))
}
async fn postgres_query(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
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
    Ok(by_success(
        psql_query(Some(&name), &query, readonly).await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct SqlitePathQuery {
    path: Option<String>,
}
fn sqlite_path(q: Option<String>) -> Result<String, ApiError> {
    let path = q.ok_or_else(|| ApiError::bad_request("path is required"))?;
    if !sk_files::is_path_allowed(&path) {
        return Err(ApiError::forbidden("Access denied"));
    }
    Ok(path)
}
async fn sqlite_pool(path: &str, readonly: bool) -> Result<sqlx::SqlitePool, ApiError> {
    let opts = SqliteConnectOptions::new()
        .filename(path)
        .read_only(readonly)
        .create_if_missing(false);
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(opts)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))
}
fn sqlite_value(row: &sqlx::sqlite::SqliteRow, idx: usize) -> Value {
    let raw = row.try_get_raw(idx);
    if raw.as_ref().map(|v| v.is_null()).unwrap_or(false) {
        return Value::Null;
    }
    row.try_get::<String, _>(idx)
        .map(Value::from)
        .or_else(|_| row.try_get::<i64, _>(idx).map(Value::from))
        .or_else(|_| row.try_get::<f64, _>(idx).map(Value::from))
        .unwrap_or(Value::Null)
}
async fn sqlite_databases(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({"databases":[]}))
}
async fn sqlite_tables(
    AuthUser(_u): AuthUser,
    Query(q): Query<SqlitePathQuery>,
) -> ApiResult<Json<Value>> {
    let path = sqlite_path(q.path)?;
    let pool = sqlite_pool(&path, true).await?;
    let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").fetch_all(&pool).await.map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(
        json!({"tables":rows.iter().map(|r|json!({"name":r.get::<String,_>("name")})).collect::<Vec<_>>() }),
    ))
}
async fn sqlite_table_structure(
    AuthUser(_u): AuthUser,
    Path(table): Path<String>,
    Query(q): Query<SqlitePathQuery>,
) -> ApiResult<Json<Value>> {
    if !sk_db::validate_identifier(&table, 64) {
        return Err(ApiError::bad_request("invalid table"));
    }
    let path = sqlite_path(q.path)?;
    let pool = sqlite_pool(&path, true).await?;
    let rows = sqlx::query(&format!("PRAGMA table_info(`{table}`)"))
        .fetch_all(&pool)
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(
        json!({"columns":["cid","name","type","notnull","dflt_value","pk"],"rows":rows.iter().map(|r|json!([sqlite_value(r,0),sqlite_value(r,1),sqlite_value(r,2),sqlite_value(r,3),sqlite_value(r,4),sqlite_value(r,5)])).collect::<Vec<_>>(),"structure":rows.iter().map(|r|json!({"name":sqlite_value(r,1),"type":sqlite_value(r,2),"notnull":sqlite_value(r,3),"default":sqlite_value(r,4),"pk":sqlite_value(r,5)})).collect::<Vec<_>>() }),
    ))
}
#[derive(Deserialize)]
struct SqliteQueryBody {
    path: Option<String>,
    query: Option<String>,
    readonly: Option<bool>,
}
async fn sqlite_query(
    AuthUser(u): AuthUser,
    Json(b): Json<SqliteQueryBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let path = sqlite_path(b.path)?;
    let query = b
        .query
        .ok_or_else(|| ApiError::bad_request("query is required"))?;
    let readonly = b.readonly.unwrap_or(true);
    if readonly && !sk_db::is_readonly_query(&query) {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"success":false,"error":"Only SELECT, SHOW, DESCRIBE, and EXPLAIN queries are allowed in readonly mode"}),
            ),
        ));
    }
    if !readonly && !u.is_admin() {
        return Err(ApiError::forbidden(
            "Admin access required to execute write queries",
        ));
    }
    let pool = sqlite_pool(&path, readonly).await?;
    let start = std::time::Instant::now();
    if readonly {
        let rows = sqlx::query(&query)
            .fetch_all(&pool)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        let columns: Vec<String> = rows
            .first()
            .map(|r| r.columns().iter().map(|c| c.name().to_string()).collect())
            .unwrap_or_default();
        let out_rows: Vec<Vec<Value>> = rows
            .iter()
            .take(1000)
            .map(|r| (0..r.columns().len()).map(|i| sqlite_value(r, i)).collect())
            .collect();
        Ok((
            StatusCode::OK,
            Json(
                json!({"success":true,"columns":columns,"rows":out_rows,"row_count":out_rows.len(),"execution_time":(start.elapsed().as_secs_f64()*10000.0).round()/10000.0,"truncated":rows.len()>1000}),
            ),
        ))
    } else {
        let result = sqlx::query(&query)
            .execute(&pool)
            .await
            .map_err(|e| ApiError::bad_request(e.to_string()))?;
        Ok((
            StatusCode::OK,
            Json(
                json!({"success":true,"rows_affected":result.rows_affected(),"execution_time":(start.elapsed().as_secs_f64()*10000.0).round()/10000.0}),
            ),
        ))
    }
}

fn managed_id() -> String {
    use rand::{distributions::Alphanumeric, Rng};
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(16)
        .map(char::from)
        .collect()
}
async fn ensure_managed_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query("CREATE TABLE IF NOT EXISTS sk_managed_databases(id TEXT PRIMARY KEY,name TEXT NOT NULL,engine TEXT NOT NULL,host TEXT,port INTEGER,database_name TEXT NOT NULL,username TEXT,secret_encrypted TEXT,metadata_json TEXT NOT NULL DEFAULT '{}',protection_json TEXT,created_at TEXT NOT NULL,updated_at TEXT NOT NULL)").execute(pool).await?;
    Ok(())
}
fn managed_row(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"engine":r.get::<String,_>("engine"),"host":r.try_get::<Option<String>,_>("host").ok().flatten(),"port":r.try_get::<Option<i64>,_>("port").ok().flatten(),"database":r.get::<String,_>("database_name"),"username":r.try_get::<Option<String>,_>("username").ok().flatten(),"has_secret":r.try_get::<Option<String>,_>("secret_encrypted").ok().flatten().is_some(),"metadata":r.try_get::<String,_>("metadata_json").ok().and_then(|s|serde_json::from_str::<Value>(&s).ok()).unwrap_or(Value::Null),"protection":r.try_get::<Option<String>,_>("protection_json").ok().flatten().and_then(|s|serde_json::from_str::<Value>(&s).ok()),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
#[derive(Deserialize)]
struct ManagedBody {
    name: Option<String>,
    engine: Option<String>,
    host: Option<String>,
    port: Option<i64>,
    database: Option<String>,
    database_name: Option<String>,
    username: Option<String>,
    password: Option<String>,
}
async fn managed_list(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    ensure_managed_schema(&s.db).await?;
    let rows = sqlx::query("SELECT * FROM sk_managed_databases ORDER BY name")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(
        json!({"databases":rows.iter().map(managed_row).collect::<Vec<_>>() }),
    ))
}
async fn managed_get(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    ensure_managed_schema(&s.db).await?;
    let r = sqlx::query("SELECT * FROM sk_managed_databases WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?;
    Ok(Json(r.as_ref().map(managed_row).unwrap_or_else(
        || json!({"success":false,"error":"managed database not found"}),
    )))
}
async fn upsert_managed(s: &SharedState, b: ManagedBody, adopt: bool) -> ApiResult<Value> {
    ensure_managed_schema(&s.db).await?;
    let id = managed_id();
    let ts = chrono::Utc::now().to_rfc3339();
    let engine = b.engine.unwrap_or_else(|| "mysql".into());
    let database = b
        .database_name
        .or(b.database)
        .ok_or_else(|| ApiError::bad_request("database is required"))?;
    let name = b.name.unwrap_or_else(|| database.clone());
    sqlx::query("INSERT INTO sk_managed_databases(id,name,engine,host,port,database_name,username,secret_encrypted,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(name).bind(engine).bind(b.host).bind(b.port).bind(database).bind(b.username).bind(b.password.map(|p|sk_core::crypto::encrypt(&p))).bind(json!({"adopted":adopt}).to_string()).bind(&ts).bind(&ts).execute(&s.db).await?;
    let r = sqlx::query("SELECT * FROM sk_managed_databases WHERE id=?")
        .bind(id)
        .fetch_one(&s.db)
        .await?;
    Ok(managed_row(&r))
}
async fn managed_create(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<ManagedBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(upsert_managed(&s, b, false).await?))
}
async fn managed_adopt(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<ManagedBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(upsert_managed(&s, b, true).await?))
}
#[derive(Deserialize)]
struct DropQuery {
    drop: Option<bool>,
}
async fn managed_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Query(q): Query<DropQuery>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    ensure_managed_schema(&s.db).await?;
    if q.drop.unwrap_or(false) {
        return Err(ApiError::bad_request("physical drop for managed database records requires the engine-specific delete endpoint"));
    }
    let n = sqlx::query("DELETE FROM sk_managed_databases WHERE id=?")
        .bind(id)
        .execute(&s.db)
        .await?
        .rows_affected();
    Ok(Json(json!({"success":n>0})))
}
async fn managed_connection_uri(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    ensure_managed_schema(&s.db).await?;
    let r = sqlx::query("SELECT * FROM sk_managed_databases WHERE id=?")
        .bind(id)
        .fetch_optional(&s.db)
        .await?;
    let Some(r) = r else {
        return Err(ApiError::not_found("managed database not found"));
    };
    let engine: String = r.get("engine");
    let user = r
        .try_get::<Option<String>, _>("username")
        .ok()
        .flatten()
        .unwrap_or_default();
    let secret = r
        .try_get::<Option<String>, _>("secret_encrypted")
        .ok()
        .flatten()
        .and_then(|v| sk_core::crypto::decrypt(&v))
        .unwrap_or_default();
    let host = r
        .try_get::<Option<String>, _>("host")
        .ok()
        .flatten()
        .unwrap_or_else(|| "localhost".into());
    let port = r.try_get::<Option<i64>, _>("port").ok().flatten();
    let db: String = r.get("database_name");
    let uri = match engine.as_str() {
        "postgresql" => format!(
            "postgresql://{user}:{secret}@{host}:{}/{}",
            port.unwrap_or(5432),
            db
        ),
        "sqlite" => format!("sqlite://{db}"),
        _ => format!(
            "mysql://{user}:{secret}@{host}:{}/{}",
            port.unwrap_or(3306),
            db
        ),
    };
    Ok(Json(json!({"connection_uri":uri,"engine":engine})))
}
async fn managed_protect(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    ensure_managed_schema(&s.db).await?;
    sqlx::query("UPDATE sk_managed_databases SET protection_json=?, updated_at=? WHERE id=?")
        .bind(b.get("policy").cloned().unwrap_or(b).to_string())
        .bind(chrono::Utc::now().to_rfc3339())
        .bind(&id)
        .execute(&s.db)
        .await?;
    managed_get(State(s), AuthUser(u), Path(id)).await
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
