//! Store registry — `magento_stores` table (fork-owned, created on demand;
//! not part of the upstream 124-table baseline).

use serde_json::{json, Value};
use sqlx::SqlitePool;

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS magento_stores (
            id INTEGER PRIMARY KEY,
            name VARCHAR(64) UNIQUE NOT NULL,
            domain VARCHAR(255) NOT NULL,
            magento_version VARCHAR(32) NOT NULL,
            distribution VARCHAR(20) NOT NULL DEFAULT 'mage-os',
            php_version VARCHAR(8) NOT NULL,
            composer_version VARCHAR(16) NOT NULL,
            root_path VARCHAR(255) NOT NULL,
            db_password VARCHAR(128),
            admin_password VARCHAR(128),
            admin_url VARCHAR(255),
            status VARCHAR(32) NOT NULL DEFAULT 'provisioning',
            status_detail TEXT,
            ssl_mode VARCHAR(20) NOT NULL DEFAULT 'none',
            use_rabbitmq INTEGER NOT NULL DEFAULT 0,
            use_varnish INTEGER NOT NULL DEFAULT 0,
            headless_mode VARCHAR(20) NOT NULL DEFAULT 'none',
            api_domain VARCHAR(255),
            split_route_mode VARCHAR(20) NOT NULL DEFAULT 'api_only',
            frontend_domain VARCHAR(255),
            frontend_port INTEGER NOT NULL DEFAULT 3000,
            magento_routes TEXT,
            frontend_root VARCHAR(255),
            admin_domain VARCHAR(255),
            frontend_cmd TEXT,
            nginx_extras TEXT,
            le_email VARCHAR(255),
            le_challenge VARCHAR(10) NOT NULL DEFAULT 'dns',
            run_user VARCHAR(32) NOT NULL DEFAULT 'www-data',
            service_versions TEXT,
            install_magento INTEGER NOT NULL DEFAULT 0,
            magento_source_path VARCHAR(255),
            backup_schedule VARCHAR(10) NOT NULL DEFAULT 'none',
            backup_retention INTEGER NOT NULL DEFAULT 7,
            created_at DATETIME,
            updated_at DATETIME
        )"#,
    )
    .execute(pool)
    .await?;
    // Additive columns for tables created before these options existed.
    // SQLite has no ADD COLUMN IF NOT EXISTS — ignore the duplicate error.
    for ddl in [
        "ALTER TABLE magento_stores ADD COLUMN ssl_mode VARCHAR(20) NOT NULL DEFAULT 'none'",
        "ALTER TABLE magento_stores ADD COLUMN use_rabbitmq INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE magento_stores ADD COLUMN use_varnish INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE magento_stores ADD COLUMN headless_mode VARCHAR(20) NOT NULL DEFAULT 'none'",
        "ALTER TABLE magento_stores ADD COLUMN api_domain VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN split_route_mode VARCHAR(20) NOT NULL DEFAULT 'api_only'",
        "ALTER TABLE magento_stores ADD COLUMN frontend_domain VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN frontend_port INTEGER NOT NULL DEFAULT 3000",
        "ALTER TABLE magento_stores ADD COLUMN magento_routes TEXT",
        "ALTER TABLE magento_stores ADD COLUMN frontend_root VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN admin_domain VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN frontend_cmd TEXT",
        "ALTER TABLE magento_stores ADD COLUMN nginx_extras TEXT",
        "ALTER TABLE magento_stores ADD COLUMN le_email VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN le_challenge VARCHAR(10) NOT NULL DEFAULT 'dns'",
        "ALTER TABLE magento_stores ADD COLUMN run_user VARCHAR(32) NOT NULL DEFAULT 'www-data'",
        "ALTER TABLE magento_stores ADD COLUMN service_versions TEXT",
        "ALTER TABLE magento_stores ADD COLUMN install_magento INTEGER NOT NULL DEFAULT 0",
        "ALTER TABLE magento_stores ADD COLUMN magento_source_path VARCHAR(255)",
        "ALTER TABLE magento_stores ADD COLUMN backup_schedule VARCHAR(10) NOT NULL DEFAULT 'none'",
        "ALTER TABLE magento_stores ADD COLUMN backup_retention INTEGER NOT NULL DEFAULT 7",
    ] {
        let _ = sqlx::query(ddl).execute(pool).await;
    }
    Ok(())
}

/// One-time migration: encrypt any store password rows that are still
/// plaintext (written before at-rest encryption existed). Idempotent — a
/// value that already decrypts is left untouched.
pub async fn encrypt_existing(pool: &SqlitePool) -> anyhow::Result<usize> {
    let rows: Vec<(i64, Option<String>, Option<String>)> =
        sqlx::query_as("SELECT id, db_password, admin_password FROM magento_stores")
            .fetch_all(pool)
            .await?;
    let mut migrated = 0usize;
    for (id, db, admin) in rows {
        let mut sets: Vec<&str> = Vec::new();
        let mut new_db = None;
        let mut new_admin = None;
        if let Some(v) = &db {
            if sk_core::crypto::decrypt(v).is_none() {
                new_db = Some(sk_core::crypto::encrypt(v));
            }
        }
        if let Some(v) = &admin {
            if sk_core::crypto::decrypt(v).is_none() {
                new_admin = Some(sk_core::crypto::encrypt(v));
            }
        }
        if new_db.is_some() {
            sets.push("db_password = ?");
        }
        if new_admin.is_some() {
            sets.push("admin_password = ?");
        }
        if sets.is_empty() {
            continue;
        }
        let sql = format!("UPDATE magento_stores SET {} WHERE id = ?", sets.join(", "));
        let mut q = sqlx::query(&sql);
        if let Some(v) = new_db {
            q = q.bind(v);
        }
        if let Some(v) = new_admin {
            q = q.bind(v);
        }
        q.bind(id).execute(pool).await?;
        migrated += 1;
    }
    if migrated > 0 {
        tracing::info!(
            count = migrated,
            "encrypted plaintext store passwords at rest"
        );
    }
    Ok(migrated)
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Store {
    pub id: i64,
    pub name: String,
    pub domain: String,
    pub magento_version: String,
    pub distribution: String,
    pub php_version: String,
    pub composer_version: String,
    pub root_path: String,
    pub db_password: Option<String>,
    pub admin_password: Option<String>,
    pub admin_url: Option<String>,
    pub status: String,
    pub status_detail: Option<String>,
    pub ssl_mode: String,
    pub use_rabbitmq: bool,
    pub use_varnish: bool,
    pub headless_mode: String,
    pub api_domain: Option<String>,
    pub split_route_mode: String,
    pub frontend_domain: Option<String>,
    pub frontend_port: i64,
    pub magento_routes: Option<String>,
    pub frontend_root: Option<String>,
    pub admin_domain: Option<String>,
    pub frontend_cmd: Option<String>,
    pub nginx_extras: Option<String>,
    pub le_email: Option<String>,
    pub le_challenge: String,
    pub run_user: String,
    pub service_versions: Option<String>,
    pub install_magento: bool,
    pub magento_source_path: Option<String>,
    pub backup_schedule: String,
    pub backup_retention: i64,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

impl Store {
    /// Per-store PHP-FPM pool name (also the socket suffix).
    pub fn fpm_pool(&self) -> String {
        self.name.replace('-', "_")
    }

    /// Resolved container image tags (overrides merged over defaults).
    pub fn service_versions_map(&self) -> serde_json::Value {
        let defaults = crate::compose::service_versions_for(&self.magento_version);
        let mut map = defaults.as_object().cloned().unwrap_or_default();
        if let Some(raw) = &self.service_versions {
            if let Ok(serde_json::Value::Object(over)) =
                serde_json::from_str::<serde_json::Value>(raw)
            {
                for (k, v) in over {
                    if v.is_string() && !v.as_str().unwrap_or("").is_empty() {
                        map.insert(k, v);
                    }
                }
            }
        }
        serde_json::Value::Object(map)
    }

    /// Decrypted DB root/user password (Fernet at rest; plaintext-tolerant
    /// for rows written before encryption).
    pub fn db_password_plain(&self) -> Option<String> {
        self.db_password
            .as_deref()
            .map(sk_core::crypto::decrypt_or_plain)
    }

    /// Decrypted Magento admin password.
    pub fn admin_password_plain(&self) -> Option<String> {
        self.admin_password
            .as_deref()
            .map(sk_core::crypto::decrypt_or_plain)
    }

    pub fn magento_src(&self) -> String {
        self.magento_source_path
            .clone()
            .unwrap_or_else(|| format!("{}/src", self.root_path))
    }

    pub fn nginx_extras_value(&self) -> serde_json::Value {
        self.nginx_extras
            .as_deref()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    }

    /// Custom path prefixes routed to Magento in shared headless mode.
    pub fn custom_routes(&self) -> Vec<String> {
        self.magento_routes
            .as_deref()
            .and_then(|s| serde_json::from_str::<Vec<String>>(s).ok())
            .unwrap_or_default()
    }

    /// API shape. Secrets are masked; `reveal` includes them (admin-only path).
    pub fn to_dict(&self, reveal: bool) -> Value {
        json!({
            "id": self.id,
            "name": self.name,
            "domain": self.domain,
            "magento_version": self.magento_version,
            "distribution": self.distribution,
            "php_version": self.php_version,
            "composer_version": self.composer_version,
            "root_path": self.root_path,
            // reveal decrypts; masked otherwise. Stored values are Fernet tokens.
            "db_password": if reveal { json!(self.db_password_plain()) } else { json!(self.db_password.as_ref().map(|_| "********")) },
            "admin_password": if reveal { json!(self.admin_password_plain()) } else { json!(self.admin_password.as_ref().map(|_| "********")) },
            "admin_url": self.admin_url,
            "status": self.status,
            "status_detail": self.status_detail,
            "ssl_mode": self.ssl_mode,
            "ssl_cert_days": if self.ssl_mode != "none" {
                serde_json::json!(crate::provision::cert_days_remaining(
                    &format!("/etc/ssl/serverkit/{}.crt", self.name)
                ))
            } else {
                serde_json::Value::Null
            },
            "use_rabbitmq": self.use_rabbitmq,
            "use_varnish": self.use_varnish,
            "headless_mode": self.headless_mode,
            "api_domain": self.api_domain,
            "split_route_mode": self.split_route_mode,
            "frontend_domain": self.frontend_domain,
            "frontend_port": self.frontend_port,
            "frontend_root": self.frontend_root,
            "admin_domain": self.admin_domain,
            "frontend_cmd": self.frontend_cmd,
            "nginx_extras": self.nginx_extras_value(),
            "le_email": self.le_email,
            "le_challenge": self.le_challenge,
            "run_user": self.run_user,
            "service_versions": self.service_versions_map(),
            "install_magento": self.install_magento,
            "magento_source_path": self.magento_source_path,
            "backup_schedule": self.backup_schedule,
            "backup_retention": self.backup_retention,
            "magento_routes": self.custom_routes(),
            "ports": {
                "db": crate::port_base(self.id),
                "search": crate::port_base(self.id) + 1,
                "redis": crate::port_base(self.id) + 2,
                "amqp": crate::port_base(self.id) + 3,
                "smtp": crate::port_base(self.id) + 4,
                "mail_ui": crate::port_base(self.id) + 5,
                "varnish": crate::port_base(self.id) + 6,
                "backend": crate::port_base(self.id) + 7,
            },
            "created_at": self.created_at,
            "updated_at": self.updated_at,
        })
    }
}

const COLS: &str = "id, name, domain, magento_version, distribution, php_version, \
    composer_version, root_path, db_password, admin_password, admin_url, status, \
    status_detail, ssl_mode, use_rabbitmq, use_varnish, headless_mode, api_domain, split_route_mode, \
    frontend_domain, frontend_port, magento_routes, frontend_root, admin_domain, frontend_cmd, \
    nginx_extras, le_email, le_challenge, run_user, service_versions, install_magento, magento_source_path, \
    backup_schedule, backup_retention, created_at, updated_at";

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Vec<Store>> {
    Ok(
        sqlx::query_as(&format!("SELECT {COLS} FROM magento_stores ORDER BY id"))
            .fetch_all(pool)
            .await?,
    )
}

pub async fn find(pool: &SqlitePool, id: i64) -> anyhow::Result<Option<Store>> {
    Ok(
        sqlx::query_as(&format!("SELECT {COLS} FROM magento_stores WHERE id = ?"))
            .bind(id)
            .fetch_optional(pool)
            .await?,
    )
}

pub async fn name_taken(pool: &SqlitePool, name: &str) -> anyhow::Result<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM magento_stores WHERE name = ?")
        .bind(name)
        .fetch_one(pool)
        .await?;
    Ok(n > 0)
}

#[allow(clippy::too_many_arguments)]
pub async fn insert(
    pool: &SqlitePool,
    name: &str,
    domain: &str,
    magento_version: &str,
    distribution: &str,
    php_version: &str,
    composer_version: &str,
    root_path: &str,
    db_password: &str,
    admin_password: &str,
    ssl_mode: &str,
    use_rabbitmq: bool,
    use_varnish: bool,
    headless_mode: &str,
    api_domain: Option<&str>,
    split_route_mode: &str,
    frontend_domain: Option<&str>,
    frontend_port: i64,
    magento_routes: &[String],
    frontend_root: Option<&str>,
    nginx_extras: Option<&str>,
    le_email: Option<&str>,
    le_challenge: &str,
    run_user: &str,
    service_versions: Option<&str>,
    install_magento: bool,
    magento_source_path: Option<&str>,
) -> anyhow::Result<i64> {
    let now = sk_core::time::now_sql();
    let res = sqlx::query(
        r#"INSERT INTO magento_stores
           (name, domain, magento_version, distribution, php_version, composer_version,
            root_path, db_password, admin_password, status, status_detail, ssl_mode,
            use_rabbitmq, use_varnish, headless_mode, api_domain, split_route_mode,
            frontend_domain, frontend_port, magento_routes, frontend_root, nginx_extras, le_email, le_challenge,
            run_user, service_versions, install_magento, magento_source_path, created_at, updated_at)
           VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'provisioning', 'queued', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(name)
    .bind(domain)
    .bind(magento_version)
    .bind(distribution)
    .bind(php_version)
    .bind(composer_version)
    .bind(root_path)
    .bind(sk_core::crypto::encrypt(db_password)) // encrypted at rest (Fernet)
    .bind(sk_core::crypto::encrypt(admin_password))
    .bind(ssl_mode)
    .bind(use_rabbitmq)
    .bind(use_varnish)
    .bind(headless_mode)
    .bind(api_domain)
    .bind(split_route_mode)
    .bind(frontend_domain)
    .bind(frontend_port)
    .bind(serde_json::to_string(magento_routes).unwrap_or_else(|_| "[]".into()))
    .bind(frontend_root)
    .bind(nginx_extras)
    .bind(le_email)
    .bind(le_challenge)
    .bind(run_user)
    .bind(service_versions)
    .bind(if install_magento { 1 } else { 0 })
    .bind(magento_source_path)
    .bind(&now)
    .bind(&now)
    .execute(pool)
    .await?;
    Ok(res.last_insert_rowid())
}

pub async fn set_status(pool: &SqlitePool, id: i64, status: &str, detail: &str) {
    let _ = sqlx::query(
        "UPDATE magento_stores SET status = ?, status_detail = ?, updated_at = ? WHERE id = ?",
    )
    .bind(status)
    .bind(detail)
    .bind(sk_core::time::now_sql())
    .bind(id)
    .execute(pool)
    .await;
    tracing::info!(store = id, status, detail, "magento store status");
}

pub async fn set_admin_url(pool: &SqlitePool, id: i64, url: &str) {
    let _ = sqlx::query("UPDATE magento_stores SET admin_url = ?, updated_at = ? WHERE id = ?")
        .bind(url)
        .bind(sk_core::time::now_sql())
        .bind(id)
        .execute(pool)
        .await;
}

/// PATCH support: update the headless/web-facing fields of a store.
#[allow(clippy::too_many_arguments)]
pub async fn update_run_user(pool: &SqlitePool, id: i64, run_user: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE magento_stores SET run_user = ?, updated_at = ? WHERE id = ?")
        .bind(run_user)
        .bind(sk_core::time::now_sql())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub async fn update_web_fields(
    pool: &SqlitePool,
    id: i64,
    headless_mode: Option<&str>,
    api_domain: Option<&str>,
    split_route_mode: Option<&str>,
    frontend_domain: Option<&str>,
    admin_domain: Option<&str>,
    frontend_port: Option<i64>,
    frontend_root: Option<&str>,
    frontend_cmd: Option<&str>,
    nginx_extras: Option<&str>,
    magento_routes: Option<&[String]>,
) -> anyhow::Result<()> {
    // build dynamically but with bind params only
    let mut sets: Vec<&str> = Vec::new();
    let mut args: Vec<String> = Vec::new();
    macro_rules! push {
        ($col:literal, $val:expr) => {
            if let Some(v) = $val {
                sets.push(concat!($col, " = ?"));
                args.push(v.to_string());
            }
        };
    }
    push!("headless_mode", headless_mode);
    push!("api_domain", api_domain);
    push!("split_route_mode", split_route_mode);
    push!("frontend_domain", frontend_domain);
    push!("admin_domain", admin_domain);
    push!(
        "frontend_port",
        frontend_port.map(|p| p.to_string()).as_deref()
    );
    push!("frontend_root", frontend_root);
    push!("frontend_cmd", frontend_cmd);
    push!("nginx_extras", nginx_extras);
    push!(
        "magento_routes",
        magento_routes
            .map(|r| serde_json::to_string(r).unwrap_or_else(|_| "[]".into()))
            .as_deref()
    );
    if sets.is_empty() {
        return Ok(());
    }
    sets.push("updated_at = ?");
    args.push(sk_core::time::now_sql());
    let sql = format!("UPDATE magento_stores SET {} WHERE id = ?", sets.join(", "));
    let mut q = sqlx::query(&sql);
    for a in &args {
        q = q.bind(a);
    }
    q.bind(id).execute(pool).await?;
    Ok(())
}

/// Set the DB backup policy (schedule + retention).
pub async fn set_backup_policy(
    pool: &SqlitePool,
    id: i64,
    schedule: &str,
    retention: i64,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE magento_stores SET backup_schedule = ?, backup_retention = ?, updated_at = ? WHERE id = ?")
        .bind(schedule)
        .bind(retention.max(1))
        .bind(sk_core::time::now_sql())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn delete(pool: &SqlitePool, id: i64) -> anyhow::Result<()> {
    sqlx::query("DELETE FROM magento_stores WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
