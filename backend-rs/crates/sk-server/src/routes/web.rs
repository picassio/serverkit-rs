//! Ports of `app/api/nginx.py` and `app/api/php.py`. All admin-only.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Map, Value};

pub fn nginx_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(nginx_status))
        .route("/versions", get(nginx_versions))
        .route("/versions/{target}/install", post(nginx_install_version))
        .route("/test", post(nginx_test))
        .route("/reload", post(nginx_reload))
        .route("/restart", post(nginx_restart))
        .route("/sites", get(list_sites).post(create_site))
        .route("/sites/{name}", delete(delete_site))
        .route("/sites/{name}/enable", post(enable_site))
        .route("/sites/{name}/disable", post(disable_site))
        .route("/sites/{name}/ssl", post(add_ssl))
        .route("/advanced/proxy", post(create_proxy))
        .route("/advanced/proxy/{name}", get(get_proxy))
        .route("/advanced/test", post(nginx_test))
        .route("/advanced/reload", post(nginx_reload))
        .route("/advanced/diff", post(diff_config))
        .route("/advanced/logs/{name}", get(vhost_logs))
        .route("/advanced/lb-methods", get(lb_methods))
}

pub fn domains_router() -> Router<SharedState> {
    Router::new()
        .route("/", get(domains_list).post(domain_create))
        .route("/base-domains", get(base_domains))
        .route("/give-subdomain", post(give_subdomain))
        .route("/nginx/sites", get(domains_nginx_sites))
        .route("/ssl/status", get(domains_ssl_status))
        .route("/suggest-subdomain", get(suggest_subdomain))
        .route(
            "/{id}",
            get(domain_get).put(domain_update).delete(domain_delete),
        )
        .route("/{id}/ssl/enable", post(domain_ssl_enable))
        .route("/{id}/ssl/disable", post(domain_ssl_disable))
        .route("/{id}/ssl/renew", post(domain_ssl_renew))
        .route("/{id}/verify", get(domain_verify))
}

pub fn ssl_router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(ssl_status))
        .route("/certificates", get(ssl_certificates).post(ssl_obtain))
        .route("/certificates/renew-all", post(ssl_renew_all))
        .route("/certificates/{domain}", delete(ssl_delete))
        .route("/certificates/{domain}/renew", post(ssl_renew))
        .route("/auto-renewal", post(ssl_auto_renewal))
        .route("/install-certbot", post(ssl_install_certbot))
        .route("/profiles", get(ssl_profiles))
        .route("/wildcard", post(ssl_wildcard))
        .route("/san", post(ssl_san))
        .route("/upload", post(ssl_upload))
        .route("/health/{domain}", get(ssl_health))
        .route("/expiry-alerts", get(ssl_expiry_alerts))
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

async fn nginx_versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({
        "installed": sk_web::nginx::installed_version().await,
        "supported": sk_web::nginx::SUPPORTED_NGINX_TARGETS,
    })))
}

async fn nginx_install_version(
    AuthUser(u): AuthUser,
    Path(target): Path<String>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    Ok(by_success(
        sk_web::nginx::install_version(&target).await,
        StatusCode::OK,
    ))
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

async fn get_proxy(AuthUser(u): AuthUser, Path(name): Path<String>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::nginx::proxy_rules(&name).await))
}

async fn create_proxy(
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let domain = b
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("domain is required"))?;
    let upstream = b
        .get("upstream")
        .or_else(|| b.get("proxy_pass"))
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("upstream/proxy_pass is required"))?;
    let name = b.get("name").and_then(Value::as_str).unwrap_or(domain);
    Ok(by_success(
        sk_web::nginx::create_proxy_site(name, domain, upstream).await,
        StatusCode::CREATED,
    ))
}

async fn diff_config(AuthUser(u): AuthUser, Json(b): Json<Value>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let domain = b
        .get("domain")
        .and_then(Value::as_str)
        .ok_or_else(|| ApiError::bad_request("domain is required"))?;
    let config = b.get("config").and_then(Value::as_str).unwrap_or("");
    Ok(Json(sk_web::nginx::diff_site_config(domain, config).await))
}

#[derive(Deserialize)]
struct LogsQuery {
    #[serde(rename = "type")]
    kind: Option<String>,
    lines: Option<usize>,
}

async fn vhost_logs(
    AuthUser(u): AuthUser,
    Path(name): Path<String>,
    Query(q): Query<LogsQuery>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::nginx::vhost_logs(
        &name,
        q.kind.as_deref().unwrap_or("access"),
        q.lines.unwrap_or(100),
    )))
}

async fn lb_methods(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::nginx::lb_methods()))
}

// ==================== DOMAINS ====================

async fn domains_list(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::domains::list(&s.db).await?))
}
async fn domain_get(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_web::domains::get(&s.db, &id)
            .await?
            .unwrap_or(Value::Null),
    ))
}
async fn domain_create(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::create(&s.db, &b).await?))
}
async fn domain_update(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::update(&s.db, &id, &b).await?))
}
async fn domain_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::delete(&s.db, &id).await?))
}
async fn domain_verify(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::domains::verify(&s.db, &id).await?))
}
async fn domain_ssl_enable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::enable_ssl(&s.db, &id, &b).await?))
}
async fn domain_ssl_disable(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::disable_ssl(&s.db, &id).await?))
}
async fn domain_ssl_renew(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::renew_ssl(&s.db, &id).await?))
}
async fn base_domains(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::domains::base_domains(&s.db).await?))
}
#[derive(Deserialize)]
struct SuggestQuery {
    application_id: String,
    base: Option<String>,
}
async fn suggest_subdomain(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<SuggestQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_web::domains::suggest_subdomain(&s.db, &q.application_id, q.base.as_deref()).await?,
    ))
}
async fn give_subdomain(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::domains::give_subdomain(&s.db, &b).await?))
}
async fn domains_nginx_sites(AuthUser(_u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::domains::nginx_sites().await?))
}
async fn domains_ssl_status(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::domains::ssl_status(&s.db).await?))
}

// ==================== SSL ====================

async fn ssl_status(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::ssl::status(&s.db).await?))
}
async fn ssl_certificates(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::ssl::list(&s.db).await?))
}
async fn ssl_obtain(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::obtain(&s.db, &b).await?))
}
async fn ssl_renew(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(domain): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::renew(&s.db, &domain).await?))
}
async fn ssl_renew_all(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::renew_all(&s.db).await?))
}
async fn ssl_delete(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(domain): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::delete(&s.db, &domain).await?))
}
async fn ssl_auto_renewal(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::setup_auto_renewal(&s.db, &b).await?))
}
async fn ssl_install_certbot(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::install_certbot().await?))
}
async fn ssl_profiles(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_web::ssl::profiles())
}
async fn ssl_wildcard(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::wildcard(&s.db, &b).await?))
}
async fn ssl_san(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::san(&s.db, &b).await?))
}
async fn ssl_upload(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_web::ssl::upload(&s.db, &b).await?))
}
async fn ssl_health(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(domain): Path<String>,
) -> ApiResult<Json<Value>> {
    Ok(Json(sk_web::ssl::health(&s.db, &domain).await?))
}
#[derive(Deserialize)]
struct ExpiryQuery {
    days: Option<i64>,
}
async fn ssl_expiry_alerts(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Query(q): Query<ExpiryQuery>,
) -> ApiResult<Json<Value>> {
    Ok(Json(
        sk_web::ssl::expiry_alerts(&s.db, q.days.unwrap_or(30)).await?,
    ))
}

// ==================== PHP ====================

async fn php_versions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(json!({
        "versions": sk_web::php::installed_versions().await,
        "default": sk_web::php::default_version().await,
        "supported": sk_web::php::SUPPORTED_VERSIONS,
        "supported_extensions": sk_web::php::SUPPORTED_EXTENSIONS,
        "profiles": ["minimal", "magento", "all-supported"],
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

#[derive(Deserialize, Default)]
struct PhpInstallBody {
    extensions: Option<Vec<String>>,
    profile: Option<String>,
    set_default: Option<bool>,
}

async fn install_version(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
    body: Option<Json<PhpInstallBody>>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let b = body.map(|b| b.0).unwrap_or_default();
    let mut extensions: Vec<&str> = if let Some(profile) = b.profile.as_deref() {
        sk_web::php::extension_profile(profile)
            .ok_or_else(|| {
                ApiError::bad_request(format!("Unsupported PHP extension profile: {profile}"))
            })?
            .to_vec()
    } else {
        sk_web::php::MAGENTO_EXTENSIONS.to_vec()
    };
    if let Some(extra) = &b.extensions {
        extensions = extra.iter().map(String::as_str).collect();
    }
    Ok(by_success(
        sk_web::php::install_version_with_options(
            &version,
            sk_web::php::InstallOptions {
                extensions,
                set_default: b.set_default.unwrap_or(false),
            },
        )
        .await,
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
    extensions: Option<Vec<String>>,
}

async fn install_extension(
    AuthUser(u): AuthUser,
    Path(version): Path<String>,
    Json(b): Json<ExtensionBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    if let Some(exts) = b.extensions {
        let refs: Vec<&str> = exts.iter().map(String::as_str).collect();
        return Ok(by_success(
            sk_web::php::install_version_with_options(
                &version,
                sk_web::php::InstallOptions {
                    extensions: refs,
                    set_default: false,
                },
            )
            .await,
            StatusCode::OK,
        ));
    }
    let ext = b
        .extension
        .ok_or_else(|| ApiError::bad_request("extension or extensions is required"))?;
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
