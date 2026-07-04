use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{get, patch, post, put};
use axum::{Json, Router};
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use sk_auth::password::hash_password;
use sqlx::Row;

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        Err(ApiError::forbidden("Admin access required"))
    } else {
        Ok(())
    }
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
pub async fn ensure_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"CREATE TABLE IF NOT EXISTS sk_admin_invitations(id TEXT PRIMARY KEY,email TEXT NOT NULL,role TEXT NOT NULL,token TEXT NOT NULL UNIQUE,status TEXT NOT NULL DEFAULT 'pending',expires_at TEXT NOT NULL,created_at TEXT NOT NULL,updated_at TEXT NOT NULL,created_by INTEGER);CREATE INDEX IF NOT EXISTS idx_sk_admin_inv_token ON sk_admin_invitations(token);"#).execute(pool).await?;
    Ok(())
}
pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/settings", get(settings).put(update_settings))
        .route("/settings/domain-detection", get(domain_detection))
        .route("/settings/canonical-domain", put(canonical_domain))
        .route("/settings/{id}", get(setting).put(update_setting))
        .route("/stats", get(stats))
        .route("/users", get(users).post(create_user))
        .route(
            "/users/{id}",
            get(get_user).put(update_user).delete(delete_user),
        )
        .route(
            "/users/{id}/permissions",
            get(user_permissions).put(update_user_permissions),
        )
        .route("/users/{id}/permissions/reset", post(reset_permissions))
        .route("/permissions/templates", get(permission_templates))
        .route("/audit-logs", get(audit_logs))
        .route("/audit-logs/actions", get(audit_actions))
        .route("/activity/feed", get(activity_feed))
        .route("/activity/summary", get(activity_summary))
        .route("/invitations", post(create_invitation))
        .route("/invitations/validate/{id}", get(validate_invitation))
        .route("/invitations/resend/{id}", post(resend_invitation))
        .route(
            "/invitations/{id}",
            get(get_invitation).delete(delete_invitation),
        )
        .route("/sites-https/status", get(sites_https_status))
        .route("/sites-https/setup", post(sites_https_setup))
        .route("/sites-https/base-domains", post(add_base_domain))
        .route(
            "/sites-https/base-domains/{id}",
            patch(update_base_domain).delete(remove_base_domain),
        )
        .route(
            "/sites-https/base-domains/{id}/default",
            post(default_base_domain),
        )
}
async fn settings(State(st): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let rows = sqlx::query(
        "SELECT key,value,value_type,updated_at,updated_by FROM system_settings ORDER BY key",
    )
    .fetch_all(&st.db)
    .await
    .map_err(ApiError::from)?;
    Ok(Json(
        json!({"settings":rows.iter().map(|r|json!({"key":r.get::<String,_>("key"),"value":r.try_get::<Option<String>,_>("value").ok().flatten(),"value_type":r.try_get::<Option<String>,_>("value_type").ok().flatten(),"updated_at":r.try_get::<Option<String>,_>("updated_at").ok().flatten(),"updated_by":r.try_get::<Option<i64>,_>("updated_by").ok().flatten()})).collect::<Vec<_>>() }),
    ))
}
async fn update_settings(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    if let Some(o) = body.as_object() {
        for (k, v) in o {
            let (val, ty) = match v {
                Value::Bool(b) => (b.to_string(), "bool"),
                Value::Number(n) => (n.to_string(), "number"),
                Value::String(s) => (s.clone(), "string"),
                _ => (v.to_string(), "json"),
            };
            sk_models::settings::set(&st.db, k, &val, ty, Some(u.id))
                .await
                .map_err(ApiError::from)?;
        }
    }
    settings(State(st), AuthUser(u)).await
}
async fn setting(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(key): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let r = sqlx::query("SELECT key,value,value_type,updated_at FROM system_settings WHERE key=?")
        .bind(&key)
        .fetch_optional(&st.db)
        .await
        .map_err(ApiError::from)?;
    match r {
        Some(r) => Ok(Json(
            json!({"setting":{"key":r.get::<String,_>("key"),"value":r.try_get::<Option<String>,_>("value").ok().flatten(),"value_type":r.try_get::<Option<String>,_>("value_type").ok().flatten(),"updated_at":r.try_get::<Option<String>,_>("updated_at").ok().flatten()}}),
        )),
        None => Err(ApiError::not_found("Setting not found")),
    }
}
async fn update_setting(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(key): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let v = body.get("value").cloned().unwrap_or(body);
    let (val, ty) = match &v {
        Value::Bool(b) => (b.to_string(), "bool"),
        Value::Number(n) => (n.to_string(), "number"),
        Value::String(s) => (s.clone(), "string"),
        _ => (v.to_string(), "json"),
    };
    sk_models::settings::set(&st.db, &key, &val, ty, Some(u.id))
        .await
        .map_err(ApiError::from)?;
    setting(State(st), AuthUser(u), Path(key)).await
}
async fn domain_detection(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let hostname = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(Json(
        json!({"success":true,"hostname":hostname,"detected_domain":std::env::var("SK_BASE_DOMAIN").ok(),"server_ip":local_ip()}),
    ))
}
fn local_ip() -> Option<String> {
    std::process::Command::new("hostname")
        .arg("-I")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .and_then(|s| s.split_whitespace().next().map(str::to_string))
}
async fn canonical_domain(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    if let Some(d) = opt(&b, "domain") {
        sk_models::settings::set(&st.db, "canonical_domain", d, "string", Some(u.id))
            .await
            .map_err(ApiError::from)?;
    }
    if let Some(h) = b.get("https_enabled").and_then(Value::as_bool) {
        sk_models::settings::set(&st.db, "https_enabled", &h.to_string(), "bool", Some(u.id))
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(json!({"success":true})))
}
async fn stats(State(st): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    let apps: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_apps")
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    let jobs = sk_jobs::job_stats(&st.db)
        .await
        .unwrap_or_else(|_| json!({}));
    Ok(Json(
        json!({"users":users,"apps":apps,"jobs":jobs,"generated_at":now()}),
    ))
}
async fn users(State(st): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let rows=sqlx::query_as::<_,sk_models::user::User>("SELECT id, email, username, password_hash, auth_provider, role, permissions, is_active, created_at, updated_at, last_login_at, created_by, failed_login_count, locked_until, totp_secret, totp_enabled, backup_codes, totp_confirmed_at, sidebar_config FROM users ORDER BY id").fetch_all(&st.db).await.map_err(ApiError::from)?;
    let mut out = Vec::new();
    for x in rows {
        out.push(x.to_dict(&st.db).await.map_err(ApiError::from)?);
    }
    Ok(Json(json!({"users":out})))
}
async fn get_user(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let usr = sk_models::user::find_by_id(&st.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    Ok(Json(
        json!({"user":usr.to_dict(&st.db).await.map_err(ApiError::from)?}),
    ))
}
async fn create_user(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let email = s(&b, "email", "");
    let username = s(&b, "username", email.split('@').next().unwrap_or("user"));
    let password = s(&b, "password", "");
    if email.is_empty() || password.len() < 8 {
        return Err(ApiError::bad_request("email and password>=8 required"));
    }
    let role = s(&b, "role", sk_models::user::ROLE_DEVELOPER);
    let id = sk_models::user::insert(&st.db, email, username, &hash_password(password), role)
        .await
        .map_err(ApiError::from)?;
    get_user(State(st), AuthUser(u), Path(id)).await
}
async fn update_user(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sqlx::query("UPDATE users SET email=COALESCE(?,email), username=COALESCE(?,username), role=COALESCE(?,role), is_active=COALESCE(?,is_active), updated_at=? WHERE id=?").bind(opt(&b,"email")).bind(opt(&b,"username")).bind(opt(&b,"role")).bind(b.get("is_active").and_then(Value::as_bool).map(|v|if v{1}else{0})).bind(sk_core::time::now_sql()).bind(id).execute(&st.db).await.map_err(ApiError::from)?;
    if let Some(p) = opt(&b, "password") {
        sqlx::query("UPDATE users SET password_hash=?, updated_at=? WHERE id=?")
            .bind(hash_password(p))
            .bind(sk_core::time::now_sql())
            .bind(id)
            .execute(&st.db)
            .await
            .map_err(ApiError::from)?;
    }
    get_user(State(st), AuthUser(u), Path(id)).await
}
async fn delete_user(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    if id == u.id {
        return Err(ApiError::bad_request("Cannot delete current user"));
    }
    let r = sqlx::query("DELETE FROM users WHERE id=?")
        .bind(id)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({"success":true,"deleted":r.rows_affected()})))
}
async fn user_permissions(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let usr = sk_models::user::find_by_id(&st.db, id)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    require_admin(&u)?;
    Ok(Json(json!({"permissions":usr.resolved_permissions()})))
}
async fn update_user_permissions(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sqlx::query("UPDATE users SET permissions=?, updated_at=? WHERE id=?")
        .bind(b.get("permissions").cloned().unwrap_or(b).to_string())
        .bind(sk_core::time::now_sql())
        .bind(id)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    user_permissions(State(st), AuthUser(u), Path(id)).await
}
async fn reset_permissions(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sqlx::query("UPDATE users SET permissions=NULL, updated_at=? WHERE id=?")
        .bind(sk_core::time::now_sql())
        .bind(id)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    user_permissions(State(st), AuthUser(u), Path(id)).await
}
async fn permission_templates(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"templates":{"admin":sk_models::permissions::role_template("admin"),"developer":sk_models::permissions::role_template("developer"),"viewer":sk_models::permissions::role_template("viewer")}}),
    ))
}
async fn audit_logs(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let rows = sqlx::query("SELECT * FROM sk_api_analytics ORDER BY created_at DESC LIMIT 200")
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    Ok(Json(
        json!({"logs":rows.iter().map(|r|json!({"method":r.get::<String,_>("method"),"path":r.get::<String,_>("path"),"status":r.get::<i64,_>("status"),"latency_ms":r.try_get::<Option<i64>,_>("latency_ms").ok().flatten(),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    ))
}
async fn audit_actions(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(
        json!({"actions":["GET","POST","PUT","PATCH","DELETE"]}),
    ))
}
async fn activity_feed(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let rows = sqlx::query("SELECT * FROM sk_telemetry_events ORDER BY created_at DESC LIMIT 100")
        .fetch_all(&st.db)
        .await
        .unwrap_or_default();
    Ok(Json(
        json!({"items":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"type":r.get::<String,_>("event_type"),"severity":r.get::<String,_>("severity"),"message":r.try_get::<Option<String>,_>("message").ok().flatten(),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>(),"total":rows.len()}),
    ))
}
async fn activity_summary(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_telemetry_events")
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    let api: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_api_analytics")
        .fetch_one(&st.db)
        .await
        .unwrap_or(0);
    Ok(Json(
        json!({"events":events,"api_requests":api,"generated_at":now()}),
    ))
}
async fn create_invitation(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let id = uuid::Uuid::new_v4().to_string();
    let t = token();
    let ts = now();
    let exp = (Utc::now() + Duration::days(7)).to_rfc3339();
    sqlx::query("INSERT INTO sk_admin_invitations(id,email,role,token,status,expires_at,created_at,updated_at,created_by) VALUES(?,?,?,?,?,?,?,?,?)").bind(&id).bind(s(&b,"email","")).bind(s(&b,"role","developer")).bind(&t).bind("pending").bind(&exp).bind(&ts).bind(&ts).bind(u.id).execute(&st.db).await.map_err(ApiError::from)?;
    Ok(Json(
        json!({"success":true,"invitation":{"id":id,"token":t,"expires_at":exp}}),
    ))
}
async fn get_invitation(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    inv_by(&st.db, "id", &id).await
}
async fn validate_invitation(
    State(st): State<SharedState>,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    match inv_by(&st.db, "token", &id).await {
        Ok(v) => Ok(v),
        Err(_) => inv_by(&st.db, "id", &id).await,
    }
}
async fn inv_by(db: &sqlx::SqlitePool, col: &str, val: &str) -> ApiResult<Json<Value>> {
    let sql = format!("SELECT * FROM sk_admin_invitations WHERE {col}=?");
    let r = sqlx::query(&sql)
        .bind(val)
        .fetch_optional(db)
        .await
        .map_err(ApiError::from)?
        .ok_or_else(|| ApiError::not_found("Invitation not found"))?;
    Ok(Json(
        json!({"invitation":{"id":r.get::<String,_>("id"),"email":r.get::<String,_>("email"),"role":r.get::<String,_>("role"),"token":r.get::<String,_>("token"),"status":r.get::<String,_>("status"),"expires_at":r.get::<String,_>("expires_at")}}),
    ))
}
async fn resend_invitation(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let t = token();
    sqlx::query("UPDATE sk_admin_invitations SET token=?, updated_at=? WHERE id=?")
        .bind(&t)
        .bind(now())
        .bind(&id)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    get_invitation(State(st), AuthUser(u), Path(id)).await
}
async fn delete_invitation(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(id): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let r = sqlx::query("DELETE FROM sk_admin_invitations WHERE id=?")
        .bind(id)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({"success":true,"deleted":r.rows_affected()})))
}
async fn sites_https_status(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sk_web::domains::ensure_schema(&st.db)
        .await
        .map_err(ApiError::from)?;
    let bases = sk_web::domains::base_domains(&st.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(
        json!({"success":true,"https_enabled":sk_models::settings::get_bool(&st.db,"https_enabled",false).await.unwrap_or(false),"base_domain":bases["base_domains"].as_array().and_then(|a|a.first()).and_then(|b|b["domain"].as_str()),"providers":[],"bases":bases["base_domains"].clone()}),
    ))
}
async fn sites_https_setup(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    if let Some(base) = opt(&b, "base") {
        sk_models::settings::set(&st.db, "canonical_domain", base, "string", Some(u.id))
            .await
            .map_err(ApiError::from)?;
    }
    Ok(Json(
        json!({"success":false,"code":"HTTPS_PROVIDER_SETUP_NOT_CONFIGURED","error":"Use DNS/email provider and SSL certificate routes to configure managed HTTPS"}),
    ))
}
async fn add_base_domain(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sk_web::domains::ensure_schema(&st.db)
        .await
        .map_err(ApiError::from)?;
    let domain = s(&b, "domain", "");
    if domain.is_empty() {
        return Err(ApiError::bad_request("domain is required"));
    }
    let ts = now();
    sqlx::query("INSERT INTO sk_base_domains(domain,dns_mode,is_default,created_at,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(domain) DO UPDATE SET dns_mode=excluded.dns_mode,updated_at=excluded.updated_at").bind(domain).bind(s(&b,"dns_mode","wildcard")).bind(if b.get("make_default").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(&ts).bind(&ts).execute(&st.db).await.map_err(ApiError::from)?;
    Ok(Json(
        sk_web::domains::base_domains(&st.db)
            .await
            .map_err(ApiError::from)?,
    ))
}
async fn update_base_domain(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(domain): Path<String>,
    Json(b): Json<Value>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    sqlx::query(
        "UPDATE sk_base_domains SET dns_mode=COALESCE(?,dns_mode), updated_at=? WHERE domain=?",
    )
    .bind(opt(&b, "dns_mode"))
    .bind(now())
    .bind(domain)
    .execute(&st.db)
    .await
    .map_err(ApiError::from)?;
    Ok(Json(
        sk_web::domains::base_domains(&st.db)
            .await
            .map_err(ApiError::from)?,
    ))
}
async fn remove_base_domain(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(domain): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let r = sqlx::query("DELETE FROM sk_base_domains WHERE domain=?")
        .bind(domain)
        .execute(&st.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({"success":true,"deleted":r.rows_affected()})))
}
async fn default_base_domain(
    State(st): State<SharedState>,
    AuthUser(u): AuthUser,
    Path(domain): Path<String>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut tx = st.db.begin().await.map_err(ApiError::from)?;
    sqlx::query("UPDATE sk_base_domains SET is_default=0")
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    sqlx::query("UPDATE sk_base_domains SET is_default=1, updated_at=? WHERE domain=?")
        .bind(now())
        .bind(domain)
        .execute(&mut *tx)
        .await
        .map_err(ApiError::from)?;
    tx.commit().await.map_err(ApiError::from)?;
    Ok(Json(
        sk_web::domains::base_domains(&st.db)
            .await
            .map_err(ApiError::from)?,
    ))
}
