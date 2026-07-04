//! Port of `backend/app/api/auth.py` (P0 endpoints).
//!
//! Response shapes are contract — see the Flask handlers for the oracle.
//! Includes setup/login/register/profile plus Rust-native TOTP 2FA and SSO setup metadata.

use crate::error::{ApiError, ApiResult};
use crate::extract::{AuthUser, RefreshUser};
use crate::state::SharedState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use hmac::{Hmac, Mac};
use rand::{distributions::Alphanumeric, Rng};
use serde::Deserialize;
use serde_json::{json, Value};
use sha1::Sha1;
use sk_auth::jwt::{create_token, TokenType};
use sk_auth::password::hash_password;
use sk_models::{settings, user};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/setup-status", get(setup_status))
        .route("/register", post(register))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/me", get(me).put(update_me))
        .route("/complete-onboarding", post(complete_onboarding))
        .route("/2fa/status", get(twofa_status))
        .route("/2fa/setup", post(twofa_setup))
        .route("/2fa/setup/confirm", post(twofa_confirm))
        .route("/2fa/disable", post(twofa_disable))
        .route(
            "/2fa/backup-codes/regenerate",
            post(twofa_backup_regenerate),
        )
        .route("/2fa/verify", post(twofa_verify))
}

fn tokens(state: &SharedState, user_id: i64) -> ApiResult<(String, String)> {
    let access = create_token(
        user_id,
        TokenType::Access,
        state.config.jwt_access_ttl_secs,
        &state.config.jwt_secret_key,
        false,
    )
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    let refresh = create_token(
        user_id,
        TokenType::Refresh,
        state.config.jwt_refresh_ttl_secs,
        &state.config.jwt_secret_key,
        false,
    )
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((access, refresh))
}

/// GET /auth/setup-status
async fn setup_status(State(state): State<SharedState>) -> ApiResult<Json<Value>> {
    let user_count = user::count(&state.db).await?;
    let setup_completed = settings::get_bool(&state.db, "setup_completed", false).await?;
    let needs_setup = user_count == 0 || !setup_completed;
    let registration_enabled = if user_count == 0 {
        true
    } else {
        settings::get_bool(&state.db, "registration_enabled", false).await?
    };

    let sso = sk_sso::providers(&state.db)
        .await
        .unwrap_or_else(|_| json!({"providers": [], "password_login_enabled": true}));
    Ok(Json(json!({
        "needs_setup": needs_setup,
        "registration_enabled": registration_enabled,
        "sso_providers": sso["providers"].clone(),
        "password_login_enabled": sso["password_login_enabled"].as_bool().unwrap_or(true),
        "needs_migration": false,
        "migration_info": {
            "pending_count": 0,
            "current_revision": "047_agent_footprint_dirs",
            "head_revision": "047_agent_footprint_dirs",
        },
    })))
}

#[derive(Deserialize)]
struct RegisterBody {
    email: Option<String>,
    username: Option<String>,
    password: Option<String>,
    // invite_token: TODO(P1) — invitation flow
}

/// POST /auth/register
async fn register(
    State(state): State<SharedState>,
    Json(body): Json<RegisterBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let (Some(email), Some(username), Some(password)) = (body.email, body.username, body.password)
    else {
        return Err(ApiError::bad_request("Missing required fields"));
    };

    let is_first_user = user::count(&state.db).await? == 0;
    if !is_first_user {
        let setup_completed = settings::get_bool(&state.db, "setup_completed", false).await?;
        let registration_enabled =
            settings::get_bool(&state.db, "registration_enabled", false).await?;
        if setup_completed && !registration_enabled {
            return Err(ApiError::forbidden("Registration is disabled"));
        }
        if !registration_enabled {
            return Err(ApiError::forbidden("Registration is disabled"));
        }
    }

    if user::email_taken(&state.db, &email, None).await?
        || user::username_taken(&state.db, &username, None).await?
    {
        return Err(ApiError::conflict("This email or username is unavailable"));
    }
    if password.len() < 8 {
        return Err(ApiError::bad_request(
            "Password must be at least 8 characters",
        ));
    }

    let role = if is_first_user {
        user::ROLE_ADMIN
    } else {
        user::ROLE_DEVELOPER
    };
    let user_id = user::insert(
        &state.db,
        &email,
        &username,
        &hash_password(&password),
        role,
    )
    .await?;

    let u = user::find_by_id(&state.db, user_id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    let (access_token, refresh_token) = tokens(&state, user_id)?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "message": "User registered successfully",
            "user": u.to_dict(&state.db).await?,
            "access_token": access_token,
            "refresh_token": refresh_token,
            "is_first_user": is_first_user,
        })),
    ))
}

#[derive(Deserialize)]
struct LoginBody {
    email: Option<String>,
    password: Option<String>,
}

/// POST /auth/login
async fn login(
    State(state): State<SharedState>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Json<Value>> {
    let (Some(login_id), Some(password)) = (body.email, body.password) else {
        return Err(ApiError::bad_request("Missing email/username or password"));
    };

    let maybe_user = user::find_by_login(&state.db, &login_id).await?;

    if let Some(u) = &maybe_user {
        if let Some(remaining) = u.locked_remaining_minutes() {
            return Err(ApiError::new(
                StatusCode::TOO_MANY_REQUESTS,
                format!("Account is locked. Try again in {remaining} minute(s)."),
            ));
        }
    }

    let Some(u) = maybe_user else {
        return Err(ApiError::unauthorized("Invalid username/email or password"));
    };

    // scrypt is CPU-heavy (~100ms) — keep it off the async executor.
    let (u, password_ok) = tokio::task::spawn_blocking(move || {
        let ok = u.check_password(&password);
        (u, ok)
    })
    .await
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if !password_ok {
        user::record_failed_login(&state.db, &u).await?;
        return Err(ApiError::unauthorized("Invalid username/email or password"));
    }

    if !u.is_active() {
        return Err(ApiError::forbidden("Account is deactivated"));
    }

    if u.totp_enabled() {
        let temp_token = create_token(
            u.id,
            TokenType::Access,
            state.config.jwt_access_ttl_secs,
            &state.config.jwt_secret_key,
            true, // 2fa_pending
        )
        .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        return Ok(Json(json!({
            "requires_2fa": true,
            "temp_token": temp_token,
            "message": "Two-factor authentication required",
        })));
    }

    user::record_successful_login(&state.db, u.id).await?;
    let (access_token, refresh_token) = tokens(&state, u.id)?;
    let u = user::find_by_id(&state.db, u.id).await?.unwrap();

    Ok(Json(json!({
        "user": u.to_dict(&state.db).await?,
        "access_token": access_token,
        "refresh_token": refresh_token,
    })))
}

/// POST /auth/refresh
async fn refresh(
    State(state): State<SharedState>,
    RefreshUser(u): RefreshUser,
) -> ApiResult<Json<Value>> {
    if !u.is_active() {
        return Err(ApiError::unauthorized("Invalid user"));
    }
    let access = create_token(
        u.id,
        TokenType::Access,
        state.config.jwt_access_ttl_secs,
        &state.config.jwt_secret_key,
        false,
    )
    .map_err(|e| ApiError::new(StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(json!({ "access_token": access })))
}

/// GET /auth/me
async fn me(State(state): State<SharedState>, AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(json!({ "user": u.to_dict(&state.db).await? })))
}

#[derive(Deserialize)]
struct UpdateMeBody {
    username: Option<String>,
    email: Option<String>,
    password: Option<String>,
    sidebar_config: Option<Value>,
}

/// PUT /auth/me
async fn update_me(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<UpdateMeBody>,
) -> ApiResult<Json<Value>> {
    if let Some(username) = &body.username {
        if user::username_taken(&state.db, username, Some(u.id)).await? {
            return Err(ApiError::conflict("Username already taken"));
        }
        sqlx::query("UPDATE users SET username = ? WHERE id = ?")
            .bind(username)
            .bind(u.id)
            .execute(&state.db)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(email) = &body.email {
        if user::email_taken(&state.db, email, Some(u.id)).await? {
            return Err(ApiError::conflict("Email already registered"));
        }
        sqlx::query("UPDATE users SET email = ? WHERE id = ?")
            .bind(email.to_lowercase())
            .bind(u.id)
            .execute(&state.db)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(password) = &body.password {
        if password.len() < 8 {
            return Err(ApiError::bad_request(
                "Password must be at least 8 characters",
            ));
        }
        sqlx::query("UPDATE users SET password_hash = ? WHERE id = ?")
            .bind(hash_password(password))
            .bind(u.id)
            .execute(&state.db)
            .await
            .map_err(anyhow::Error::from)?;
    }
    if let Some(config) = &body.sidebar_config {
        let preset = config
            .get("preset")
            .and_then(|v| v.as_str())
            .unwrap_or("full");
        const VALID: &[&str] = &["full", "web", "email", "devops", "minimal", "custom"];
        if !VALID.contains(&preset) {
            return Err(ApiError::bad_request(format!(
                "Invalid sidebar preset: {preset}"
            )));
        }
        let hidden = config
            .get("hiddenItems")
            .cloned()
            .unwrap_or_else(|| json!([]));
        if !hidden.is_array() {
            return Err(ApiError::bad_request("hiddenItems must be a list"));
        }
        let stored = json!({ "preset": preset, "hiddenItems": hidden });
        sqlx::query("UPDATE users SET sidebar_config = ? WHERE id = ?")
            .bind(stored.to_string())
            .bind(u.id)
            .execute(&state.db)
            .await
            .map_err(anyhow::Error::from)?;
    }
    sqlx::query("UPDATE users SET updated_at = ? WHERE id = ?")
        .bind(sk_core::time::now_sql())
        .bind(u.id)
        .execute(&state.db)
        .await
        .map_err(anyhow::Error::from)?;

    let u = user::find_by_id(&state.db, u.id).await?.unwrap();
    Ok(Json(json!({ "user": u.to_dict(&state.db).await? })))
}

const ALLOWED_USE_CASES: &[&str] = &["wordpress", "web-apps", "self-hosted", "devops"];

#[derive(Deserialize)]
struct OnboardingBody {
    #[serde(default)]
    use_cases: Vec<String>,
}

/// POST /auth/complete-onboarding
async fn complete_onboarding(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<OnboardingBody>,
) -> ApiResult<Json<Value>> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    let invalid: Vec<_> = body
        .use_cases
        .iter()
        .filter(|c| !ALLOWED_USE_CASES.contains(&c.as_str()))
        .cloned()
        .collect();
    if !invalid.is_empty() {
        return Err(ApiError::bad_request(format!(
            "Invalid use cases: {}",
            invalid.join(", ")
        )));
    }

    settings::set(
        &state.db,
        "onboarding_use_cases",
        &serde_json::to_string(&body.use_cases).unwrap(),
        "json",
        Some(u.id),
    )
    .await?;
    settings::set(&state.db, "setup_completed", "true", "bool", Some(u.id)).await?;

    Ok(Json(
        json!({ "message": "Onboarding completed successfully" }),
    ))
}

const B32: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
fn base32(bytes: &[u8]) -> String {
    let mut out = String::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for b in bytes {
        buffer = (buffer << 8) | (*b as u32);
        bits += 8;
        while bits >= 5 {
            bits -= 5;
            out.push(B32[((buffer >> bits) & 31) as usize] as char);
        }
    }
    if bits > 0 {
        out.push(B32[((buffer << (5 - bits)) & 31) as usize] as char);
    }
    out
}
fn unbase32(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits = 0;
    for c in s.chars().filter(|c| *c != ' ' && *c != '=') {
        let Some(v) = B32
            .iter()
            .position(|b| *b as char == c.to_ascii_uppercase())
            .map(|v| v as u32)
        else {
            continue;
        };
        buffer = (buffer << 5) | v;
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push(((buffer >> bits) & 255) as u8);
        }
    }
    out
}
fn random_secret() -> String {
    let bytes: Vec<u8> = (0..20).map(|_| rand::thread_rng().gen::<u8>()).collect();
    base32(&bytes)
}
fn backup_codes() -> Vec<String> {
    (0..10)
        .map(|_| {
            rand::thread_rng()
                .sample_iter(&Alphanumeric)
                .take(10)
                .map(char::from)
                .collect()
        })
        .collect()
}
fn hotp(secret: &str, counter: u64) -> Option<u32> {
    type HmacSha1 = Hmac<Sha1>;
    let key = unbase32(secret);
    let mut mac = HmacSha1::new_from_slice(&key).ok()?;
    mac.update(&counter.to_be_bytes());
    let bytes = mac.finalize().into_bytes();
    let off = (bytes[19] & 0x0f) as usize;
    let bin = ((bytes[off] & 0x7f) as u32) << 24
        | (bytes[off + 1] as u32) << 16
        | (bytes[off + 2] as u32) << 8
        | bytes[off + 3] as u32;
    Some(bin % 1_000_000)
}
fn verify_totp(secret: &str, code: &str) -> bool {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return false;
    }
    let now = chrono::Utc::now().timestamp() as u64 / 30;
    for c in now.saturating_sub(1)..=now + 1 {
        if hotp(secret, c)
            .map(|v| format!("{v:06}") == code)
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}
fn hash_codes(codes: &[String]) -> String {
    serde_json::to_string(&codes.iter().map(|c| hash_password(c)).collect::<Vec<_>>())
        .unwrap_or_else(|_| "[]".into())
}
async fn twofa_status(AuthUser(u): AuthUser) -> Json<Value> {
    Json(json!({"enabled":u.totp_enabled(),"totp_enabled":u.totp_enabled()}))
}
async fn twofa_setup(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
) -> ApiResult<Json<Value>> {
    let secret = random_secret();
    sqlx::query("UPDATE users SET totp_secret=?, totp_enabled=0, updated_at=? WHERE id=?")
        .bind(&secret)
        .bind(sk_core::time::now_sql())
        .bind(u.id)
        .execute(&state.db)
        .await?;
    let uri = format!(
        "otpauth://totp/ServerKit:{}?secret={}&issuer=ServerKit",
        urlencoding::encode(&u.email),
        secret
    );
    Ok(Json(
        json!({"secret":secret,"otpauth_url":uri,"qr_code_url":uri}),
    ))
}
#[derive(Deserialize)]
struct CodeBody {
    code: Option<String>,
    temp_token: Option<String>,
}
async fn twofa_confirm(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<CodeBody>,
) -> ApiResult<Json<Value>> {
    let u = sk_models::user::find_by_id(&state.db, u.id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    let secret = u
        .totp_secret
        .clone()
        .ok_or_else(|| ApiError::bad_request("2FA setup not started"))?;
    if !verify_totp(&secret, b.code.as_deref().unwrap_or("")) {
        return Err(ApiError::bad_request("Invalid 2FA code"));
    }
    let codes = backup_codes();
    sqlx::query("UPDATE users SET totp_enabled=1, totp_confirmed_at=?, backup_codes=?, updated_at=? WHERE id=?").bind(sk_core::time::now_sql()).bind(hash_codes(&codes)).bind(sk_core::time::now_sql()).bind(u.id).execute(&state.db).await?;
    Ok(Json(json!({"success":true,"backup_codes":codes})))
}
async fn twofa_disable(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<CodeBody>,
) -> ApiResult<Json<Value>> {
    let fresh = sk_models::user::find_by_id(&state.db, u.id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    if fresh.totp_enabled() {
        let sec = fresh.totp_secret.clone().unwrap_or_default();
        if !verify_totp(&sec, b.code.as_deref().unwrap_or("")) {
            return Err(ApiError::bad_request("Invalid 2FA code"));
        }
    }
    sqlx::query("UPDATE users SET totp_enabled=0, totp_secret=NULL, backup_codes=NULL, totp_confirmed_at=NULL, updated_at=? WHERE id=?").bind(sk_core::time::now_sql()).bind(u.id).execute(&state.db).await?;
    Ok(Json(json!({"success":true})))
}
async fn twofa_backup_regenerate(
    State(state): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(b): Json<CodeBody>,
) -> ApiResult<Json<Value>> {
    let fresh = sk_models::user::find_by_id(&state.db, u.id)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    let sec = fresh.totp_secret.clone().unwrap_or_default();
    if !verify_totp(&sec, b.code.as_deref().unwrap_or("")) {
        return Err(ApiError::bad_request("Invalid 2FA code"));
    }
    let codes = backup_codes();
    sqlx::query("UPDATE users SET backup_codes=?, updated_at=? WHERE id=?")
        .bind(hash_codes(&codes))
        .bind(sk_core::time::now_sql())
        .bind(u.id)
        .execute(&state.db)
        .await?;
    Ok(Json(json!({"success":true,"backup_codes":codes})))
}
async fn twofa_verify(
    State(state): State<SharedState>,
    Json(b): Json<CodeBody>,
) -> ApiResult<Json<Value>> {
    let temp = b
        .temp_token
        .as_deref()
        .ok_or_else(|| ApiError::bad_request("temp_token is required"))?;
    let claims =
        sk_auth::jwt::decode_token(temp, &state.config.jwt_secret_key, TokenType::Access, true)
            .map_err(|_| ApiError::unauthorized("Invalid temporary token"))?;
    if claims.twofa_pending != Some(true) {
        return Err(ApiError::bad_request("Token is not pending 2FA"));
    }
    let uid = claims
        .sub
        .as_i64()
        .ok_or_else(|| ApiError::unauthorized("Invalid token subject"))?;
    let u = sk_models::user::find_by_id(&state.db, uid)
        .await?
        .ok_or_else(|| ApiError::not_found("User not found"))?;
    if !u.is_active() || !u.totp_enabled() {
        return Err(ApiError::unauthorized("2FA is not enabled"));
    }
    let sec = u.totp_secret.clone().unwrap_or_default();
    if !verify_totp(&sec, b.code.as_deref().unwrap_or("")) {
        return Err(ApiError::bad_request("Invalid 2FA code"));
    }
    user::record_successful_login(&state.db, uid).await?;
    let (access_token, refresh_token) = tokens(&state, uid)?;
    let u = user::find_by_id(&state.db, uid).await?.unwrap();
    Ok(Json(
        json!({"user": u.to_dict(&state.db).await?, "access_token": access_token, "refresh_token": refresh_token}),
    ))
}
