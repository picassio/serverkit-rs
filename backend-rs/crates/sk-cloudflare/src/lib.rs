use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

const API: &str = "https://api.cloudflare.com/client/v4";
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_cf_settings(zone_id TEXT NOT NULL, setting_id TEXT NOT NULL, value_json TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY(zone_id,setting_id));
CREATE TABLE IF NOT EXISTS sk_cf_waf_rules(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, ruleset_id TEXT, rule_id TEXT, rule_json TEXT NOT NULL, provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_cf_workers(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, name TEXT NOT NULL, script TEXT, metadata_json TEXT NOT NULL DEFAULT '{}', provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(zone_id,name));
CREATE TABLE IF NOT EXISTS sk_cf_worker_routes(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, pattern TEXT NOT NULL, script TEXT NOT NULL, provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_cf_tunnels(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, name TEXT NOT NULL, provider_tunnel_id TEXT, provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_cf_tunnel_hostnames(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, tunnel_id TEXT NOT NULL, hostname TEXT NOT NULL, service TEXT NOT NULL, provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(tunnel_id,hostname));
CREATE TABLE IF NOT EXISTS sk_cf_storage(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, kind TEXT NOT NULL, name TEXT NOT NULL, provider_id TEXT, provider_status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(zone_id,kind,name));
"#).execute(pool).await.context("ensure sk-cloudflare schema")?;
    Ok(())
}

async fn cf_zone(
    pool: &SqlitePool,
    zone_id: &str,
) -> anyhow::Result<(bool, Option<String>, Option<String>)> {
    let token = std::env::var("SK_CF_API_TOKEN").ok();
    let row =
        sqlx::query("SELECT provider_zone_id, domain FROM sk_dns_zones WHERE id=? OR domain=?")
            .bind(zone_id)
            .bind(zone_id)
            .fetch_optional(pool)
            .await
            .ok()
            .flatten();
    let provider = row
        .as_ref()
        .and_then(|r| {
            r.try_get::<Option<String>, _>("provider_zone_id")
                .ok()
                .flatten()
        })
        .or_else(|| std::env::var("SK_CF_ZONE_ID").ok());
    Ok((token.is_some() && provider.is_some(), token, provider))
}
async fn cf_request(
    pool: &SqlitePool,
    zone_id: &str,
    method: &str,
    path: &str,
    body: Option<&Value>,
) -> anyhow::Result<Value> {
    let (configured, token, provider) = cf_zone(pool, zone_id).await?;
    if !configured {
        return Ok(
            json!({"success":false,"configured":false,"error":"Cloudflare is not configured for this zone","zone_id":zone_id}),
        );
    }
    let url = format!(
        "{API}{}",
        path.replace("{zone}", provider.as_deref().unwrap())
    );
    let client = reqwest::Client::new();
    let mut req = match method {
        "GET" => client.get(&url),
        "POST" => client.post(&url),
        "PATCH" => client.patch(&url),
        "DELETE" => client.delete(&url),
        _ => client.get(&url),
    }
    .bearer_auth(token.unwrap());
    if let Some(b) = body {
        req = req.json(b);
    }
    let v: Value = req.send().await?.json().await?;
    Ok(v)
}
fn setting_defaults() -> Vec<&'static str> {
    vec![
        "ssl",
        "always_use_https",
        "automatic_https_rewrites",
        "min_tls_version",
        "brotli",
        "security_level",
        "cache_level",
    ]
}
pub async fn settings(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_cf_settings WHERE zone_id=?")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    let local:Vec<Value>=rows.iter().map(|r|json!({"id":r.get::<String,_>("setting_id"),"value":j(Some(r.get::<String,_>("value_json"))),"source":"local","updated_at":r.get::<String,_>("updated_at")})).collect();
    let provider = cf_request(pool, zone_id, "GET", "/zones/{zone}/settings", None).await?;
    Ok(
        json!({"configured":provider["success"].as_bool()==Some(true),"settings":if provider["success"].as_bool()==Some(true){provider["result"].clone()}else{json!(local)},"local_settings":local,"provider":provider}),
    )
}
pub async fn setting(pool: &SqlitePool, zone_id: &str, setting_id: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_cf_settings WHERE zone_id=? AND setting_id=?")
        .bind(zone_id)
        .bind(setting_id)
        .fetch_optional(pool)
        .await?;
    let provider = cf_request(
        pool,
        zone_id,
        "GET",
        &format!("/zones/{{zone}}/settings/{setting_id}"),
        None,
    )
    .await?;
    Ok(
        json!({"configured":provider["success"].as_bool()==Some(true),"setting":if provider["success"].as_bool()==Some(true){provider["result"].clone()}else{r.map(|r|json!({"id":setting_id,"value":j(Some(r.get::<String,_>("value_json")))})).unwrap_or_else(||json!({"id":setting_id,"value":Value::Null}))},"provider":provider}),
    )
}
pub async fn update_setting(
    pool: &SqlitePool,
    zone_id: &str,
    setting_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let value = b.get("value").cloned().unwrap_or(Value::Null);
    sqlx::query("INSERT INTO sk_cf_settings(zone_id,setting_id,value_json,updated_at) VALUES(?,?,?,?) ON CONFLICT(zone_id,setting_id) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at").bind(zone_id).bind(setting_id).bind(value.to_string()).bind(now()).execute(pool).await?;
    let provider = cf_request(
        pool,
        zone_id,
        "PATCH",
        &format!("/zones/{{zone}}/settings/{setting_id}"),
        Some(&json!({"value":value})),
    )
    .await?;
    Ok(
        json!({"success":true,"configured":provider["success"].as_bool()==Some(true),"setting":{"id":setting_id,"value":value},"provider":provider}),
    )
}
pub async fn apply_preset(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let preset = json!({"ssl":"full","always_use_https":"on","automatic_https_rewrites":"on","min_tls_version":"1.2","brotli":"on","security_level":"medium","cache_level":"aggressive"});
    let mut results = Vec::new();
    for k in setting_defaults() {
        results.push(update_setting(pool, zone_id, k, &json!({"value":preset[k]})).await?);
    }
    Ok(json!({"success":true,"results":results}))
}
pub async fn purge_cache(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    cf_request(pool, zone_id, "POST", "/zones/{zone}/purge_cache", Some(b)).await
}

fn waf_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"zone_id":r.get::<String,_>("zone_id"),"ruleset_id":r.try_get::<Option<String>,_>("ruleset_id").ok().flatten(),"rule_id":r.try_get::<Option<String>,_>("rule_id").ok().flatten(),"rule":j(Some(r.get::<String,_>("rule_json"))),"provider_status":r.get::<String,_>("provider_status"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn waf_rules(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_cf_waf_rules WHERE zone_id=? ORDER BY created_at DESC")
            .bind(zone_id)
            .fetch_all(pool)
            .await?;
    Ok(
        json!({"configured":cf_zone(pool,zone_id).await?.0,"rules":rows.iter().map(waf_value).collect::<Vec<_>>() }),
    )
}
pub async fn add_waf_rule(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_cf_waf_rules(id,zone_id,rule_json,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(&id).bind(zone_id).bind(b.to_string()).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"configured":cf_zone(pool,zone_id).await?.0,"rule":waf_rules(pool,zone_id).await?["rules"].as_array().and_then(|a|a.iter().find(|r|r["id"]==id)).cloned()}),
    )
}
pub async fn waf_preset(
    pool: &SqlitePool,
    zone_id: &str,
    preset: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    add_waf_rule(pool,zone_id,&json!({"preset":preset,"params":b.get("params").cloned().unwrap_or(Value::Null),"description":"ServerKit preset desired state"})).await
}
pub async fn update_waf_rule(
    pool: &SqlitePool,
    zone_id: &str,
    _ruleset_id: &str,
    rule_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let n=sqlx::query("UPDATE sk_cf_waf_rules SET rule_json=?, updated_at=? WHERE zone_id=? AND (id=? OR rule_id=?)").bind(b.to_string()).bind(now()).bind(zone_id).bind(rule_id).bind(rule_id).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}
pub async fn delete_waf_rule(
    pool: &SqlitePool,
    zone_id: &str,
    _ruleset_id: &str,
    rule_id: &str,
) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_cf_waf_rules WHERE zone_id=? AND (id=? OR rule_id=?)")
        .bind(zone_id)
        .bind(rule_id)
        .bind(rule_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}

fn row_resource(r: &sqlx::sqlite::SqliteRow, kind: &str) -> Value {
    json!({"id":r.get::<String,_>("id"),"zone_id":r.get::<String,_>("zone_id"),"kind":kind,"name":r.get::<String,_>("name"),"provider_status":r.get::<String,_>("provider_status"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn workers(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_cf_workers WHERE zone_id=? ORDER BY name")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    let routes = sqlx::query("SELECT * FROM sk_cf_worker_routes WHERE zone_id=? ORDER BY pattern")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"configured":cf_zone(pool,zone_id).await?.0,"workers":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"script":r.try_get::<Option<String>,_>("script").ok().flatten(),"metadata":j(Some(r.get::<String,_>("metadata_json"))),"provider_status":r.get::<String,_>("provider_status")})).collect::<Vec<_>>(),"routes":routes.iter().map(|r|json!({"id":r.get::<String,_>("id"),"pattern":r.get::<String,_>("pattern"),"script":r.get::<String,_>("script"),"provider_status":r.get::<String,_>("provider_status")})).collect::<Vec<_>>() }),
    )
}
pub async fn add_worker(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let name = s(b, "name", s(b, "script_name", "worker"));
    sqlx::query("INSERT INTO sk_cf_workers(id,zone_id,name,script,metadata_json,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(zone_id,name) DO UPDATE SET script=excluded.script,metadata_json=excluded.metadata_json,provider_status=excluded.provider_status,updated_at=excluded.updated_at").bind(&id).bind(zone_id).bind(name).bind(opt(b,"script")).bind(b.to_string()).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"configured":cf_zone(pool,zone_id).await?.0,"worker":{"name":name}}))
}
pub async fn delete_worker(pool: &SqlitePool, zone_id: &str, name: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_cf_workers WHERE zone_id=? AND name=?")
        .bind(zone_id)
        .bind(name)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}
pub async fn add_worker_route(
    pool: &SqlitePool,
    zone_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_cf_worker_routes(id,zone_id,pattern,script,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(zone_id).bind(s(b,"pattern","")).bind(s(b,"script","")).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"id":id,"configured":cf_zone(pool,zone_id).await?.0}))
}
pub async fn delete_worker_route(
    pool: &SqlitePool,
    zone_id: &str,
    route_id: &str,
) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_cf_worker_routes WHERE zone_id=? AND id=?")
        .bind(zone_id)
        .bind(route_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}

pub async fn tunnels(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_cf_tunnels WHERE zone_id=? ORDER BY name")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"configured":cf_zone(pool,zone_id).await?.0,"tunnels":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"provider_tunnel_id":r.try_get::<Option<String>,_>("provider_tunnel_id").ok().flatten(),"provider_status":r.get::<String,_>("provider_status")})).collect::<Vec<_>>() }),
    )
}
pub async fn add_tunnel(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let name = s(b, "name", "serverkit-tunnel");
    sqlx::query("INSERT INTO sk_cf_tunnels(id,zone_id,name,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(&id).bind(zone_id).bind(name).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"configured":cf_zone(pool,zone_id).await?.0,"tunnel":{"id":id,"name":name}}),
    )
}
pub async fn delete_tunnel(
    pool: &SqlitePool,
    zone_id: &str,
    tunnel_id: &str,
) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_cf_tunnel_hostnames WHERE tunnel_id=?")
        .bind(tunnel_id)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_cf_tunnels WHERE zone_id=? AND id=?")
        .bind(zone_id)
        .bind(tunnel_id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}
pub async fn tunnel_install(
    _pool: &SqlitePool,
    zone_id: &str,
    tunnel_id: &str,
) -> anyhow::Result<Value> {
    Ok(
        json!({"zone_id":zone_id,"tunnel_id":tunnel_id,"commands":["cloudflared tunnel login","cloudflared tunnel run <tunnel>"],"systemd_unit":"cloudflared.service"}),
    )
}
pub async fn tunnel_hostnames(
    pool: &SqlitePool,
    zone_id: &str,
    tunnel_id: &str,
) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_cf_tunnel_hostnames WHERE zone_id=? AND tunnel_id=? ORDER BY hostname",
    )
    .bind(zone_id)
    .bind(tunnel_id)
    .fetch_all(pool)
    .await?;
    Ok(
        json!({"configured":cf_zone(pool,zone_id).await?.0,"hostnames":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"hostname":r.get::<String,_>("hostname"),"service":r.get::<String,_>("service"),"provider_status":r.get::<String,_>("provider_status")})).collect::<Vec<_>>() }),
    )
}
pub async fn add_tunnel_hostname(
    pool: &SqlitePool,
    zone_id: &str,
    tunnel_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_cf_tunnel_hostnames(id,zone_id,tunnel_id,hostname,service,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(tunnel_id,hostname) DO UPDATE SET service=excluded.service,provider_status=excluded.provider_status,updated_at=excluded.updated_at").bind(&id).bind(zone_id).bind(tunnel_id).bind(s(b,"hostname","")).bind(s(b,"service","")).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"configured":cf_zone(pool,zone_id).await?.0,"id":id}))
}
pub async fn delete_tunnel_hostname(
    pool: &SqlitePool,
    zone_id: &str,
    tunnel_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let n = sqlx::query(
        "DELETE FROM sk_cf_tunnel_hostnames WHERE zone_id=? AND tunnel_id=? AND hostname=?",
    )
    .bind(zone_id)
    .bind(tunnel_id)
    .bind(s(b, "hostname", ""))
    .execute(pool)
    .await?
    .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}

pub async fn storage(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_cf_storage WHERE zone_id=? ORDER BY kind,name")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"configured":cf_zone(pool,zone_id).await?.0,"items":rows.iter().map(|r|row_resource(r,&r.get::<String,_>("kind"))).collect::<Vec<_>>() }),
    )
}
pub async fn add_storage(
    pool: &SqlitePool,
    zone_id: &str,
    kind: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let name = s(b, "name", s(b, "title", "resource"));
    sqlx::query("INSERT INTO sk_cf_storage(id,zone_id,kind,name,provider_status,created_at,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(zone_id,kind,name) DO UPDATE SET provider_status=excluded.provider_status,updated_at=excluded.updated_at").bind(&id).bind(zone_id).bind(kind).bind(name).bind(if cf_zone(pool,zone_id).await?.0{"pending-provider"}else{"local-unconfigured"}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"configured":cf_zone(pool,zone_id).await?.0,"resource":{"id":id,"kind":kind,"name":name}}),
    )
}
pub async fn delete_storage(
    pool: &SqlitePool,
    zone_id: &str,
    kind: &str,
    name: &str,
) -> anyhow::Result<Value> {
    let n =
        sqlx::query("DELETE FROM sk_cf_storage WHERE zone_id=? AND kind=? AND (name=? OR id=?)")
            .bind(zone_id)
            .bind(kind)
            .bind(name)
            .bind(name)
            .execute(pool)
            .await?
            .rows_affected();
    Ok(json!({"success":n>0,"configured":cf_zone(pool,zone_id).await?.0}))
}
