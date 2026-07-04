//! `users` table access + `User.to_dict()` parity.

use serde_json::{json, Value};
use sk_core::time::{now_sql, parse_sql_datetime, to_isoformat};
use sqlx::{FromRow, SqlitePool};

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_DEVELOPER: &str = "developer";
pub const ROLE_VIEWER: &str = "viewer";

/// Progressive lockout: 5 min, 15 min, 60 min (`User.LOCKOUT_DURATIONS`).
const LOCKOUT_DURATIONS_MIN: [i64; 3] = [5, 15, 60];
const MAX_FAILED_ATTEMPTS: i64 = 5;

#[derive(Debug, Clone, FromRow)]
pub struct User {
    pub id: i64,
    pub email: String,
    pub username: String,
    pub password_hash: Option<String>,
    pub auth_provider: Option<String>,
    pub role: Option<String>,
    pub permissions: Option<String>,
    pub is_active: Option<bool>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
    pub last_login_at: Option<String>,
    pub created_by: Option<i64>,
    pub failed_login_count: Option<i64>,
    pub locked_until: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_enabled: Option<bool>,
    pub backup_codes: Option<String>,
    pub totp_confirmed_at: Option<String>,
    pub sidebar_config: Option<String>,
}

const USER_COLS: &str = "id, email, username, password_hash, auth_provider, role, permissions, \
     is_active, created_at, updated_at, last_login_at, created_by, failed_login_count, \
     locked_until, totp_secret, totp_enabled, backup_codes, totp_confirmed_at, sidebar_config";

impl User {
    pub fn role(&self) -> &str {
        self.role.as_deref().unwrap_or(ROLE_DEVELOPER)
    }

    pub fn is_admin(&self) -> bool {
        self.role() == ROLE_ADMIN
    }

    pub fn is_active(&self) -> bool {
        self.is_active.unwrap_or(true)
    }

    pub fn totp_enabled(&self) -> bool {
        self.totp_enabled.unwrap_or(false)
    }

    /// `User.is_locked` — locked while `locked_until` is in the future.
    pub fn locked_remaining_minutes(&self) -> Option<i64> {
        let until = parse_sql_datetime(self.locked_until.as_deref()?)?;
        let now = sk_core::time::now_naive();
        (until > now).then(|| (until - now).num_seconds() / 60)
    }

    pub fn check_password(&self, password: &str) -> bool {
        match &self.password_hash {
            Some(h) => sk_auth::password::verify_password(h, password),
            None => false,
        }
    }

    /// `User.get_permissions()` — admin always gets the full template; custom
    /// JSON overrides the role template for other roles.
    pub fn resolved_permissions(&self) -> Value {
        if self.is_admin() {
            return crate::permissions::role_template(ROLE_ADMIN);
        }
        if let Some(raw) = &self.permissions {
            if let Ok(v) = serde_json::from_str::<Value>(raw) {
                return v;
            }
        }
        crate::permissions::role_template(self.role())
    }

    fn sidebar(&self) -> Value {
        self.sidebar_config
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_else(|| json!({ "preset": "full", "hiddenItems": [] }))
    }

    /// `User.to_dict()` — exact field/shape parity with the Flask model.
    pub async fn to_dict(&self, pool: &SqlitePool) -> anyhow::Result<Value> {
        let (passkeys,): (i64,) = sqlx::query_as(
            "SELECT COUNT(*) FROM passkey_credentials WHERE user_id = ? AND is_active = 1",
        )
        .fetch_one(pool)
        .await
        .map(|r: (i64,)| r)
        .unwrap_or((0,));
        let _ = &passkeys;

        Ok(json!({
            "id": self.id,
            "email": self.email,
            "username": self.username,
            "role": self.role(),
            "permissions": self.resolved_permissions(),
            "is_active": self.is_active(),
            "totp_enabled": self.totp_enabled(),
            "auth_provider": self.auth_provider.as_deref().unwrap_or("local"),
            "has_password": self.password_hash.is_some(),
            "passkey_enabled": passkeys > 0,
            "sidebar_config": self.sidebar(),
            "created_at": self.created_at.as_deref().map(to_isoformat),
            "updated_at": self.updated_at.as_deref().map(to_isoformat),
            "last_login_at": self.last_login_at.as_deref().map(to_isoformat),
            "created_by": self.created_by,
            "is_admin": self.is_admin(),
        }))
    }
}

pub async fn count(pool: &SqlitePool) -> anyhow::Result<i64> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    Ok(n)
}

pub async fn find_by_id(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<User>> {
    Ok(
        sqlx::query_as::<_, User>(&format!("SELECT {USER_COLS} FROM users WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

/// Login lookup: case-insensitive email OR exact username (matches Flask).
pub async fn find_by_login(pool: &SqlitePool, login: &str) -> anyhow::Result<Option<User>> {
    Ok(sqlx::query_as::<_, User>(&format!(
        "SELECT {USER_COLS} FROM users WHERE lower(email) = lower(?) OR username = ?"
    ))
    .bind(login)
    .bind(login)
    .fetch_optional(pool)
    .await?)
}

pub async fn email_taken(
    pool: &SqlitePool,
    email: &str,
    exclude: Option<i64>,
) -> anyhow::Result<bool> {
    let (n,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM users WHERE lower(email) = lower(?) AND id != ?")
            .bind(email)
            .bind(exclude.unwrap_or(-1))
            .fetch_one(pool)
            .await?;
    Ok(n > 0)
}

pub async fn username_taken(
    pool: &SqlitePool,
    username: &str,
    exclude: Option<i64>,
) -> anyhow::Result<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users WHERE username = ? AND id != ?")
        .bind(username)
        .bind(exclude.unwrap_or(-1))
        .fetch_one(pool)
        .await?;
    Ok(n > 0)
}

pub async fn insert(
    pool: &SqlitePool,
    email: &str,
    username: &str,
    password_hash: &str,
    role: &str,
) -> anyhow::Result<i64> {
    let now = now_sql();
    let res = sqlx::query(
        r#"INSERT INTO users
           (email, username, password_hash, auth_provider, role, is_active,
            created_at, updated_at, failed_login_count, totp_enabled)
           VALUES (?, ?, ?, 'local', ?, 1, ?, ?, 0, 0)"#,
    )
    .bind(email.to_lowercase())
    .bind(username)
    .bind(password_hash)
    .bind(role)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

/// `User.record_failed_login()` — progressive lockout.
pub async fn record_failed_login(pool: &SqlitePool, user: &User) -> anyhow::Result<()> {
    let count = user.failed_login_count.unwrap_or(0) + 1;
    let locked_until = if count >= MAX_FAILED_ATTEMPTS {
        let idx = (((count - MAX_FAILED_ATTEMPTS) / MAX_FAILED_ATTEMPTS) as usize)
            .min(LOCKOUT_DURATIONS_MIN.len() - 1);
        let until =
            sk_core::time::now_naive() + chrono::Duration::minutes(LOCKOUT_DURATIONS_MIN[idx]);
        Some(until.format("%Y-%m-%d %H:%M:%S%.6f").to_string())
    } else {
        None
    };
    sqlx::query(
        "UPDATE users SET failed_login_count = ?, locked_until = ?, updated_at = ? WHERE id = ?",
    )
    .bind(count)
    .bind(locked_until)
    .bind(now_sql())
    .bind(user.id)
    .execute(pool)
    .await?;
    Ok(())
}

/// Successful login: reset lockout, stamp `last_login_at`.
pub async fn record_successful_login(pool: &SqlitePool, user_id: i64) -> anyhow::Result<()> {
    let now = now_sql();
    sqlx::query(
        "UPDATE users SET failed_login_count = 0, locked_until = NULL, last_login_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&now)
    .bind(&now)
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
