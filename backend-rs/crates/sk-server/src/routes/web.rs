//! Ports of `app/api/nginx.py` and `app/api/php.py`. All admin-only.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::Path;
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub fn nginx_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(nginx_status))
        .route("/test", post(nginx_test))
        .route("/reload", post(nginx_reload))
        .route("/restart", post(nginx_restart))
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/{name}", delete(delete_site))
        .route("/sites/{name}/enable", post(enable_site))
        .route("/sites/{name}/disable", post(disable_site))
        .route("/sites/{name}/ssl", post(add_ssl))
}

pub fn php_router() -> Router<SharedState> {
    Router::new()
        .route("/versions", get(php_versions))
        .route("/versions/default", post(set_default))
        .route("/versions/{version}/install", post(install_version))
        .route(
            "/versions/{version}/extensions",
            get(get_extensions).post(install_extension),
        )
        .route(
            "/versions/{version}/pools",
            get(get_pools).post(create_pool),
        )
        .route("/versions/{version}/pools/{pool}", delete(delete_pool))
        .route("/versions/{version}/fpm/restart", post(fpm_restart))
        .route("/versions/{version}/fpm/reload", post(fpm_reload))
        .route("/versions/{version}/fpm/status", get(fpm_status))
        .route("/versions/{version}/info", get(php_info))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
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

// ==================== NGINX ====================

async fn nginx_status(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::nginx::status().await))
}

async fn nginx_test(AuthUser(u): AuthUser) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::nginx::test_config().await,
        StatusCode::OK,
    ))
}

async fn nginx_reload(AuthUser(u): AuthUser) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(sk_web::nginx::reload().await, StatusCode::OK))
}

async fn nginx_restart(AuthUser(u): AuthUser) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(sk_web::nginx::restart().await, StatusCode::OK))
}

async fn list_sites(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let sites = tokio::task::spawn_blocking(sk_web::nginx::list_sites)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "sites": sites })))
}

#[derive(Deserialize)]
struct CreateSiteBody {
    name: Option<String>,
    app_type: Option<String>,
    domains: Option<Vec<String>>,
    root_path: Option<String>,
    port: Option<u16>,
    php_version: Option<String>,
    ssl_cert: Option<String>,
    ssl_key: Option<String>,
}

async fn create_site(
    AuthUser(u): AuthUser,
    Json(b): Json<CreateSiteBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let mut missing = Vec::new();
    if b.name.is_none() {
        missing.push("name");
    }
    if b.app_type.is_none() {
        missing.push("app_type");
    }
    if b.domains.is_none() {
        missing.push("domains");
    }
    if b.root_path.is_none() {
        missing.push("root_path");
    }
    if !missing.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Missing required fields: {}",
            missing.join(", ")
        )));
    }
    let spec = sk_web::nginx::SiteSpec {
        name: b.name.unwrap(),
        app_type: b.app_type.unwrap(),
        domains: b.domains.unwrap(),
        root_path: b.root_path.unwrap(),
        port: b.port,
        php_version: b.php_version.unwrap_or_else(|| "8.2".into()),
        ssl_cert: b.ssl_cert,
        ssl_key: b.ssl_key,
    };
    Ok(by_success(
        sk_web::nginx::create_site(&spec).await,
        StatusCode::CREATED,
    ))
}

async fn enable_site(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::nginx::enable_site(&name).await,
        StatusCode::OK,
    ))
}

async fn disable_site(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::nginx::disable_site(&name).await,
        StatusCode::OK,
    ))
}

async fn delete_site(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::nginx::delete_site(&name).await,
        StatusCode::OK,
    ))
}

#[derive(Deserialize)]
struct SslBody {
    cert_path: Option<String>,
    key_path: Option<String>,
}

async fn add_ssl(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Json(b): Json<SslBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let (Some(cert), Some(key)) = (b.cert_path, b.key_path) else {
        return Err(ApiError::bad_request("cert_path and key_path are required"));
    };
    Ok(by_success(
        sk_web::nginx::add_ssl_to_site(&name, &cert, &key).await,
        StatusCode::OK,
    ))
}

// ==================== PHP ====================

async fn php_versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({
        "versions": sk_web::php::installed_versions().await,
        "default": sk_web::php::default_version().await,
        "supported": sk_web::php::SUPPORTED_VERSIONS,
    })))
}

#[derive(Deserialize)]
struct VersionBody {
    version: Option<String>,
}

async fn set_default(
    AuthUser(u): AuthUser,
    Json(b): Json<VersionBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let version = b
        .version
        .ok_or_else(|| ApiError::bad_request("version is required"))?;
    Ok(by_success(
        sk_web::php::set_default_version(&version).await,
        StatusCode::OK,
    ))
}

async fn install_version(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::php::install_version(&version).await,
        StatusCode::OK,
    ))
}

async fn get_extensions(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({
        "extensions": sk_web::php::extensions(&version).await,
        "version": version,
    })))
}

#[derive(Deserialize)]
struct ExtensionBody {
    extension: Option<String>,
}

async fn install_extension(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
    Json(b): Json<ExtensionBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let ext = b
        .extension
        .ok_or_else(|| ApiError::bad_request("extension is required"))?;
    Ok(by_success(
        sk_web::php::install_extension(&version, &ext).await,
        StatusCode::OK,
    ))
}

async fn get_pools(AuthUser(u): AuthUser, Path(version): Path<String>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let pools = tokio::task::spawn_blocking(move || sk_web::php::pools(&version))
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(json!({ "pools": pools })))
}

#[derive(Deserialize)]
struct PoolBody {
    name: Option<String>,
    #[serde(default)]
    config: Map<String, Value>,
}

async fn create_pool(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
    Json(b): Json<PoolBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let name = b
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    Ok(by_success(
        sk_web::php::create_pool(&version, &name, &b.config).await,
        StatusCode::CREATED,
    ))
}

async fn delete_pool(
    AuthUser(u): AuthUser,
    Path((version, pool)): Path<(String, String)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::php::delete_pool(&version, &pool).await,
        StatusCode::OK,
    ))
}

async fn fpm_restart(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::php::restart_fpm(&version).await,
        StatusCode::OK,
    ))
}

async fn fpm_reload(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::php::reload_fpm(&version).await,
        StatusCode::OK,
    ))
}

async fn fpm_status(AuthUser(u): AuthUser, Path(version): Path<String>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::php::fpm_status(&version).await))
}

async fn php_info(AuthUser(u): AuthUser, Path(version): Path<String>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::php::php_info(&version).await))
}
