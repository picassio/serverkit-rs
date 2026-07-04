//! Port of `app/api/templates.py` (catalog + install/uninstall). The
//! deployment-job machinery is replaced by a direct compose deploy.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;

fn template_repos_path() -> PathBuf {
    PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
        .join("template_repos.json")
}

fn load_repos() -> Vec<Value> {
    std::fs::read_to_string(template_repos_path())
        .ok()
        .and_then(|s| serde_json::from_str::<Value>(&s).ok())
        .and_then(|v| v.get("repos").and_then(Value::as_array).cloned())
        .unwrap_or_default()
}

fn save_repos(repos: &[Value]) -> std::io::Result<()> {
    let path = template_repos_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, serde_json::to_vec_pretty(&json!({"repos": repos}))?)
}

pub fn router() -> Router<SharedState> {
    // Full paths + merged (not nested) so both `/templates` and `/templates/`
    // match — the frontend calls the trailing-slash form.
    Router::new()
        .route("/templates", get(list))
        .route("/templates/", get(list))
        .route("/templates/categories", get(categories))
        .route(
            "/templates/repos",
            get(repos).post(add_repo).delete(remove_repo),
        )
        .route("/templates/sync", post(sync_templates))
        .route("/templates/test-db-connection", post(test_db_connection))
        .route("/templates/apps/{id}/check-update", get(app_check_update))
        .route("/templates/apps/{id}/update", post(app_update))
        .route("/templates/apps/{id}/template-info", get(app_template_info))
        .route("/templates/installed", get(installed))
        .route(
            "/templates/installed/{name}",
            axum::routing::delete(uninstall),
        )
        .route("/templates/validate-install", post(validate))
        .route("/templates/{id}", get(detail))
        .route("/templates/{id}/install", post(install))
}

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

#[derive(Deserialize)]
struct ListQuery {
    category: Option<String>,
    search: Option<String>,
}

async fn list(AuthUser(_u): AuthUser, Query(q): Query<ListQuery>) -> Json<Value> {
    let templates = sk_templates::list(q.category.as_deref(), q.search.as_deref());
    Json(json!({ "count": templates.len(), "templates": templates }))
}

async fn categories(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "categories": sk_templates::categories() }))
}

async fn detail(AuthUser(_u): AuthUser, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    match sk_templates::detail(&id) {
        Some(t) => Ok(Json(json!({ "template": t }))),
        None => Err(ApiError::not_found("Template not found")),
    }
}

#[derive(Deserialize)]
struct InstallBody {
    app_name: Option<String>,
    #[serde(default)]
    variables: Value,
}

async fn install(
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<InstallBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let app_name = b
        .app_name
        .ok_or_else(|| ApiError::bad_request("app_name is required"))?;
    let vars = if b.variables.is_object() {
        b.variables
    } else {
        json!({})
    };
    let result = sk_templates::install(&id, &app_name, &vars).await;
    let ok = result["success"].as_bool().unwrap_or(false);
    Ok((
        if ok {
            StatusCode::CREATED
        } else {
            StatusCode::BAD_REQUEST
        },
        Json(result),
    ))
}

#[derive(Deserialize)]
struct ValidateBody {
    template_id: Option<String>,
    app_name: Option<String>,
    #[serde(default)]
    variables: Value,
}

async fn validate(
    AuthUser(_u): AuthUser,
    Json(b): Json<ValidateBody>,
) -> (StatusCode, Json<Value>) {
    let mut errors: Vec<String> = Vec::new();
    match &b.app_name {
        None => errors.push("App name is required".into()),
        Some(n) if !sk_templates::app_name_valid(n) => {
            errors.push("Invalid app name (3-63 chars, lowercase/digits/hyphens)".into())
        }
        _ => {}
    }
    match b.template_id.as_deref().map(sk_templates::detail) {
        None => errors.push("Template ID is required".into()),
        Some(None) => errors.push("Template not found".into()),
        Some(Some(t)) => {
            let provided = b.variables.as_object();
            if let Some(vars) = t["variables"].as_array() {
                for v in vars {
                    if v["required"].as_bool() == Some(true) {
                        let name = v["name"].as_str().unwrap_or("");
                        let has = provided.map(|p| p.contains_key(name)).unwrap_or(false);
                        if !name.is_empty() && !has {
                            errors.push(format!("Required variable \"{name}\" is not provided"));
                        }
                    }
                }
            }
        }
    }
    if errors.is_empty() {
        (StatusCode::OK, Json(json!({ "valid": true })))
    } else {
        (
            StatusCode::BAD_REQUEST,
            Json(json!({ "valid": false, "errors": errors })),
        )
    }
}

async fn installed(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "apps": sk_templates::list_installed() }))
}

async fn uninstall(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let result = sk_templates::uninstall(&name).await;
    let ok = result["success"].as_bool().unwrap_or(false);
    Ok((
        if ok {
            StatusCode::OK
        } else {
            StatusCode::BAD_REQUEST
        },
        Json(result),
    ))
}

#[derive(Deserialize)]
struct RepoBody {
    name: Option<String>,
    url: Option<String>,
}

async fn repos(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "success": true, "repos": load_repos() }))
}

async fn add_repo(
    AuthUser(u): AuthUser,
    Json(body): Json<RepoBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let url = body
        .url
        .filter(|u| u.starts_with("https://") || u.starts_with("http://") || u.starts_with("git@"))
        .ok_or_else(|| ApiError::bad_request("valid repository url is required"))?;
    let mut repos = load_repos();
    let name = body.name.unwrap_or_else(|| url.clone());
    if !repos
        .iter()
        .any(|r| r["url"].as_str() == Some(url.as_str()))
    {
        repos.push(json!({"name": name, "url": url, "enabled": true, "added_at": chrono::Utc::now().to_rfc3339()}));
        save_repos(&repos).map_err(|e| ApiError::bad_request(e.to_string()))?;
    }
    Ok((
        StatusCode::CREATED,
        Json(json!({"success": true, "repos": repos})),
    ))
}

async fn remove_repo(AuthUser(u): AuthUser, Json(body): Json<RepoBody>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let url = body
        .url
        .ok_or_else(|| ApiError::bad_request("url is required"))?;
    let mut repos = load_repos();
    let before = repos.len();
    repos.retain(|r| r["url"].as_str() != Some(url.as_str()));
    save_repos(&repos).map_err(|e| ApiError::bad_request(e.to_string()))?;
    Ok(Json(
        json!({"success": true, "deleted": before.saturating_sub(repos.len()), "repos": repos}),
    ))
}

async fn sync_templates(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let repos = load_repos();
    // Local bundled templates are authoritative in this Rust build. Remote repos are persisted
    // as desired state and reported honestly until a repository fetch adapter is configured.
    Ok(Json(json!({
        "success": true,
        "synced": sk_templates::catalog().len(),
        "remote_repos": repos.len(),
        "remote_sync": {"configured": false, "reason": "REMOTE_TEMPLATE_REPO_SYNC_NOT_CONFIGURED"}
    })))
}

async fn test_db_connection(
    AuthUser(_u): AuthUser,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    let engine = body
        .get("engine")
        .or_else(|| body.get("type"))
        .and_then(Value::as_str)
        .unwrap_or("mysql");
    let host = body
        .get("host")
        .and_then(Value::as_str)
        .unwrap_or("127.0.0.1");
    let port = body
        .get("port")
        .and_then(Value::as_i64)
        .unwrap_or(if engine == "postgres" { 5432 } else { 3306 });
    let addr = format!("{host}:{port}");
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio::net::TcpStream::connect(&addr),
    )
    .await
    {
        Ok(Ok(_)) => (
            StatusCode::OK,
            Json(json!({"success": true, "reachable": true, "engine": engine, "address": addr})),
        ),
        Ok(Err(e)) => (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"success": false, "reachable": false, "engine": engine, "address": addr, "error": e.to_string()}),
            ),
        ),
        Err(_) => (
            StatusCode::BAD_REQUEST,
            Json(
                json!({"success": false, "reachable": false, "engine": engine, "address": addr, "error": "connection timed out"}),
            ),
        ),
    }
}

async fn app_template_info(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let Some(app) = sk_apps::get(&s.db, &id).await.map_err(ApiError::from)? else {
        return Err(ApiError::not_found("App not found"));
    };
    Ok(Json(
        json!({"success": true, "template": {"app_id": id, "template_id": app["template_id"].clone(), "app": app}}),
    ))
}

async fn app_check_update(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    let Some(app) = sk_apps::get(&s.db, &id).await.map_err(ApiError::from)? else {
        return Err(ApiError::not_found("App not found"));
    };
    let template_id = app["template_id"].as_str().unwrap_or("");
    let current = app["template_version"].as_str().unwrap_or("");
    let latest =
        sk_templates::detail(template_id).and_then(|t| t["version"].as_str().map(str::to_string));
    Ok(Json(
        json!({"success": true, "app_id": id, "template_id": template_id, "current_version": current, "latest_version": latest, "update_available": latest.as_deref().is_some_and(|v| !current.is_empty() && v != current)}),
    ))
}

async fn app_update(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let Some(app) = sk_apps::get(&s.db, &id).await.map_err(ApiError::from)? else {
        return Err(ApiError::not_found("App not found"));
    };
    let template_id = app["template_id"].as_str().unwrap_or("");
    if template_id.is_empty() || sk_templates::detail(template_id).is_none() {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(
                json!({"success": false, "code": "APP_TEMPLATE_NOT_MANAGED", "error": "App is not linked to a bundled template"}),
            ),
        ));
    }
    Ok((
        StatusCode::BAD_REQUEST,
        Json(
            json!({"success": false, "code": "TEMPLATE_UPDATE_REDEPLOY_REQUIRED", "error": "Automatic in-place template updates are not safe; reinstall or redeploy with explicit migration plan", "app": app}),
        ),
    ))
}
