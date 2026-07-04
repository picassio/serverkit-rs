use anyhow::Context;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration as StdDuration, Instant};
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
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_status_pages(id TEXT PRIMARY KEY, name TEXT NOT NULL, slug TEXT NOT NULL UNIQUE, description TEXT, public INTEGER NOT NULL DEFAULT 1, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_status_components(id TEXT PRIMARY KEY, page_id TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, target TEXT, status TEXT NOT NULL, component_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_status_checks(id TEXT PRIMARY KEY, component_id TEXT NOT NULL, status TEXT NOT NULL, latency_ms INTEGER, result_json TEXT NOT NULL DEFAULT '{}', checked_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_status_incidents(id TEXT PRIMARY KEY, page_id TEXT NOT NULL, title TEXT NOT NULL, status TEXT NOT NULL, severity TEXT NOT NULL, message TEXT, incident_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_uptime_config(id INTEGER PRIMARY KEY CHECK(id=1), tracking INTEGER NOT NULL DEFAULT 0, started_at TEXT, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_uptime_samples(id TEXT PRIMARY KEY, status TEXT NOT NULL, uptime_seconds INTEGER NOT NULL, load_json TEXT NOT NULL DEFAULT '{}', sampled_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-status schema")?;
    Ok(())
}

fn page_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"slug":r.get::<String,_>("slug"),"description":r.get::<Option<String>,_>("description"),"public":r.get::<i64,_>("public")!=0,"metadata":j(Some(r.get("metadata_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn component_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"page_id":r.get::<String,_>("page_id"),"name":r.get::<String,_>("name"),"kind":r.get::<String,_>("kind"),"target":r.get::<Option<String>,_>("target"),"status":r.get::<String,_>("status"),"config":j(Some(r.get("component_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn incident_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"page_id":r.get::<String,_>("page_id"),"title":r.get::<String,_>("title"),"status":r.get::<String,_>("status"),"severity":r.get::<String,_>("severity"),"message":r.get::<Option<String>,_>("message"),"details":j(Some(r.get("incident_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
async fn page_by_id_or_slug(pool: &SqlitePool, id_or_slug: &str) -> anyhow::Result<Option<Value>> {
    Ok(
        sqlx::query("SELECT * FROM sk_status_pages WHERE id=? OR slug=?")
            .bind(id_or_slug)
            .bind(id_or_slug)
            .fetch_optional(pool)
            .await?
            .map(page_row),
    )
}

pub async fn pages(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_status_pages ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let vals: Vec<Value> = rows.into_iter().map(page_row).collect();
    Ok(json!({"pages":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_page(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let pid = id();
    let ts = now();
    let name = s(b, "name", "Status page");
    let slug = opt(b, "slug").map(str::to_string).unwrap_or_else(|| {
        name.to_lowercase()
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
            .collect::<String>()
            .trim_matches('-')
            .to_string()
    });
    sqlx::query("INSERT INTO sk_status_pages(id,name,slug,description,public,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(&pid).bind(name).bind(&slug).bind(opt(b,"description")).bind(if b.get("public").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"page":get_page(pool,&pid).await?["page"].clone()}))
}
pub async fn get_page(pool: &SqlitePool, id_or_slug: &str) -> anyhow::Result<Value> {
    let Some(page) = page_by_id_or_slug(pool, id_or_slug).await? else {
        return Ok(
            json!({"success":false,"code":"STATUS_PAGE_NOT_FOUND","error":"Status page not found"}),
        );
    };
    let comps = components(pool, page["id"].as_str().unwrap_or("")).await?["components"].clone();
    let incidents = incidents(pool, page["id"].as_str().unwrap_or("")).await?["incidents"].clone();
    Ok(json!({"page":page,"components":comps,"incidents":incidents}))
}
pub async fn update_page(pool: &SqlitePool, id: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_status_pages WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(
            json!({"success":false,"code":"STATUS_PAGE_NOT_FOUND","error":"Status page not found"}),
        );
    };
    let old_name: String = r.get("name");
    let old_slug: String = r.get("slug");
    let old_desc: Option<String> = r.get("description");
    sqlx::query("UPDATE sk_status_pages SET name=?,slug=?,description=?,public=?,metadata_json=?,updated_at=? WHERE id=?")
        .bind(s(b,"name",&old_name)).bind(s(b,"slug",&old_slug)).bind(opt(b,"description").map(str::to_string).or(old_desc)).bind(if b.get("public").and_then(Value::as_bool).unwrap_or(r.get::<i64,_>("public")!=0){1}else{0}).bind(b.to_string()).bind(now()).bind(id).execute(pool).await?;
    Ok(json!({"success":true,"page":get_page(pool,id).await?["page"].clone()}))
}
pub async fn delete_page(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let cids = sqlx::query("SELECT id FROM sk_status_components WHERE page_id=?")
        .bind(id)
        .fetch_all(pool)
        .await?;
    for r in cids {
        sqlx::query("DELETE FROM sk_status_checks WHERE component_id=?")
            .bind(r.get::<String, _>("id"))
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM sk_status_components WHERE page_id=?")
        .bind(id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_status_incidents WHERE page_id=?")
        .bind(id)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_status_pages WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn public_page(pool: &SqlitePool, slug: &str) -> anyhow::Result<Value> {
    get_page(pool, slug).await
}
pub async fn badge(pool: &SqlitePool, slug: &str) -> anyhow::Result<Value> {
    let p = get_page(pool, slug).await?;
    let comps = p["components"].as_array().cloned().unwrap_or_default();
    let degraded = comps.iter().any(|c| c["status"] != "operational");
    Ok(
        json!({"schemaVersion":1,"label":slug,"message":if degraded{"degraded"}else{"operational"},"color":if degraded{"yellow"}else{"brightgreen"},"page":p.get("page").cloned().unwrap_or(Value::Null)}),
    )
}

pub async fn components(pool: &SqlitePool, page_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_status_components WHERE page_id=? ORDER BY created_at ASC")
            .bind(page_id)
            .fetch_all(pool)
            .await?;
    let vals: Vec<Value> = rows.into_iter().map(component_row).collect();
    Ok(json!({"components":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_component(
    pool: &SqlitePool,
    page_id: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let cid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_status_components(id,page_id,name,kind,target,status,component_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(&cid).bind(page_id).bind(s(b,"name","Component")).bind(s(b,"kind","http")).bind(opt(b,"target").or_else(||opt(b,"url"))).bind(s(b,"status","unknown")).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"component":component(pool,&cid).await?}))
}
async fn component(pool: &SqlitePool, cid: &str) -> anyhow::Result<Value> {
    Ok(sqlx::query("SELECT * FROM sk_status_components WHERE id=?")
        .bind(cid)
        .fetch_one(pool)
        .await
        .map(component_row)?)
}
pub async fn update_component(pool: &SqlitePool, cid: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_status_components WHERE id=?")
        .bind(cid)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(
            json!({"success":false,"code":"COMPONENT_NOT_FOUND","error":"Component not found"}),
        );
    };
    let name = s(b, "name", &r.get::<String, _>("name")).to_string();
    let kind = s(b, "kind", &r.get::<String, _>("kind")).to_string();
    let target = opt(b, "target")
        .or_else(|| opt(b, "url"))
        .map(str::to_string)
        .or(r.get::<Option<String>, _>("target"));
    let status = s(b, "status", &r.get::<String, _>("status")).to_string();
    sqlx::query("UPDATE sk_status_components SET name=?,kind=?,target=?,status=?,component_json=?,updated_at=? WHERE id=?").bind(name).bind(kind).bind(target).bind(status).bind(b.to_string()).bind(now()).bind(cid).execute(pool).await?;
    Ok(json!({"success":true,"component":component(pool,cid).await?}))
}
pub async fn delete_component(pool: &SqlitePool, cid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_status_checks WHERE component_id=?")
        .bind(cid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_status_components WHERE id=?")
        .bind(cid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}

pub async fn run_check(pool: &SqlitePool, cid: &str) -> anyhow::Result<Value> {
    let c = component(pool, cid).await?;
    let kind = c["kind"].as_str().unwrap_or("http");
    let target = c["target"].as_str().unwrap_or("");
    let start = Instant::now();
    let (status, result) = if target.is_empty() {
        (
            "unknown".to_string(),
            json!({"success":false,"error":"No target configured"}),
        )
    } else if kind == "http" || target.starts_with("http://") || target.starts_with("https://") {
        match reqwest::Client::new()
            .get(target)
            .timeout(StdDuration::from_secs(10))
            .send()
            .await
        {
            Ok(r) => (
                if r.status().is_success() {
                    "operational"
                } else {
                    "degraded"
                }
                .to_string(),
                json!({"success":r.status().is_success(),"status_code":r.status().as_u16()}),
            ),
            Err(e) => (
                "outage".to_string(),
                json!({"success":false,"error":e.to_string()}),
            ),
        }
    } else if kind == "tcp" {
        let ok = target
            .to_socket_addrs()
            .ok()
            .and_then(|mut a| a.next())
            .map(|addr| TcpStream::connect_timeout(&addr, StdDuration::from_secs(5)).is_ok())
            .unwrap_or(false);
        (
            if ok { "operational" } else { "outage" }.to_string(),
            json!({"success":ok}),
        )
    } else {
        (
            c["status"].as_str().unwrap_or("unknown").to_string(),
            json!({"success":true,"note":"Manual component; status preserved"}),
        )
    };
    let latency = start.elapsed().as_millis().min(i64::MAX as u128) as i64;
    let check_id = id();
    sqlx::query("INSERT INTO sk_status_checks(id,component_id,status,latency_ms,result_json,checked_at) VALUES(?,?,?,?,?,?)").bind(&check_id).bind(cid).bind(&status).bind(latency).bind(result.to_string()).bind(now()).execute(pool).await?;
    sqlx::query("UPDATE sk_status_components SET status=?,updated_at=? WHERE id=?")
        .bind(&status)
        .bind(now())
        .bind(cid)
        .execute(pool)
        .await?;
    Ok(
        json!({"success":true,"check":{"id":check_id,"component_id":cid,"status":status,"latency_ms":latency,"result":result}}),
    )
}
pub async fn history(pool: &SqlitePool, cid: &str, hours: i64) -> anyhow::Result<Value> {
    let cutoff = (Utc::now() - Duration::hours(hours.clamp(1, 24 * 365))).to_rfc3339();
    let rows=sqlx::query("SELECT * FROM sk_status_checks WHERE component_id=? AND checked_at>=? ORDER BY checked_at DESC").bind(cid).bind(cutoff).fetch_all(pool).await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"component_id":r.get::<String,_>("component_id"),"status":r.get::<String,_>("status"),"latency_ms":r.get::<Option<i64>,_>("latency_ms"),"result":j(Some(r.get("result_json"))),"checked_at":r.get::<String,_>("checked_at")})).collect();
    Ok(json!({"history":vals,"items":vals,"count":vals.len()}))
}

pub async fn incidents(pool: &SqlitePool, page_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_status_incidents WHERE page_id=? ORDER BY created_at DESC")
            .bind(page_id)
            .fetch_all(pool)
            .await?;
    let vals: Vec<Value> = rows.into_iter().map(incident_row).collect();
    Ok(json!({"incidents":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_incident(pool: &SqlitePool, page_id: &str, b: &Value) -> anyhow::Result<Value> {
    let iid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_status_incidents(id,page_id,title,status,severity,message,incident_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(&iid).bind(page_id).bind(s(b,"title","Incident")).bind(s(b,"status","investigating")).bind(s(b,"severity","minor")).bind(opt(b,"message")).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"incident":incident(pool,&iid).await?}))
}
async fn incident(pool: &SqlitePool, iid: &str) -> anyhow::Result<Value> {
    Ok(sqlx::query("SELECT * FROM sk_status_incidents WHERE id=?")
        .bind(iid)
        .fetch_one(pool)
        .await
        .map(incident_row)?)
}
pub async fn update_incident(pool: &SqlitePool, iid: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_status_incidents WHERE id=?")
        .bind(iid)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(
            json!({"success":false,"code":"INCIDENT_NOT_FOUND","error":"Incident not found"}),
        );
    };
    let title = s(b, "title", &r.get::<String, _>("title")).to_string();
    let status = s(b, "status", &r.get::<String, _>("status")).to_string();
    let severity = s(b, "severity", &r.get::<String, _>("severity")).to_string();
    let message = opt(b, "message")
        .map(str::to_string)
        .or(r.get::<Option<String>, _>("message"));
    sqlx::query("UPDATE sk_status_incidents SET title=?,status=?,severity=?,message=?,incident_json=?,updated_at=? WHERE id=?").bind(title).bind(status).bind(severity).bind(message).bind(b.to_string()).bind(now()).bind(iid).execute(pool).await?;
    Ok(json!({"success":true,"incident":incident(pool,iid).await?}))
}
pub async fn delete_incident(pool: &SqlitePool, iid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_status_incidents WHERE id=?")
        .bind(iid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}

pub async fn apps_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT id,name,status,app_type,root_path,domains_json FROM sk_apps ORDER BY name",
    )
    .fetch_all(pool)
    .await
    .unwrap_or_default();
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"app_type":r.get::<String,_>("app_type"),"root_path":r.get::<Option<String>,_>("root_path"),"domains":j(Some(r.get("domains_json")))})).collect();
    Ok(json!({"apps":vals,"items":vals,"count":vals.len()}))
}
pub async fn app_status(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT * FROM sk_apps WHERE id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await
        .unwrap_or(None);
    Ok(match row {
        Some(r) => {
            json!({"app":{"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"app_type":r.get::<String,_>("app_type"),"root_path":r.get::<Option<String>,_>("root_path")},"containers":[],"source":"sk_apps"})
        }
        None => json!({"success":false,"code":"APP_NOT_FOUND","error":"Application not found"}),
    })
}

fn uptime_seconds() -> i64 {
    std::fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|s| {
            s.split_whitespace()
                .next()
                .and_then(|x| x.parse::<f64>().ok())
        })
        .map(|x| x as i64)
        .unwrap_or(0)
}
fn load() -> Value {
    std::fs::read_to_string("/proc/loadavg").map(|s|{let p:Vec<_>=s.split_whitespace().collect(); json!({"1m":p.first().copied().unwrap_or("0"),"5m":p.get(1).copied().unwrap_or("0"),"15m":p.get(2).copied().unwrap_or("0")})}).unwrap_or_else(|_|json!({}))
}
pub async fn sample_uptime(pool: &SqlitePool) -> anyhow::Result<Value> {
    let up = uptime_seconds();
    let status = if up > 0 { "up" } else { "unknown" };
    let sid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_uptime_samples(id,status,uptime_seconds,load_json,sampled_at) VALUES(?,?,?,?,?)").bind(&sid).bind(status).bind(up).bind(load().to_string()).bind(&ts).execute(pool).await.ok();
    Ok(json!({"id":sid,"status":status,"uptime_seconds":up,"load":load(),"sampled_at":ts}))
}
pub async fn uptime_current(pool: &SqlitePool) -> anyhow::Result<Value> {
    let sample = sample_uptime(pool).await?;
    Ok(
        json!({"current":sample,"boot_time_epoch":chrono::Utc::now().timestamp()-sample["uptime_seconds"].as_i64().unwrap_or(0)}),
    )
}
pub async fn uptime_stats(pool: &SqlitePool) -> anyhow::Result<Value> {
    let sample = sample_uptime(pool).await?;
    let total = sqlx::query("SELECT COUNT(*) count FROM sk_uptime_samples")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("count"))
        .unwrap_or(0);
    Ok(
        json!({"status":sample["status"],"uptime_seconds":sample["uptime_seconds"],"samples":total,"availability_percent":100.0}),
    )
}
pub async fn uptime_history(pool: &SqlitePool, hours: i64) -> anyhow::Result<Value> {
    let _ = sample_uptime(pool).await;
    let cutoff = (Utc::now() - Duration::hours(hours.clamp(1, 24 * 365))).to_rfc3339();
    let rows =
        sqlx::query("SELECT * FROM sk_uptime_samples WHERE sampled_at>=? ORDER BY sampled_at DESC")
            .bind(cutoff)
            .fetch_all(pool)
            .await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"status":r.get::<String,_>("status"),"uptime_seconds":r.get::<i64,_>("uptime_seconds"),"load":j(Some(r.get("load_json"))),"sampled_at":r.get::<String,_>("sampled_at")})).collect();
    Ok(json!({"history":vals,"items":vals,"count":vals.len()}))
}
pub async fn uptime_graph(pool: &SqlitePool, period: &str) -> anyhow::Result<Value> {
    let hours = match period {
        "1h" => 1,
        "24h" => 24,
        "7d" => 24 * 7,
        "30d" => 24 * 30,
        _ => 24,
    };
    let h = uptime_history(pool, hours).await?;
    Ok(json!({"period":period,"points":h["history"].clone()}))
}
pub async fn tracking_start(pool: &SqlitePool) -> anyhow::Result<Value> {
    let ts = now();
    sqlx::query("INSERT INTO sk_uptime_config(id,tracking,started_at,updated_at) VALUES(1,1,?,?) ON CONFLICT(id) DO UPDATE SET tracking=1,started_at=excluded.started_at,updated_at=excluded.updated_at").bind(&ts).bind(&ts).execute(pool).await?;
    let sample = sample_uptime(pool).await?;
    Ok(json!({"success":true,"tracking":true,"started_at":ts,"sample":sample}))
}
pub async fn tracking_stop(pool: &SqlitePool) -> anyhow::Result<Value> {
    let ts = now();
    sqlx::query("INSERT INTO sk_uptime_config(id,tracking,updated_at) VALUES(1,0,?) ON CONFLICT(id) DO UPDATE SET tracking=0,updated_at=excluded.updated_at").bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"tracking":false,"stopped_at":ts}))
}
pub async fn tracking_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_uptime_config WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            json!({"tracking":r.get::<i64,_>("tracking")!=0,"started_at":r.get::<Option<String>,_>("started_at"),"updated_at":r.get::<String,_>("updated_at")})
        }
        None => json!({"tracking":false}),
    })
}
