//! /api/v1/magento — the sk-magento extension API (fork-native; no Flask
//! oracle). Store CRUD + provisioning, quick actions, health, provisioning log.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};
use sk_magento::{provision, store};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/versions", get(versions))
        .route("/actions", get(actions_list))
        .route("/stores", get(list_stores).post(create_store))
        .route(
            "/stores/{id}",
            get(get_store).patch(patch_store).delete(delete_store),
        )
        .route("/stores/{id}/apply-web", post(apply_web))
        .route("/stores/{id}/frontend/{action}", post(frontend_action))
        .route("/stores/{id}/log", get(store_log))
        .route("/stores/{id}/vhost", get(get_vhost).put(put_vhost))
        .route("/stores/{id}/renew-cert", post(renew_cert))
        .route(
            "/stores/{id}/backups",
            get(list_backups).post(create_backup),
        )
        .route("/stores/{id}/backups/policy", post(set_backup_policy))
        .route(
            "/stores/{id}/backups/{filename}",
            axum::routing::delete(delete_backup),
        )
        .route(
            "/stores/{id}/backups/{filename}/restore",
            post(restore_backup),
        )
        .route("/stores/{id}/health", get(store_health))
        .route("/stores/{id}/actions/{action}", post(run_action))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

async fn versions(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_magento::versions_payload())
}

async fn actions_list(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "actions": sk_magento::actions::list_actions() }))
}

async fn list_stores(
    State(state): State<SharedState>,
    AuthUser(_u): AuthUser,
) -> ApiResult<Json<Value>> {
    store::ensure_schema(&state.db).await?;
    let stores = store::list(&state.db).await?;
    Ok(Json(json!({
        "stores": stores.iter().map(|s| s.to_dict(false)).collect::<Vec<_>>()
    })))
}

#[derive(Deserialize)]
struct RevealQuery {
    reveal: Option<String>,
}

async fn get_store(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Query(q): Query<RevealQuery>,
) -> ApiResult<Json<Value>> {
    store::ensure_schema(&state.db).await?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let reveal = q
        .reveal
        .as_deref()
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    if reveal {
        require_admin(&u)?; // credential reveal is admin-only
    }
    Ok(Json(json!({ "store": s.to_dict(reveal) })))
}

#[derive(Deserialize)]
struct CreateStoreBody {
    name: Option<String>,
    domain: Option<String>,
    magento_version: Option<String>,
    distribution: Option<String>,
    php_version: Option<String>,
    base_dir: Option<String>,
    /// "none" (default) | "self-signed" | "letsencrypt"
    ssl: Option<String>,
    /// LE challenge: "dns" (Cloudflare, default) | "http" (webroot)
    le_challenge: Option<String>,
    le_email: Option<String>,
    use_rabbitmq: Option<bool>,
    use_varnish: Option<bool>,
    /// Fully custom project root (overrides base_dir/{name}).
    root_path: Option<String>,
    /// "none" (default) | "shared" | "separate" | "split" (frontend_domain +
    /// api domain (allowlisted) + admin_domain).
    headless_mode: Option<String>,
    admin_domain: Option<String>,
    frontend_cmd: Option<String>,
    frontend_domain: Option<String>,
    /// Node app port; 0 + frontend_root = serve static export via nginx.
    frontend_port: Option<i64>,
    /// Headless app project folder (static root, or reference for proxy mode).
    frontend_root: Option<String>,
    /// Extra path prefixes routed to Magento in shared mode (e.g. "/checkout").
    magento_routes: Option<Vec<String>>,
    /// Unix user PHP-FPM + files + the frontend process run as (default www-data).
    run_user: Option<String>,
    /// Per-service image overrides, e.g. {"db":"mariadb:11.4","redis":"redis:7.4"}.
    service_versions: Option<serde_json::Map<String, Value>>,
    /// False means create/start only the data-plane stack; do not run composer/setup:install.
    install_magento: Option<bool>,
    /// Existing Magento source tree, e.g. /srv/shop/current. Defaults to root_path/src.
    magento_source_path: Option<String>,
    /// Install missing host PHP/FPM + selected extensions before creating the stack.
    auto_install_php: Option<bool>,
    /// PHP extension profile or explicit extension list for auto-install.
    php_extension_profile: Option<String>,
    php_extensions: Option<Vec<String>>,
}

/// POST /magento/stores — insert row + spawn the provisioning task.
async fn create_store(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<CreateStoreBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    store::ensure_schema(&state.db).await?;

    let name = b
        .name
        .ok_or_else(|| ApiError::bad_request("name is required"))?;
    let domain = b
        .domain
        .ok_or_else(|| ApiError::bad_request("domain is required"))?;
    if !sk_magento::valid_store_name(&name) {
        return Err(ApiError::bad_request(
            "Invalid store name: lowercase letters, digits and hyphens, starting with a letter",
        ));
    }
    if store::name_taken(&state.db, &name).await? {
        return Err(ApiError::conflict("A store with this name already exists"));
    }

    let magento_version = b.magento_version.unwrap_or_else(|| "2.4.8".into());
    let (composer, recommended_php) =
        sk_magento::matrix_lookup(&magento_version).ok_or_else(|| {
            ApiError::bad_request(format!("Unsupported Magento version: {magento_version}"))
        })?;
    let php_version = b.php_version.unwrap_or_else(|| recommended_php.to_string());
    let distribution = b.distribution.unwrap_or_else(|| "mage-os".into());
    if !matches!(distribution.as_str(), "mage-os" | "magento") {
        return Err(ApiError::bad_request(
            "distribution must be 'mage-os' or 'magento'",
        ));
    }

    let install_magento = b.install_magento.unwrap_or(false);

    let ssl_mode = b.ssl.unwrap_or_else(|| "none".into());
    if !matches!(ssl_mode.as_str(), "none" | "self-signed" | "letsencrypt") {
        return Err(ApiError::bad_request(
            "ssl must be 'none', 'self-signed' or 'letsencrypt'",
        ));
    }
    let le_challenge = b.le_challenge.unwrap_or_else(|| "dns".into());
    if !matches!(le_challenge.as_str(), "dns" | "http") {
        return Err(ApiError::bad_request(
            "le_challenge must be 'dns' or 'http'",
        ));
    }
    let run_user = b.run_user.unwrap_or_else(|| "www-data".into());
    if run_user.is_empty()
        || !run_user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::bad_request(
            "run_user must be a valid unix username",
        ));
    }
    let service_versions_json = b
        .service_versions
        .as_ref()
        .map(|m| serde_json::to_string(m).unwrap_or_else(|_| "{}".into()));
    let use_rabbitmq = b.use_rabbitmq.unwrap_or(false);
    let use_varnish = b.use_varnish.unwrap_or(false);

    let headless_mode = b.headless_mode.unwrap_or_else(|| "none".into());
    if !matches!(
        headless_mode.as_str(),
        "none" | "shared" | "separate" | "split" | "legacy_split"
    ) {
        return Err(ApiError::bad_request(
            "headless_mode must be 'none', 'shared', 'separate', 'split' or 'legacy_split'",
        ));
    }
    if matches!(
        headless_mode.as_str(),
        "separate" | "split" | "legacy_split"
    ) && b.frontend_domain.is_none()
    {
        return Err(ApiError::bad_request(
            "frontend_domain is required for this headless mode",
        ));
    }
    if let Some(cmd) = &b.frontend_cmd {
        if !sk_magento::provision::valid_frontend_cmd(cmd) {
            return Err(ApiError::bad_request(
                "Invalid frontend_cmd: absolute path, no shell operators",
            ));
        }
    }
    if headless_mode == "shared" && use_varnish {
        return Err(ApiError::bad_request("headless shared mode and varnish are mutually exclusive (both use the internal backend port)"));
    }
    let frontend_port = b.frontend_port.unwrap_or(3000);
    let frontend_root = b.frontend_root;
    if frontend_port == 0 && frontend_root.is_none() {
        return Err(ApiError::bad_request(
            "frontend_port=0 (static mode) requires frontend_root",
        ));
    }
    if let Some(fr) = &frontend_root {
        if !fr.starts_with('/') || fr.contains("..") {
            return Err(ApiError::bad_request(
                "frontend_root must be an absolute path",
            ));
        }
    }
    if let Some(src) = &b.magento_source_path {
        if !src.starts_with('/') || src.contains("..") {
            return Err(ApiError::bad_request(
                "magento_source_path must be an absolute path",
            ));
        }
    }
    let magento_routes = b.magento_routes.unwrap_or_default();
    for r in &magento_routes {
        if !r.starts_with('/')
            || r.contains("..")
            || r.chars()
                .any(|c| c.is_whitespace() || c == '{' || c == '}' || c == ';' || c == '"')
        {
            return Err(ApiError::bad_request(format!("Invalid magento route: {r}")));
        }
    }

    // Custom project folder: explicit root_path wins over base_dir/{name}.
    let base_dir = b.base_dir.unwrap_or_else(|| "/var/www/magento".into());
    let root_path = match b.root_path {
        Some(rp) => {
            if !rp.starts_with('/') || rp.contains("..") {
                return Err(ApiError::bad_request("root_path must be an absolute path"));
            }
            rp
        }
        None => format!("{base_dir}/{name}"),
    };
    if !std::path::Path::new(&format!("/usr/bin/php{php_version}")).exists() {
        if b.auto_install_php.unwrap_or(false) {
            let mut extensions: Vec<&str> =
                if let Some(profile) = b.php_extension_profile.as_deref() {
                    sk_web::php::extension_profile(profile)
                        .ok_or_else(|| {
                            ApiError::bad_request(format!(
                                "Unsupported PHP extension profile: {profile}"
                            ))
                        })?
                        .to_vec()
                } else {
                    sk_web::php::MAGENTO_EXTENSIONS.to_vec()
                };
            if let Some(extra) = &b.php_extensions {
                extensions = extra.iter().map(String::as_str).collect();
            }
            let result = sk_web::php::install_version_with_options(
                &php_version,
                sk_web::php::InstallOptions {
                    extensions,
                    set_default: false,
                },
            )
            .await;
            if !result["success"].as_bool().unwrap_or(false) {
                return Err(ApiError::bad_request(format!(
                    "PHP {php_version} auto-install failed: {}",
                    result["error"].as_str().unwrap_or("unknown error")
                )));
            }
        } else if install_magento || b.magento_source_path.is_some() || headless_mode != "none" {
            return Err(ApiError::bad_request(format!(
                "PHP {php_version} is not installed — install it first via /api/v1/php/versions/{php_version}/install or set auto_install_php=true"
            )));
        }
    }

    let db_password = sk_magento::generate_password(20);
    // Magento requires alnum + special char in admin passwords
    let admin_password = format!("{}#A1", sk_magento::generate_password(14));

    let id = store::insert(
        &state.db,
        &name,
        &domain,
        &magento_version,
        &distribution,
        &php_version,
        composer,
        &root_path,
        &db_password,
        &admin_password,
        &ssl_mode,
        use_rabbitmq,
        use_varnish,
        &headless_mode,
        b.frontend_domain.as_deref(),
        frontend_port,
        &magento_routes,
        frontend_root.as_deref(),
        b.le_email.as_deref(),
        &le_challenge,
        &run_user,
        service_versions_json.as_deref(),
        install_magento,
        b.magento_source_path.as_deref(),
    )
    .await?;
    if b.admin_domain.is_some() || b.frontend_cmd.is_some() {
        store::update_web_fields(
            &state.db,
            id,
            None,
            None,
            b.admin_domain.as_deref(),
            None,
            None,
            b.frontend_cmd.as_deref(),
            None,
        )
        .await?;
    }

    let s = store::find(&state.db, id).await?.unwrap();
    provision::spawn(
        state.db.clone(),
        s.clone(),
        provision::ProvisionSpec { base_dir },
    );

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "success": true,
            "message": "Store provisioning started",
            "store": s.to_dict(true), // creator sees initial credentials once
        })),
    ))
}

#[derive(Deserialize, Default)]
struct DeleteBody {
    remove_files: Option<bool>,
}

async fn delete_store(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    body: Option<Json<DeleteBody>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let remove_files = body.and_then(|b| b.0.remove_files).unwrap_or(false);
    let warnings = provision::teardown(&s, remove_files).await;
    store::delete(&state.db, id).await?;
    Ok(Json(
        json!({ "success": true, "message": format!("Store {} removed", s.name), "warnings": warnings }),
    ))
}

#[derive(Deserialize)]
struct LogQuery {
    lines: Option<usize>,
}

async fn store_log(
    State(state): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<i64>,
    Query(q): Query<LogQuery>,
) -> ApiResult<Json<Value>> {
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let lines = q.lines.unwrap_or(60).min(1000);
    Ok(Json(sk_magento::actions::provision_log(&s, lines)))
}

#[derive(Deserialize)]
struct PatchBody {
    /// "none" | "self-signed" — applied on apply-web (cert re-issued with SANs)
    ssl: Option<String>,
    headless_mode: Option<String>,
    frontend_domain: Option<String>,
    admin_domain: Option<String>,
    frontend_port: Option<i64>,
    frontend_root: Option<String>,
    frontend_cmd: Option<String>,
    magento_routes: Option<Vec<String>>,
}

/// PATCH /magento/stores/{id} — update web-facing fields (apply with
/// POST .../apply-web).
async fn patch_store(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<PatchBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    if let Some(m) = &b.headless_mode {
        if !matches!(
            m.as_str(),
            "none" | "shared" | "separate" | "split" | "legacy_split"
        ) {
            return Err(ApiError::bad_request("Invalid headless_mode"));
        }
    }
    if let Some(cmd) = &b.frontend_cmd {
        if !sk_magento::provision::valid_frontend_cmd(cmd) {
            return Err(ApiError::bad_request(
                "Invalid frontend_cmd: absolute path, no shell operators",
            ));
        }
    }
    if let Some(fr) = &b.frontend_root {
        if !fr.starts_with('/') || fr.contains("..") {
            return Err(ApiError::bad_request(
                "frontend_root must be an absolute path",
            ));
        }
    }
    if let Some(ssl) = &b.ssl {
        if !matches!(ssl.as_str(), "none" | "self-signed") {
            return Err(ApiError::bad_request("ssl must be 'none' or 'self-signed'"));
        }
        sqlx::query("UPDATE magento_stores SET ssl_mode = ? WHERE id = ?")
            .bind(ssl)
            .bind(id)
            .execute(&state.db)
            .await
            .map_err(anyhow::Error::from)?;
    }
    store::update_web_fields(
        &state.db,
        id,
        b.headless_mode.as_deref(),
        b.frontend_domain.as_deref(),
        b.admin_domain.as_deref(),
        b.frontend_port,
        b.frontend_root.as_deref(),
        b.frontend_cmd.as_deref(),
        b.magento_routes.as_deref(),
    )
    .await?;
    let s = store::find(&state.db, id).await?.unwrap();
    Ok(Json(json!({ "success": true, "store": s.to_dict(false) })))
}

/// POST /magento/stores/{id}/apply-web — regenerate vhosts + frontend unit
/// from the stored fields and reload nginx.
async fn apply_web(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    match sk_magento::provision::apply_web(&s).await {
        Ok(notes) => Ok(Json(json!({ "success": true, "applied": notes }))),
        Err(e) => Err(ApiError::bad_request(e)),
    }
}

/// POST /magento/stores/{id}/frontend/{start|stop|restart|status|logs}
async fn frontend_action(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, action)): Path<(i64, String)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let result = sk_magento::provision::frontend_ctl(&s, &action).await;
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

/// GET /magento/stores/{id}/vhost — raw nginx config(s) for editing.
async fn get_vhost(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let path = format!("/etc/nginx/sites-available/{}", s.name);
    let content = std::fs::read_to_string(&path)
        .map_err(|e| ApiError::not_found(format!("vhost not found: {e}")))?;
    let fe_path = format!("/etc/nginx/sites-available/{}-frontend", s.name);
    let frontend = std::fs::read_to_string(&fe_path).ok();
    Ok(Json(json!({
        "path": path,
        "content": content,
        "frontend_path": frontend.as_ref().map(|_| fe_path),
        "frontend_content": frontend,
    })))
}

#[derive(Deserialize)]
struct VhostBody {
    content: Option<String>,
    /// edit the {name}-frontend vhost instead of the main one
    frontend: Option<bool>,
}

/// PUT /magento/stores/{id}/vhost — editable nginx config with safety net:
/// backup → write → nginx -t → rollback on failure → reload on success.
async fn put_vhost(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<VhostBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let content = b
        .content
        .ok_or_else(|| ApiError::bad_request("content is required"))?;
    let file = if b.frontend.unwrap_or(false) {
        format!("{}-frontend", s.name)
    } else {
        s.name.clone()
    };
    let result = sk_magento::provision::update_vhost(&file, &content).await;
    let ok = result["success"].as_bool().unwrap_or(false);
    if ok {
        Ok(Json(result))
    } else {
        Err(ApiError::bad_request(
            result["error"]
                .as_str()
                .unwrap_or("vhost update failed")
                .to_string(),
        ))
    }
}

/// POST /magento/stores/{id}/renew-cert — force a Let's Encrypt renewal now.
async fn renew_cert(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    match sk_magento::provision::renew_cert(&s, true, 30).await {
        Ok(v) => Ok(Json(v)),
        Err(e) => Err(ApiError::bad_request(e)),
    }
}

// ==================== DB BACKUPS ====================

async fn list_backups(
    State(state): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    Ok(Json(
        json!({ "backups": sk_magento::backup::list_backups(&s) }),
    ))
}

async fn create_backup(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let result = sk_magento::backup::backup_db(&s).await;
    if result["success"].as_bool().unwrap_or(false) {
        sk_magento::backup::prune(&s, s.backup_retention.max(1) as usize);
    }
    Ok(Json(result))
}

async fn restore_backup(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, filename)): Path<(i64, String)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let result = sk_magento::backup::restore_db(&s, &filename).await;
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

async fn delete_backup(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, filename)): Path<(i64, String)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let result = sk_magento::backup::delete_backup(&s, &filename);
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
struct BackupPolicyBody {
    schedule: Option<String>,
    retention: Option<i64>,
}

async fn set_backup_policy(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<BackupPolicyBody>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    let schedule = b.schedule.unwrap_or_else(|| "none".into());
    if !matches!(schedule.as_str(), "none" | "hourly" | "daily" | "weekly") {
        return Err(ApiError::bad_request(
            "schedule must be none|hourly|daily|weekly",
        ));
    }
    store::set_backup_policy(&state.db, id, &schedule, b.retention.unwrap_or(7)).await?;
    let s = store::find(&state.db, id).await?.unwrap();
    Ok(Json(json!({ "success": true, "store": s.to_dict(false) })))
}

async fn store_health(
    State(state): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    Ok(Json(sk_magento::actions::health(&s).await))
}

async fn run_action(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Path((id, action)): Path<(i64, String)>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let s = store::find(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::not_found("Store not found"))?;
    if s.status != "running" {
        return Err(ApiError::bad_request(format!(
            "Store is not running (status: {})",
            s.status
        )));
    }
    let result = sk_magento::actions::run_action(&s, &action).await;
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
