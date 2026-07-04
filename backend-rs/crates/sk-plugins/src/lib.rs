use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn j(s: String) -> Value {
    serde_json::from_str(&s).unwrap_or(Value::Null)
}
pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_plugins(id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, version TEXT NOT NULL, source TEXT NOT NULL, status TEXT NOT NULL, manifest_json TEXT NOT NULL DEFAULT '{}', installed_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_plugin_configs(plugin_id TEXT PRIMARY KEY, config_json TEXT NOT NULL DEFAULT '{}', updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_agent_plugins(id TEXT PRIMARY KEY, slug TEXT NOT NULL UNIQUE, name TEXT NOT NULL, version TEXT NOT NULL, status TEXT NOT NULL, spec_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_agent_plugin_installs(id TEXT PRIMARY KEY, plugin_id TEXT NOT NULL, server_id TEXT NOT NULL, status TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-plugins schema")?;
    Ok(())
}
const BUILTINS: &[(&str, &str, &str, &str)] = &[
    ("serverkit-gpu", "GPU Monitor", "monitoring", "1.0.0"),
    ("serverkit-ftp", "FTP Server", "utility", "1.0.0"),
    (
        "serverkit-workflows",
        "Workflow Builder",
        "deployment",
        "1.0.0",
    ),
    (
        "serverkit-gui",
        "ServerKit Agent GUI",
        "monitoring",
        "0.1.0",
    ),
    (
        "serverkit-cloudflare-ops",
        "Cloudflare Zone Ops",
        "integration",
        "1.0.0",
    ),
    ("serverkit-email", "Email Server", "integration", "1.0.0"),
    ("serverkit-git", "Git Server", "deployment", "1.0.0"),
    (
        "serverkit-cloud-provision",
        "Cloud Provisioning",
        "deployment",
        "1.0.0",
    ),
    ("serverkit-wordpress", "WordPress", "integration", "1.0.0"),
    ("serverkit-status", "Status Pages", "monitoring", "1.0.0"),
    (
        "serverkit-remote-access",
        "Remote Access",
        "integration",
        "1.0.0",
    ),
];
fn builtin(
    slug: &str,
    name: &str,
    cat: &str,
    ver: &str,
    installed: bool,
    install_id: Option<String>,
    status: Option<String>,
) -> Value {
    json!({"folder":slug,"path":format!("frontend/src/plugins/{slug}"),"slug":slug,"manifest":{"name":slug,"display_name":name,"version":ver,"category":cat,"description":format!("Bundled {name} extension")},"display_name":name,"description":format!("Bundled {name} extension"),"version":ver,"category":cat,"author":"ServerKit","first_party":true,"permissions":[],"source":"bundled","source_kind":"bundled","installed":installed,"install_id":install_id,"status":status.unwrap_or_else(||if installed{"active".into()}else{"available".into()}),"installed_version":if installed{Value::String(ver.into())}else{Value::Null}})
}
async fn installed_map(
    pool: &SqlitePool,
) -> anyhow::Result<std::collections::HashMap<String, (String, String)>> {
    let rows = sqlx::query("SELECT id,slug,status FROM sk_plugins")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| {
            (
                r.get::<String, _>("slug"),
                (r.get::<String, _>("id"), r.get::<String, _>("status")),
            )
        })
        .collect())
}
pub async fn builtin_extensions(pool: &SqlitePool) -> anyhow::Result<Value> {
    let m = installed_map(pool).await?;
    let vals: Vec<_> = BUILTINS
        .iter()
        .map(|(slug, name, cat, ver)| {
            let x = m.get(*slug);
            builtin(
                slug,
                name,
                cat,
                ver,
                x.is_some(),
                x.map(|p| p.0.clone()),
                x.map(|p| p.1.clone()),
            )
        })
        .collect();
    Ok(json!({"success":true,"plugins":vals,"extensions":vals,"source":"bundled"}))
}
fn plugin_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"slug":r.get::<String,_>("slug"),"name":r.get::<String,_>("name"),"version":r.get::<String,_>("version"),"source":r.get::<String,_>("source"),"status":r.get::<String,_>("status"),"manifest":j(r.get::<String,_>("manifest_json")),"installed_at":r.get::<String,_>("installed_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn plugins(pool: &SqlitePool, status: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(st) = status {
        sqlx::query("SELECT * FROM sk_plugins WHERE status=? ORDER BY slug")
            .bind(st)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_plugins ORDER BY slug")
            .fetch_all(pool)
            .await?
    };
    let vals: Vec<_> = rows.into_iter().map(plugin_row).collect();
    Ok(json!({"success":true,"plugins":vals,"count":vals.len()}))
}
pub async fn plugin(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_plugins WHERE id=? OR slug=?")
        .bind(pid)
        .bind(pid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"success":true,"plugin":plugin_row(r)}),
        None => json!({"success":false,"code":"PLUGIN_NOT_FOUND"}),
    })
}
async fn install_record(
    pool: &SqlitePool,
    slug: &str,
    name: &str,
    version: &str,
    source: &str,
    manifest: Value,
) -> anyhow::Result<Value> {
    if let Some(r) = sqlx::query("SELECT * FROM sk_plugins WHERE slug=?")
        .bind(slug)
        .fetch_optional(pool)
        .await?
    {
        return Ok(json!({"success":true,"plugin":plugin_row(r)}));
    }
    let pid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_plugins(id,slug,name,version,source,status,manifest_json,installed_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(&pid).bind(slug).bind(name).bind(version).bind(source).bind("active").bind(manifest.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"plugin":plugin(pool,&pid).await?["plugin"].clone()}))
}
pub async fn install_builtin(pool: &SqlitePool, slug: &str) -> anyhow::Result<Value> {
    let Some((slug, name, cat, ver)) = BUILTINS.iter().find(|x| x.0 == slug) else {
        return Ok(json!({"success":false,"code":"BUILTIN_NOT_FOUND"}));
    };
    install_record(
        pool,
        slug,
        name,
        ver,
        "builtin",
        json!({"category":cat,"name":slug,"display_name":name,"version":ver}),
    )
    .await
}
pub async fn install_url(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let url = s(body, "url", "");
    if url.is_empty() {
        return Ok(json!({"success":false,"error":"url required"}));
    }
    let slug = url
        .rsplit('/')
        .next()
        .unwrap_or("plugin")
        .trim_end_matches(".git");
    install_record(pool, slug, slug, "0.0.0", "url", json!({"url":url})).await
}
pub async fn install_local(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let path = s(body, "path", "");
    if path.is_empty() {
        return Ok(json!({"success":false,"error":"path required"}));
    }
    if !std::path::Path::new(path).exists() {
        return Ok(json!({"success":false,"code":"LOCAL_PLUGIN_PATH_NOT_FOUND"}));
    }
    let slug = std::path::Path::new(path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("local-plugin");
    install_record(pool, slug, slug, "0.0.0", "local", json!({"path":path})).await
}
pub async fn install_upload(
    pool: &SqlitePool,
    filename: &str,
    bytes: usize,
) -> anyhow::Result<Value> {
    let slug = filename.trim_end_matches(".zip");
    install_record(
        pool,
        slug,
        slug,
        "0.0.0",
        "upload",
        json!({"filename":filename,"bytes":bytes,"stored":false}),
    )
    .await
}
pub async fn uninstall(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_plugins WHERE id=? OR slug=?")
        .bind(pid)
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    sqlx::query("DELETE FROM sk_plugin_configs WHERE plugin_id=?")
        .bind(pid)
        .execute(pool)
        .await?;
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn set_status(pool: &SqlitePool, pid: &str, status: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_plugins SET status=?,updated_at=? WHERE id=? OR slug=?")
        .bind(status)
        .bind(now())
        .bind(pid)
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"plugin":plugin(pool,pid).await?["plugin"].clone()}))
}
pub async fn config(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT config_json FROM sk_plugin_configs WHERE plugin_id=?")
        .bind(pid)
        .fetch_optional(pool)
        .await?;
    Ok(
        json!({"success":true,"config":r.map(|r|j(r.get::<String,_>("config_json"))).unwrap_or_else(||json!({})),"config_schema":{}}),
    )
}
pub async fn update_config(pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    let cfg = body.get("config").cloned().unwrap_or_else(|| body.clone());
    sqlx::query("INSERT INTO sk_plugin_configs(plugin_id,config_json,updated_at) VALUES(?,?,?) ON CONFLICT(plugin_id) DO UPDATE SET config_json=excluded.config_json,updated_at=excluded.updated_at").bind(pid).bind(cfg.to_string()).bind(now()).execute(pool).await?;
    config(pool, pid).await
}
pub async fn contributions(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT slug FROM sk_plugins WHERE status='active'")
        .fetch_all(pool)
        .await?;
    let nav: Vec<_> = rows
        .into_iter()
        .map(|r| {
            let slug = r.get::<String, _>("slug");
            json!({"id":slug,"plugin":slug,"label":slug,"route":format!("/{slug}")})
        })
        .collect();
    Ok(
        json!({"nav":nav,"routes":[],"page_titles":{},"command_palette":[],"widgets":[],"layouts":[],"tabs":[],"ai":{"suggested_prompts":[],"tool_renderers":[]}}),
    )
}
pub async fn updates(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT id,slug,version,source FROM sk_plugins ORDER BY slug")
        .fetch_all(pool)
        .await?;
    let vals:Vec<_>=rows.into_iter().map(|r|json!({"slug":r.get::<String,_>("slug"),"plugin_id":r.get::<String,_>("id"),"installed_version":r.get::<String,_>("version"),"available_version":r.get::<String,_>("version"),"update_available":false,"compatible":true,"source":r.get::<String,_>("source")})).collect();
    Ok(json!({"success":true,"updates":vals}))
}
pub async fn update_plugin(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    sqlx::query("UPDATE sk_plugins SET updated_at=? WHERE id=? OR slug=?")
        .bind(now())
        .bind(pid)
        .bind(pid)
        .execute(pool)
        .await?;
    plugin(pool, pid).await
}
pub async fn marketplace(pool: &SqlitePool) -> anyhow::Result<Value> {
    let b = builtin_extensions(pool).await?;
    Ok(json!({"success":true,"extensions":b["extensions"].clone(),"source":"bundled"}))
}
fn agent_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"slug":r.get::<String,_>("slug"),"name":r.get::<String,_>("name"),"version":r.get::<String,_>("version"),"status":r.get::<String,_>("status"),"spec":j(r.get::<String,_>("spec_json")),"created_at":r.get::<String,_>("created_at")})
}
pub async fn agent_plugins(pool: &SqlitePool, status: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(st) = status {
        sqlx::query("SELECT * FROM sk_agent_plugins WHERE status=? ORDER BY slug")
            .bind(st)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_agent_plugins ORDER BY slug")
            .fetch_all(pool)
            .await?
    };
    let vals: Vec<_> = rows.into_iter().map(agent_row).collect();
    Ok(json!({"success":true,"plugins":vals,"count":vals.len()}))
}
pub async fn create_agent(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let pid = id();
    let ts = now();
    let slug = s(body, "slug", s(body, "name", "agent-plugin"));
    sqlx::query("INSERT INTO sk_agent_plugins(id,slug,name,version,status,spec_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(&pid).bind(slug).bind(s(body,"name",slug)).bind(s(body,"version","0.1.0")).bind(s(body,"status","active")).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"plugin":agent(pool,&pid).await?["plugin"].clone()}))
}
pub async fn agent(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_agent_plugins WHERE id=? OR slug=?")
        .bind(pid)
        .bind(pid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"success":true,"plugin":agent_row(r)}),
        None => json!({"success":false,"code":"AGENT_PLUGIN_NOT_FOUND"}),
    })
}
pub async fn update_agent(pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    let n=sqlx::query("UPDATE sk_agent_plugins SET name=?,version=?,status=?,spec_json=?,updated_at=? WHERE id=? OR slug=?").bind(s(body,"name","agent-plugin")).bind(s(body,"version","0.1.0")).bind(s(body,"status","active")).bind(body.to_string()).bind(now()).bind(pid).bind(pid).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"plugin":agent(pool,pid).await?["plugin"].clone()}))
}
pub async fn delete_agent(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_agent_plugin_installs WHERE plugin_id=?")
        .bind(pid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_agent_plugins WHERE id=? OR slug=?")
        .bind(pid)
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
fn inst_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"plugin_id":r.get::<String,_>("plugin_id"),"server_id":r.get::<String,_>("server_id"),"status":r.get::<String,_>("status"),"config":j(r.get::<String,_>("config_json")),"created_at":r.get::<String,_>("created_at")})
}
pub async fn install_agent(pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    let server = s(body, "server_id", "local");
    let iid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_agent_plugin_installs(id,plugin_id,server_id,status,config_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?)").bind(&iid).bind(pid).bind(server).bind("installed").bind(body.get("config").cloned().unwrap_or_else(||json!({})).to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"install":install(pool,&iid).await?["install"].clone()}))
}
pub async fn bulk_install_agent(
    pool: &SqlitePool,
    pid: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let arr = body
        .get("server_ids")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut vals = Vec::new();
    for sid in arr {
        vals.push(install_agent(pool,pid,&json!({"server_id":sid.as_str().unwrap_or("local"),"config":body.get("config").cloned().unwrap_or_else(||json!({}))})).await?["install"].clone());
    }
    Ok(json!({"success":true,"installs":vals}))
}
pub async fn installations(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_agent_plugin_installs WHERE plugin_id=? ORDER BY created_at DESC",
    )
    .bind(pid)
    .fetch_all(pool)
    .await?;
    let vals: Vec<_> = rows.into_iter().map(inst_row).collect();
    Ok(json!({"success":true,"installations":vals,"installs":vals}))
}
pub async fn server_plugins(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_agent_plugin_installs WHERE server_id=? ORDER BY created_at DESC",
    )
    .bind(sid)
    .fetch_all(pool)
    .await?;
    let vals: Vec<_> = rows.into_iter().map(inst_row).collect();
    Ok(json!({"success":true,"plugins":vals,"installs":vals}))
}
pub async fn install(pool: &SqlitePool, iid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_agent_plugin_installs WHERE id=?")
        .bind(iid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"success":true,"install":inst_row(r)}),
        None => json!({"success":false,"code":"INSTALL_NOT_FOUND"}),
    })
}
pub async fn set_install_status(
    pool: &SqlitePool,
    iid: &str,
    status: &str,
) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_agent_plugin_installs SET status=?,updated_at=? WHERE id=?")
        .bind(status)
        .bind(now())
        .bind(iid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"install":install(pool,iid).await?["install"].clone()}))
}
pub async fn delete_install(pool: &SqlitePool, iid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_agent_plugin_installs WHERE id=?")
        .bind(iid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn update_install_config(
    pool: &SqlitePool,
    iid: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    sqlx::query("UPDATE sk_agent_plugin_installs SET config_json=?,updated_at=? WHERE id=?")
        .bind(
            body.get("config")
                .cloned()
                .unwrap_or_else(|| body.clone())
                .to_string(),
        )
        .bind(now())
        .bind(iid)
        .execute(pool)
        .await?;
    install(pool, iid).await
}
pub fn agent_spec() -> Value {
    json!({"success":true,"spec":{"schema_version":"1","hooks":["collect","remediate","report"],"config_schema":{},"install_target":"server-agent"}})
}
