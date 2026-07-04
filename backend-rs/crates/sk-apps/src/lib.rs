//! First-class persisted applications/services domain.

use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn token(n: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(n)
        .map(char::from)
        .collect()
}
fn s<'a>(b: &'a Value, k: &str, d: &'a str) -> &'a str {
    b.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(b: &'a Value, k: &str) -> Option<&'a str> {
    b.get(k).and_then(Value::as_str)
}
fn j(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sk_apps (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, app_type TEXT NOT NULL, source TEXT NOT NULL,
            source_id TEXT, status TEXT NOT NULL, root_path TEXT, domains_json TEXT NOT NULL DEFAULT '[]',
            version TEXT, project_id TEXT, environment_id TEXT, workspace_id TEXT, metadata_json TEXT NOT NULL DEFAULT '{}',
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_sk_apps_source ON sk_apps(source, source_id);
        CREATE TABLE IF NOT EXISTS sk_app_env_vars (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, key TEXT NOT NULL, value_encrypted TEXT NOT NULL,
            is_secret INTEGER NOT NULL DEFAULT 0, description TEXT, target_service TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(app_id, key)
        );
        CREATE TABLE IF NOT EXISTS sk_app_env_history (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, key TEXT NOT NULL, action TEXT NOT NULL, created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_grants (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, user_id TEXT NOT NULL, role TEXT NOT NULL, created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_private_urls (
            app_id TEXT PRIMARY KEY, slug TEXT NOT NULL, token TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_links (
            app_id TEXT PRIMARY KEY, target_app_id TEXT NOT NULL, as_environment TEXT, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_volumes (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, name TEXT NOT NULL, mount_path TEXT, source_path TEXT, wipe_on_delete INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_policies (
            app_id TEXT NOT NULL, kind TEXT NOT NULL, policy_json TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY(app_id, kind)
        );
        CREATE TABLE IF NOT EXISTS sk_app_previews (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_snapshots (
            id TEXT PRIMARY KEY, app_id TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_modules (
            name TEXT PRIMARY KEY, label TEXT NOT NULL, description TEXT, enabled INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_app_image_updates (
            app_id TEXT PRIMARY KEY, status TEXT NOT NULL, update_available INTEGER NOT NULL DEFAULT 0,
            image TEXT, current_digest TEXT, latest_digest TEXT, checked_at TEXT NOT NULL, error TEXT, metadata_json TEXT NOT NULL DEFAULT '{}'
        );
    "#).execute(pool).await.context("ensure sk-apps schema")?;
    Ok(())
}

fn app(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"), "app_type": row.get::<String,_>("app_type"),
        "source": row.get::<String,_>("source"), "source_id": row.try_get::<Option<String>,_>("source_id").ok().flatten(),
        "status": row.get::<String,_>("status"), "root_path": row.try_get::<Option<String>,_>("root_path").ok().flatten(),
        "domains": j(row.try_get::<Option<String>,_>("domains_json").ok().flatten()), "version": row.try_get::<Option<String>,_>("version").ok().flatten(),
        "project_id": row.try_get::<Option<String>,_>("project_id").ok().flatten(), "environment_id": row.try_get::<Option<String>,_>("environment_id").ok().flatten(),
        "workspace_id": row.try_get::<Option<String>,_>("workspace_id").ok().flatten(), "metadata": j(row.try_get::<Option<String>,_>("metadata_json").ok().flatten()),
        "project_name": Value::Null, "environment_name": Value::Null, "last_deploy_at": Value::Null,
        "deploy_repo_url": Value::Null, "upload_path": Value::Null,
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn env_var(row: &sqlx::sqlite::SqliteRow, reveal: bool) -> Value {
    let is_secret = row.get::<i64, _>("is_secret") != 0;
    let val = sk_core::crypto::decrypt_or_plain(&row.get::<String, _>("value_encrypted"));
    json!({"id":row.get::<String,_>("id"),"app_id":row.get::<String,_>("app_id"),"key":row.get::<String,_>("key"),"value":if is_secret&&!reveal{Value::Null}else{json!(val)},"is_secret":is_secret,"description":row.try_get::<Option<String>,_>("description").ok().flatten(),"target_service":row.try_get::<Option<String>,_>("target_service").ok().flatten(),"created_at":row.get::<String,_>("created_at"),"updated_at":row.get::<String,_>("updated_at")})
}

fn apps_dir() -> String {
    std::env::var("SK_APPS_DIR").unwrap_or_else(|_| "/var/www/serverkit-apps".into())
}
fn compose_status(compose: &str) -> &'static str {
    match std::process::Command::new("docker")
        .args(["compose", "-f", compose, "ps", "-q"])
        .output()
    {
        Ok(o) if o.status.success() && !o.stdout.trim_ascii().is_empty() => "running",
        _ => "stopped",
    }
}

pub async fn sync_adopted(pool: &SqlitePool) -> anyhow::Result<()> {
    let ts = now();
    if let Ok(stores) = sk_magento::store::list(pool).await {
        for st in stores {
            sqlx::query("INSERT INTO sk_apps (id,name,app_type,source,source_id,status,root_path,domains_json,version,metadata_json,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(source,source_id) DO UPDATE SET name=excluded.name,status=excluded.status,root_path=excluded.root_path,domains_json=excluded.domains_json,version=excluded.version,updated_at=excluded.updated_at")
        .bind(format!("magento-{}",st.id)).bind(&st.name).bind("magento").bind("magento").bind(st.id.to_string()).bind(&st.status).bind(&st.root_path).bind(json!([st.domain]).to_string()).bind(&st.magento_version).bind(json!({"distribution":st.distribution,"php_version":st.php_version}).to_string()).bind(&ts).bind(&ts).execute(pool).await?;
        }
    }
    let dir = apps_dir();
    for t in sk_templates::list_installed() {
        let name = t.get("name").and_then(Value::as_str).unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let root = format!("{dir}/{name}");
        let compose = format!("{root}/docker-compose.yml");
        sqlx::query("INSERT INTO sk_apps (id,name,app_type,source,source_id,status,root_path,domains_json,version,metadata_json,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(source,source_id) DO UPDATE SET status=excluded.status,root_path=excluded.root_path,domains_json=excluded.domains_json,metadata_json=excluded.metadata_json,updated_at=excluded.updated_at")
        .bind(format!("app-{name}")).bind(name).bind("docker").bind("template").bind(name).bind(compose_status(&compose)).bind(root).bind(t.get("domains").cloned().unwrap_or_else(||json!([])).to_string()).bind(t.get("template").and_then(Value::as_str)).bind(t.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    }
    Ok(())
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Value> {
    sync_adopted(pool).await?;
    let rows = sqlx::query("SELECT * FROM sk_apps ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"apps":rows.iter().map(app).collect::<Vec<_>>() }))
}
pub async fn get(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    sync_adopted(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_apps WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(app))
}
pub async fn create(pool: &SqlitePool, body: &Value, source: &str) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_apps (id,name,app_type,source,status,root_path,domains_json,version,project_id,environment_id,workspace_id,metadata_json,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(s(body,"name","Application")).bind(s(body,"app_type",s(body,"type","manual"))).bind(source).bind("created").bind(opt(body,"root_path")).bind(body.get("domains").cloned().unwrap_or_else(||json!([])).to_string()).bind(opt(body,"version")).bind(opt(body,"project_id")).bind(opt(body,"environment_id")).bind(opt(body,"workspace_id")).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    get(pool, &id).await?.context("created app missing")
}
pub async fn update(pool: &SqlitePool, id: &str, body: &Value) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_apps SET name=COALESCE(?,name), app_type=COALESCE(?,app_type), root_path=COALESCE(?,root_path), project_id=COALESCE(?,project_id), environment_id=COALESCE(?,environment_id), workspace_id=COALESCE(?,workspace_id), updated_at=? WHERE id=?").bind(opt(body,"name")).bind(opt(body,"app_type").or_else(||opt(body,"type"))).bind(opt(body,"root_path")).bind(opt(body,"project_id")).bind(opt(body,"environment_id")).bind(opt(body,"workspace_id")).bind(now()).bind(id).execute(pool).await?;
    get(pool, id).await
}
pub async fn delete(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_apps WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
pub async fn move_to_project(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let ids = body
        .get("app_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut apps = Vec::new();
    for v in ids {
        if let Some(id) = v.as_str() {
            sqlx::query(
                "UPDATE sk_apps SET project_id=?, environment_id=?, updated_at=? WHERE id=?",
            )
            .bind(opt(body, "project_id"))
            .bind(opt(body, "environment_id"))
            .bind(now())
            .bind(id)
            .execute(pool)
            .await?;
            if let Some(a) = get(pool, id).await? {
                apps.push(a)
            }
        }
    }
    Ok(json!({"apps":apps,"skipped":[]}))
}
pub async fn set_workspace(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_apps SET workspace_id=?, updated_at=? WHERE id=?")
        .bind(opt(body, "workspace_id"))
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}
pub async fn set_environment(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_apps SET environment_id=?, updated_at=? WHERE id=?")
        .bind(opt(body, "environment_id").or_else(|| opt(body, "environment_type")))
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    get(pool, id).await
}

async fn compose_path(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<String>> {
    Ok(get(pool, id).await?.and_then(|a| {
        a.get("root_path")
            .and_then(Value::as_str)
            .map(|r| format!("{r}/docker-compose.yml"))
    }))
}
pub async fn compose_action(pool: &SqlitePool, id: &str, action: &str) -> anyhow::Result<Value> {
    let Some(compose) = compose_path(pool, id).await? else {
        return Ok(json!({"success":false,"error":"compose app not found"}));
    };
    let out = std::process::Command::new("docker")
        .args(["compose", "-f", &compose, action])
        .output();
    match out {
        Ok(o) if o.status.success() => {
            let status = if action == "stop" {
                "stopped"
            } else {
                "running"
            };
            sqlx::query("UPDATE sk_apps SET status=?, updated_at=? WHERE id=?")
                .bind(status)
                .bind(now())
                .bind(id)
                .execute(pool)
                .await?;
            Ok(json!({"success":true,"status":status}))
        }
        Ok(o) => Ok(json!({"success":false,"error":String::from_utf8_lossy(&o.stderr)})),
        Err(e) => Ok(json!({"success":false,"error":e.to_string()})),
    }
}
pub async fn status(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let Some(a) = get(pool, id).await? else {
        return Ok(json!({"error":"not found"}));
    };
    if let Some(root) = a.get("root_path").and_then(Value::as_str) {
        let st = compose_status(&format!("{root}/docker-compose.yml"));
        Ok(json!({"status":st,"app":a}))
    } else {
        Ok(json!({"status":a["status"],"app":a}))
    }
}
pub async fn logs(pool: &SqlitePool, id: &str, lines: i64) -> anyhow::Result<Value> {
    let Some(compose) = compose_path(pool, id).await? else {
        return Ok(json!({"logs":"","lines":[]}));
    };
    let out = std::process::Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose,
            "logs",
            "--tail",
            &lines.to_string(),
        ])
        .output();
    let text = out
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
        .unwrap_or_default();
    Ok(json!({"logs":text,"lines":text.lines().collect::<Vec<_>>() }))
}
pub async fn compose_services(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let Some(compose) = compose_path(pool, id).await? else {
        return Ok(json!({"services":[]}));
    };
    let out = std::process::Command::new("docker")
        .args(["compose", "-f", &compose, "config", "--services"])
        .output();
    let services = out
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    Ok(json!({"services":services}))
}

pub async fn env_list(pool: &SqlitePool, app_id: &str, mask: bool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_app_env_vars WHERE app_id=? ORDER BY key")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(json!({"env_vars":rows.iter().map(|r|env_var(r,!mask)).collect::<Vec<_>>() }))
}
pub async fn env_get(pool: &SqlitePool, app_id: &str, key: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_app_env_vars WHERE app_id=? AND key=?")
        .bind(app_id)
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(|r| env_var(r, true)))
}
pub async fn env_set(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let eid = id();
    let ts = now();
    let key = s(body, "key", "");
    let val = sk_core::crypto::encrypt(s(body, "value", ""));
    sqlx::query("INSERT INTO sk_app_env_vars (id,app_id,key,value_encrypted,is_secret,description,target_service,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?) ON CONFLICT(app_id,key) DO UPDATE SET value_encrypted=excluded.value_encrypted,is_secret=excluded.is_secret,description=excluded.description,target_service=excluded.target_service,updated_at=excluded.updated_at").bind(&eid).bind(app_id).bind(key).bind(val).bind(if body.get("is_secret").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(opt(body,"description")).bind(opt(body,"target_service")).bind(&ts).bind(&ts).execute(pool).await?;
    sqlx::query(
        "INSERT INTO sk_app_env_history (id,app_id,key,action,created_at) VALUES (?,?,?,?,?)",
    )
    .bind(id())
    .bind(app_id)
    .bind(key)
    .bind("set")
    .bind(&ts)
    .execute(pool)
    .await?;
    env_get(pool, app_id, key).await?.context("env missing")
}
pub async fn env_delete(pool: &SqlitePool, app_id: &str, key: &str) -> anyhow::Result<Value> {
    let ts = now();
    let ok = sqlx::query("DELETE FROM sk_app_env_vars WHERE app_id=? AND key=?")
        .bind(app_id)
        .bind(key)
        .execute(pool)
        .await?
        .rows_affected()
        > 0;
    sqlx::query(
        "INSERT INTO sk_app_env_history (id,app_id,key,action,created_at) VALUES (?,?,?,?,?)",
    )
    .bind(id())
    .bind(app_id)
    .bind(key)
    .bind("delete")
    .bind(ts)
    .execute(pool)
    .await?;
    Ok(json!({"success":ok}))
}
pub async fn env_bulk(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let arr = body
        .get("env_vars")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut vars = Vec::new();
    for v in arr {
        vars.push(env_set(pool, app_id, &v).await?)
    }
    Ok(json!({"env_vars":vars}))
}
pub async fn env_import(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let content = s(body, "content", "");
    let mut vars = Vec::new();
    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            vars.push(env_set(pool, app_id, &json!({"key":k.trim(),"value":v.trim()})).await?)
        }
    }
    Ok(json!({"env_vars":vars}))
}
pub async fn env_export(
    pool: &SqlitePool,
    app_id: &str,
    include_secrets: bool,
) -> anyhow::Result<Value> {
    let v = env_list(pool, app_id, !include_secrets).await?;
    let mut content = String::new();
    for e in v["env_vars"].as_array().unwrap_or(&Vec::new()) {
        if let Some(k) = e["key"].as_str() {
            let val = e["value"].as_str().unwrap_or("");
            content.push_str(&format!("{k}={val}\n"));
        }
    }
    Ok(json!({"content":content}))
}
pub async fn env_history(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_app_env_history WHERE app_id=? ORDER BY created_at DESC LIMIT 100",
    )
    .bind(app_id)
    .fetch_all(pool)
    .await?;
    Ok(
        json!({"history":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"key":r.get::<String,_>("key"),"action":r.get::<String,_>("action"),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn env_clear(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_app_env_vars WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":true,"deleted":n}))
}

pub async fn grants(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_app_grants WHERE app_id=?")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"grants":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"app_id":r.get::<String,_>("app_id"),"user_id":r.get::<String,_>("user_id"),"role":r.get::<String,_>("role"),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn grant(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let gid = id();
    sqlx::query("INSERT INTO sk_app_grants (id,app_id,user_id,role,created_at) VALUES (?,?,?,?,?)")
        .bind(&gid)
        .bind(app_id)
        .bind(s(body, "user_id", "0"))
        .bind(s(body, "role", "viewer"))
        .bind(now())
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"id":gid}))
}
pub async fn revoke_grant(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_app_grants WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn private_url(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_app_private_urls WHERE app_id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await?;
    Ok(r.map(|r|json!({"app_id":r.get::<String,_>("app_id"),"slug":r.get::<String,_>("slug"),"token":r.get::<String,_>("token"),"enabled":r.get::<i64,_>("enabled")!=0})).unwrap_or(Value::Null))
}
pub async fn set_private_url(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let slug = opt(body, "slug")
        .map(str::to_string)
        .unwrap_or_else(|| format!("app-{}", token(8)));
    let tok = token(24);
    let ts = now();
    sqlx::query("INSERT INTO sk_app_private_urls (app_id,slug,token,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?) ON CONFLICT(app_id) DO UPDATE SET slug=excluded.slug, enabled=1, updated_at=excluded.updated_at").bind(app_id).bind(&slug).bind(&tok).bind(1).bind(&ts).bind(&ts).execute(pool).await?;
    private_url(pool, app_id).await
}
pub async fn disable_private_url(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_app_private_urls WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":true}))
}
pub async fn regenerate_private_url(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let tok = token(24);
    sqlx::query("UPDATE sk_app_private_urls SET token=?, updated_at=? WHERE app_id=?")
        .bind(tok)
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    private_url(pool, app_id).await
}

pub async fn link(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let ts = now();
    sqlx::query("INSERT INTO sk_app_links (app_id,target_app_id,as_environment,metadata_json,created_at,updated_at) VALUES (?,?,?,?,?,?) ON CONFLICT(app_id) DO UPDATE SET target_app_id=excluded.target_app_id,as_environment=excluded.as_environment,metadata_json=excluded.metadata_json,updated_at=excluded.updated_at").bind(app_id).bind(s(body,"target_app_id","")).bind(opt(body,"as_environment")).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    linked(pool, app_id).await
}
pub async fn linked(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_app_links WHERE app_id=? OR target_app_id=?")
        .bind(app_id)
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"links":rows.iter().map(|r|json!({"app_id":r.get::<String,_>("app_id"),"target_app_id":r.get::<String,_>("target_app_id"),"as_environment":r.try_get::<Option<String>,_>("as_environment").ok().flatten(),"metadata":j(r.try_get::<Option<String>,_>("metadata_json").ok().flatten())})).collect::<Vec<_>>() }),
    )
}
pub async fn unlink(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_app_links WHERE app_id=?").bind(app_id).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn volumes(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_app_volumes WHERE app_id=?")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"volumes":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"mount_path":r.try_get::<Option<String>,_>("mount_path").ok().flatten(),"source_path":r.try_get::<Option<String>,_>("source_path").ok().flatten(),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn add_volume(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let vid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_app_volumes (id,app_id,name,mount_path,source_path,wipe_on_delete,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)").bind(&vid).bind(app_id).bind(s(body,"name","volume")).bind(opt(body,"mount_path")).bind(opt(body,"source_path")).bind(if body.get("wipe_on_delete").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"id":vid}))
}
pub async fn delete_volume(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_app_volumes WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn policy(pool: &SqlitePool, app_id: &str, kind: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT policy_json FROM sk_app_policies WHERE app_id=? AND kind=?")
        .bind(app_id)
        .bind(kind)
        .fetch_optional(pool)
        .await?;
    Ok(j(r.map(|r| r.get::<String, _>("policy_json"))))
}
pub async fn set_policy(
    pool: &SqlitePool,
    app_id: &str,
    kind: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_app_policies (app_id,kind,policy_json,updated_at) VALUES (?,?,?,?) ON CONFLICT(app_id,kind) DO UPDATE SET policy_json=excluded.policy_json, updated_at=excluded.updated_at").bind(app_id).bind(kind).bind(body.to_string()).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"policy":body}))
}

fn module_defs() -> Vec<(&'static str, &'static str, &'static str, bool)> {
    vec![
        (
            "email",
            "Email",
            "Email accounts, mailbox tools, and delivery-related feature areas.",
            true,
        ),
        (
            "wordpress",
            "WordPress",
            "WordPress site-management and pipeline feature areas.",
            true,
        ),
    ]
}

pub async fn ensure_default_modules(pool: &SqlitePool) -> anyhow::Result<()> {
    let ts = now();
    for (name, label, description, enabled) in module_defs() {
        sqlx::query("INSERT INTO sk_modules(name,label,description,enabled,updated_at) VALUES(?,?,?,?,?) ON CONFLICT(name) DO NOTHING")
            .bind(name)
            .bind(label)
            .bind(description)
            .bind(if enabled { 1 } else { 0 })
            .bind(&ts)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn modules(pool: &SqlitePool) -> anyhow::Result<Value> {
    ensure_default_modules(pool).await?;
    let rows = sqlx::query("SELECT * FROM sk_modules ORDER BY label")
        .fetch_all(pool)
        .await?;
    Ok(json!({"modules":rows.iter().map(|r|json!({
        "name":r.get::<String,_>("name"),
        "label":r.get::<String,_>("label"),
        "description":r.try_get::<Option<String>,_>("description").ok().flatten(),
        "enabled":r.get::<i64,_>("enabled") != 0,
        "updated_at":r.get::<String,_>("updated_at"),
    })).collect::<Vec<_>>() }))
}

pub async fn set_module(pool: &SqlitePool, name: &str, enabled: bool) -> anyhow::Result<Value> {
    ensure_default_modules(pool).await?;
    sqlx::query("UPDATE sk_modules SET enabled=?, updated_at=? WHERE name=?")
        .bind(if enabled { 1 } else { 0 })
        .bind(now())
        .bind(name)
        .execute(pool)
        .await?;
    let row = sqlx::query("SELECT * FROM sk_modules WHERE name=?")
        .bind(name)
        .fetch_optional(pool)
        .await?;
    Ok(row
        .as_ref()
        .map(|r| {
            json!({
                "name":r.get::<String,_>("name"),
                "label":r.get::<String,_>("label"),
                "description":r.try_get::<Option<String>,_>("description").ok().flatten(),
                "enabled":r.get::<i64,_>("enabled") != 0,
                "updated_at":r.get::<String,_>("updated_at"),
            })
        })
        .unwrap_or_else(|| json!({"success":false,"error":"module not found"})))
}

fn command_out(cmd: &str, args: &[&str]) -> (bool, String, String) {
    match std::process::Command::new(cmd).args(args).output() {
        Ok(o) => (
            o.status.success(),
            String::from_utf8_lossy(&o.stdout).trim().to_string(),
            String::from_utf8_lossy(&o.stderr).trim().to_string(),
        ),
        Err(e) => (false, String::new(), e.to_string()),
    }
}

fn first_image_from_compose(compose: &str) -> Option<String> {
    let (ok, stdout, _) = command_out("docker", &["compose", "-f", compose, "config", "--images"]);
    if !ok {
        return None;
    }
    stdout
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn docker_current_digest(image: &str) -> Option<String> {
    let (ok, stdout, _) = command_out(
        "docker",
        &[
            "image",
            "inspect",
            "--format",
            "{{json .RepoDigests}}",
            image,
        ],
    );
    if !ok {
        return None;
    }
    serde_json::from_str::<Vec<String>>(&stdout)
        .ok()
        .and_then(|items| items.into_iter().find(|d| d.contains("@sha256:")))
        .and_then(|d| d.split('@').nth(1).map(str::to_string))
}

fn find_digest(v: &Value) -> Option<String> {
    match v {
        Value::Object(map) => {
            if let Some(d) = map.get("digest").and_then(Value::as_str) {
                if d.starts_with("sha256:") {
                    return Some(d.to_string());
                }
            }
            for value in map.values() {
                if let Some(d) = find_digest(value) {
                    return Some(d);
                }
            }
            None
        }
        Value::Array(items) => items.iter().find_map(find_digest),
        _ => None,
    }
}

fn docker_latest_digest(image: &str) -> Result<Option<String>, String> {
    let (ok, stdout, stderr) = command_out("docker", &["manifest", "inspect", image]);
    if !ok {
        return Err(if stderr.is_empty() {
            "docker manifest inspect failed".to_string()
        } else {
            stderr
        });
    }
    Ok(serde_json::from_str::<Value>(&stdout)
        .ok()
        .and_then(|v| find_digest(&v)))
}

async fn persist_image_update(pool: &SqlitePool, app_id: &str, info: &Value) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sk_app_image_updates(app_id,status,update_available,image,current_digest,latest_digest,checked_at,error,metadata_json) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(app_id) DO UPDATE SET status=excluded.status,update_available=excluded.update_available,image=excluded.image,current_digest=excluded.current_digest,latest_digest=excluded.latest_digest,checked_at=excluded.checked_at,error=excluded.error,metadata_json=excluded.metadata_json")
        .bind(app_id)
        .bind(info.get("status").and_then(Value::as_str).unwrap_or("unknown"))
        .bind(if info.get("update_available").and_then(Value::as_bool).unwrap_or(false) { 1 } else { 0 })
        .bind(info.get("image").and_then(Value::as_str))
        .bind(info.get("current_digest").and_then(Value::as_str))
        .bind(info.get("latest_digest").and_then(Value::as_str))
        .bind(info.get("checked_at").and_then(Value::as_str).unwrap_or(&now()))
        .bind(info.get("error").and_then(Value::as_str))
        .bind(info.to_string())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn image_update(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    if let Some(row) = sqlx::query("SELECT * FROM sk_app_image_updates WHERE app_id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await?
    {
        return Ok(json!({
            "app_id":app_id,
            "status":row.get::<String,_>("status"),
            "update_available":row.get::<i64,_>("update_available") != 0,
            "image":row.try_get::<Option<String>,_>("image").ok().flatten(),
            "current_digest":row.try_get::<Option<String>,_>("current_digest").ok().flatten(),
            "latest_digest":row.try_get::<Option<String>,_>("latest_digest").ok().flatten(),
            "checked_at":row.get::<String,_>("checked_at"),
            "error":row.try_get::<Option<String>,_>("error").ok().flatten(),
            "metadata":j(row.try_get::<Option<String>,_>("metadata_json").ok().flatten()),
        }));
    }
    Ok(
        json!({"app_id":app_id,"status":"unknown","update_available":false,"current_digest":Value::Null,"latest_digest":Value::Null,"checked_at":Value::Null}),
    )
}

pub async fn check_image_update(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"app not found"}));
    };
    let compose = app
        .get("root_path")
        .and_then(Value::as_str)
        .map(|r| format!("{r}/docker-compose.yml"));
    let ts = now();
    let Some(compose) = compose.filter(|p| std::path::Path::new(p).exists()) else {
        let info = json!({"app_id":app_id,"status":"unknown","update_available":false,"checked_at":ts,"error":"app has no docker-compose.yml to inspect"});
        persist_image_update(pool, app_id, &info).await?;
        return Ok(info);
    };
    let Some(image) = first_image_from_compose(&compose) else {
        let info = json!({"app_id":app_id,"status":"unknown","update_available":false,"checked_at":ts,"error":"compose file has no image reference"});
        persist_image_update(pool, app_id, &info).await?;
        return Ok(info);
    };
    let current = docker_current_digest(&image);
    let latest = docker_latest_digest(&image);
    let (status, update_available, error) = match (&current, &latest) {
        (Some(c), Ok(Some(l))) if c == l => ("up_to_date", false, None),
        (Some(_), Ok(Some(_))) => ("update_available", true, None),
        (_, Err(e)) => ("unknown", false, Some(e.clone())),
        _ => (
            "unknown",
            false,
            Some("could not determine current or latest image digest".to_string()),
        ),
    };
    let info = json!({
        "app_id":app_id,"status":status,"update_available":update_available,"image":image,
        "current_digest":current,"latest_digest":latest.ok().flatten(),"checked_at":ts,"error":error,
    });
    persist_image_update(pool, app_id, &info).await?;
    Ok(info)
}

pub async fn apply_image_update(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(compose) = compose_path(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"compose app not found"}));
    };
    let (pull_ok, pull_out, pull_err) = command_out("docker", &["compose", "-f", &compose, "pull"]);
    if !pull_ok {
        return Ok(json!({"success":false,"error":pull_err,"output":pull_out}));
    }
    let (up_ok, up_out, up_err) = command_out("docker", &["compose", "-f", &compose, "up", "-d"]);
    if up_ok {
        sqlx::query("UPDATE sk_apps SET status='running', updated_at=? WHERE id=?")
            .bind(now())
            .bind(app_id)
            .execute(pool)
            .await?;
        let checked = check_image_update(pool, app_id).await?;
        Ok(
            json!({"success":true,"message":"Image pulled and app recreated","pull_output":pull_out,"up_output":up_out,"image_update":checked}),
        )
    } else {
        Ok(json!({"success":false,"error":up_err,"output":up_out}))
    }
}

pub async fn previews(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_app_previews WHERE app_id=? ORDER BY created_at DESC")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"previews":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"metadata":j(r.try_get::<Option<String>,_>("metadata_json").ok().flatten())})).collect::<Vec<_>>() }),
    )
}
pub async fn preview_settings(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    policy(pool, app_id, "preview").await
}
pub async fn snapshots(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_app_snapshots WHERE app_id=? ORDER BY created_at DESC")
            .bind(app_id)
            .fetch_all(pool)
            .await?;
    Ok(
        json!({"snapshots":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"kind":r.get::<String,_>("kind"),"metadata":j(r.try_get::<Option<String>,_>("metadata_json").ok().flatten()),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn snapshot(pool: &SqlitePool, app_id: &str, sid: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_app_snapshots WHERE app_id=? AND id=?")
        .bind(app_id)
        .bind(sid)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"kind":r.get::<String,_>("kind"),"metadata":j(r.try_get::<Option<String>,_>("metadata_json").ok().flatten()),"created_at":r.get::<String,_>("created_at")})))
}

pub fn ok(action: &str) -> Value {
    json!({"success":true,"action":action})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn app_env_roundtrip() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let a = create(&pool, &json!({"name":"A"}), "manual").await.unwrap();
        let e = env_set(
            &pool,
            a["id"].as_str().unwrap(),
            &json!({"key":"X","value":"1","is_secret":true}),
        )
        .await
        .unwrap();
        assert_eq!(e["key"], "X");
        let list = env_list(&pool, a["id"].as_str().unwrap(), true)
            .await
            .unwrap();
        assert!(list["env_vars"][0]["value"].is_null());
    }
}
