use anyhow::Context;
use chrono::{Duration, Utc};
use rand::Rng;
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
fn arr(v: &Value, k: &str) -> Value {
    v.get(k)
        .filter(|x| x.is_array())
        .cloned()
        .unwrap_or_else(|| json!([]))
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_pairing_codes(code TEXT PRIMARY KEY, passphrase TEXT NOT NULL, agent_json TEXT NOT NULL DEFAULT '{}', status TEXT NOT NULL DEFAULT 'pending', server_id TEXT, expires_at TEXT NOT NULL, created_at TEXT NOT NULL, claimed_at TEXT);
CREATE TABLE IF NOT EXISTS sk_tunnels(id TEXT PRIMARY KEY, edge_server_id TEXT NOT NULL, private_server_id TEXT NOT NULL, name TEXT NOT NULL, status TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_tunnel_services(id TEXT PRIMARY KEY, tunnel_id TEXT NOT NULL, hostname TEXT NOT NULL, port INTEGER NOT NULL, service_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, UNIQUE(tunnel_id,hostname));
CREATE TABLE IF NOT EXISTS sk_server_templates(id TEXT PRIMARY KEY, name TEXT NOT NULL, category TEXT NOT NULL, template_json TEXT NOT NULL DEFAULT '{}', source TEXT NOT NULL DEFAULT 'custom', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_server_template_assignments(id TEXT PRIMARY KEY, template_id TEXT NOT NULL, server_id TEXT NOT NULL, status TEXT NOT NULL, drift_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL, UNIQUE(template_id,server_id));
CREATE TABLE IF NOT EXISTS sk_fleet_thresholds(id TEXT PRIMARY KEY, server_id TEXT, metric TEXT NOT NULL, operator TEXT NOT NULL, value REAL NOT NULL, severity TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_fleet_alerts(id TEXT PRIMARY KEY, server_id TEXT, metric TEXT NOT NULL, severity TEXT NOT NULL, status TEXT NOT NULL, message TEXT NOT NULL, value REAL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-fleet schema")?;
    Ok(())
}

async fn server_exists(pool: &SqlitePool, server_id: &str) -> anyhow::Result<bool> {
    if server_id == "local" {
        return Ok(true);
    }
    let exists = sqlx::query("SELECT 1 FROM sk_servers WHERE id=? LIMIT 1")
        .bind(server_id)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some();
    Ok(exists)
}
fn server_ref(server_id: &str) -> Value {
    if server_id == "local" {
        json!({"id":"local","name":"Local (this server)","status":"online","is_local":true})
    } else {
        json!({"id":server_id,"status":"agent_offline","is_local":false})
    }
}

pub async fn lookup_pairing(pool: &SqlitePool, code: &str) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT * FROM sk_pairing_codes WHERE code=?")
        .bind(code)
        .fetch_optional(pool)
        .await?;
    let Some(r) = row else {
        return Ok(json!({"found":false,"claimable":false,"error":"Pairing code not found"}));
    };
    let status: String = r.get("status");
    let expires_at: String = r.get("expires_at");
    let expired = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map(|d| d.with_timezone(&Utc) < Utc::now())
        .unwrap_or(true);
    Ok(
        json!({"found":true,"claimable":status=="pending" && !expired,"status":status,"expires_at":expires_at,"agent":j(Some(r.get("agent_json")))}),
    )
}
pub async fn claim_pairing(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let code = s(body, "code", "");
    let passphrase = s(body, "passphrase", "");
    let row = sqlx::query("SELECT * FROM sk_pairing_codes WHERE code=?")
        .bind(code)
        .fetch_optional(pool)
        .await?;
    let Some(r) = row else {
        return Ok(
            json!({"success":false,"code":"PAIRING_NOT_FOUND","error":"Pairing code not found"}),
        );
    };
    if r.get::<String, _>("passphrase") != passphrase {
        return Ok(
            json!({"success":false,"code":"PAIRING_DENIED","error":"Invalid pairing passphrase"}),
        );
    }
    let expires_at: String = r.get("expires_at");
    let expired = chrono::DateTime::parse_from_rfc3339(&expires_at)
        .map(|d| d.with_timezone(&Utc) < Utc::now())
        .unwrap_or(true);
    if expired {
        return Ok(
            json!({"success":false,"code":"PAIRING_EXPIRED","error":"Pairing code expired"}),
        );
    }
    let server_id = id();
    let name = s(body, "name", "Paired agent");
    sqlx::query(
        "UPDATE sk_pairing_codes SET status='claimed', server_id=?, claimed_at=? WHERE code=?",
    )
    .bind(&server_id)
    .bind(now())
    .bind(code)
    .execute(pool)
    .await?;
    Ok(
        json!({"success":true,"server":{"id":server_id,"name":name,"status":"pending_registration","agent_offline":true},"message":"Pairing claimed; awaiting agent registration"}),
    )
}
pub async fn seed_pairing(pool: &SqlitePool, agent: Value) -> anyhow::Result<Value> {
    let code = format!("{:06}", rand::thread_rng().gen_range(0..1_000_000));
    let passphrase = id();
    let expires_at = (Utc::now() + Duration::minutes(15)).to_rfc3339();
    sqlx::query("INSERT INTO sk_pairing_codes(code,passphrase,agent_json,status,expires_at,created_at) VALUES(?,?,?,?,?,?)")
        .bind(&code).bind(&passphrase).bind(agent.to_string()).bind("pending").bind(&expires_at).bind(now()).execute(pool).await?;
    Ok(json!({"code":code,"passphrase":passphrase,"expires_at":expires_at}))
}

pub async fn tunnels(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_tunnels ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let mut out = Vec::new();
    for r in rows {
        out.push(tunnel_row(pool, &r.get::<String, _>("id")).await?);
    }
    Ok(json!({"tunnels":out,"items":out,"count":out.len()}))
}
async fn tunnel_row(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_tunnels WHERE id=?")
        .bind(tid)
        .fetch_one(pool)
        .await?;
    let services = tunnel_services(pool, tid).await?["services"].clone();
    Ok(
        json!({"id":r.get::<String,_>("id"),"edge_server_id":r.get::<String,_>("edge_server_id"),"private_server_id":r.get::<String,_>("private_server_id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"configured":false,"backend":"wireguard-desired-state","config":j(Some(r.get("config_json"))),"services":services,"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")}),
    )
}
pub async fn get_tunnel(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    Ok(json!({"tunnel":tunnel_row(pool, tid).await?}))
}
pub async fn create_tunnel(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let edge = s(b, "edge_server_id", "");
    let private = s(b, "private_server_id", "");
    if edge.is_empty() || private.is_empty() {
        return Ok(
            json!({"success":false,"code":"VALIDATION_ERROR","error":"edge_server_id and private_server_id are required"}),
        );
    }
    if !server_exists(pool, edge).await? || !server_exists(pool, private).await? {
        return Ok(
            json!({"success":false,"code":"SERVER_NOT_FOUND","error":"Both tunnel endpoints must reference known servers or local"}),
        );
    }
    let tid = id();
    let ts = now();
    let name = s(b, "name", "Remote access tunnel");
    let cfg = json!({"wireguard":{"configured":false,"reason":"Agent handshake has not provisioned WireGuard yet"}});
    sqlx::query("INSERT INTO sk_tunnels(id,edge_server_id,private_server_id,name,status,config_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(&tid).bind(edge).bind(private).bind(name).bind("pending_agent").bind(cfg.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"tunnel":tunnel_row(pool,&tid).await?}))
}
pub async fn delete_tunnel(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_tunnel_services WHERE tunnel_id=?")
        .bind(tid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_tunnels WHERE id=?")
        .bind(tid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn tunnel_services(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_tunnel_services WHERE tunnel_id=? ORDER BY created_at DESC")
            .bind(tid)
            .fetch_all(pool)
            .await?;
    let services: Vec<Value> = rows.into_iter().map(|r| json!({"id":r.get::<String,_>("id"),"tunnel_id":r.get::<String,_>("tunnel_id"),"hostname":r.get::<String,_>("hostname"),"port":r.get::<i64,_>("port"),"configured":false,"status":"pending_proxy","details":j(Some(r.get("service_json"))),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"services":services,"count":services.len()}))
}
pub async fn publish_service(pool: &SqlitePool, tid: &str, b: &Value) -> anyhow::Result<Value> {
    if sqlx::query("SELECT 1 FROM sk_tunnels WHERE id=?")
        .bind(tid)
        .fetch_optional(pool)
        .await?
        .is_none()
    {
        return Ok(json!({"success":false,"code":"TUNNEL_NOT_FOUND","error":"Tunnel not found"}));
    }
    let hostname = s(b, "hostname", "");
    let port = b.get("port").and_then(Value::as_i64).unwrap_or(0);
    if hostname.is_empty() || port <= 0 {
        return Ok(
            json!({"success":false,"code":"VALIDATION_ERROR","error":"hostname and port are required"}),
        );
    }
    let sid = id();
    sqlx::query("INSERT INTO sk_tunnel_services(id,tunnel_id,hostname,port,service_json,created_at) VALUES(?,?,?,?,?,?)")
        .bind(&sid).bind(tid).bind(hostname).bind(port).bind(b.to_string()).bind(now()).execute(pool).await?;
    Ok(
        json!({"success":true,"service":{"id":sid,"tunnel_id":tid,"hostname":hostname,"port":port,"status":"pending_proxy"}}),
    )
}
pub async fn unpublish_service(pool: &SqlitePool, tid: &str, sid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_tunnel_services WHERE tunnel_id=? AND id=?")
        .bind(tid)
        .bind(sid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}

fn library_items() -> Vec<Value> {
    vec![
        json!({"key":"ubuntu-baseline","name":"Ubuntu baseline","category":"security","description":"UFW, unattended-upgrades, fail2ban desired-state baseline","spec":{"packages":["ufw","fail2ban","unattended-upgrades"],"services":["ufw","fail2ban"]}}),
        json!({"key":"docker-host","name":"Docker host","category":"runtime","description":"Docker Engine host desired-state checks","spec":{"packages":["docker.io"],"groups":["docker"]}}),
        json!({"key":"web-edge","name":"Web edge","category":"web","description":"Nginx edge server desired-state checks","spec":{"packages":["nginx"],"ports":[80,443]}}),
    ]
}
pub async fn template_library() -> anyhow::Result<Value> {
    Ok(json!({"library":library_items(),"items":library_items()}))
}
pub async fn create_template_from_library(pool: &SqlitePool, key: &str) -> anyhow::Result<Value> {
    let Some(item) = library_items().into_iter().find(|x| x["key"] == key) else {
        return Ok(
            json!({"success":false,"code":"LIBRARY_TEMPLATE_NOT_FOUND","error":"Library template not found"}),
        );
    };
    create_template(pool, &json!({"name":item["name"],"category":item["category"],"spec":item["spec"],"source":"library","library_key":key})).await
}
pub async fn templates(pool: &SqlitePool, category: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(c) = category {
        sqlx::query("SELECT * FROM sk_server_templates WHERE category=? ORDER BY created_at DESC")
            .bind(c)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_server_templates ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    let items: Vec<Value> = rows.into_iter().map(|r| json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"category":r.get::<String,_>("category"),"source":r.get::<String,_>("source"),"template":j(Some(r.get("template_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})).collect();
    Ok(json!({"templates":items,"items":items,"count":items.len()}))
}
pub async fn get_template(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_server_templates WHERE id=?")
        .bind(tid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            json!({"template":{"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"category":r.get::<String,_>("category"),"source":r.get::<String,_>("source"),"template":j(Some(r.get("template_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")}})
        }
        None => json!({"success":false,"code":"TEMPLATE_NOT_FOUND","error":"Template not found"}),
    })
}
pub async fn create_template(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let tid = id();
    let ts = now();
    let name = s(b, "name", "Server template");
    let category = s(b, "category", "general");
    let source = s(b, "source", "custom");
    sqlx::query("INSERT INTO sk_server_templates(id,name,category,template_json,source,created_at,updated_at) VALUES(?,?,?,?,?,?,?)")
        .bind(&tid).bind(name).bind(category).bind(b.to_string()).bind(source).bind(&ts).bind(&ts).execute(pool).await?;
    get_template(pool, &tid)
        .await
        .map(|v| json!({"success":true,"template":v["template"].clone()}))
}
pub async fn update_template(pool: &SqlitePool, tid: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_server_templates WHERE id=?")
        .bind(tid)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(
            json!({"success":false,"code":"TEMPLATE_NOT_FOUND","error":"Template not found"}),
        );
    };
    let old_name: String = r.get("name");
    let old_category: String = r.get("category");
    let name = s(b, "name", &old_name);
    let category = s(b, "category", &old_category);
    sqlx::query(
        "UPDATE sk_server_templates SET name=?,category=?,template_json=?,updated_at=? WHERE id=?",
    )
    .bind(name)
    .bind(category)
    .bind(b.to_string())
    .bind(now())
    .bind(tid)
    .execute(pool)
    .await?;
    get_template(pool, tid)
        .await
        .map(|v| json!({"success":true,"template":v["template"].clone()}))
}
pub async fn delete_template(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_server_template_assignments WHERE template_id=?")
        .bind(tid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_server_templates WHERE id=?")
        .bind(tid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn assign_template(
    pool: &SqlitePool,
    tid: &str,
    server_id: &str,
) -> anyhow::Result<Value> {
    if !server_exists(pool, server_id).await? {
        return Ok(json!({"success":false,"code":"SERVER_NOT_FOUND","error":"Server not found"}));
    }
    let aid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_server_template_assignments(id,template_id,server_id,status,drift_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(template_id,server_id) DO UPDATE SET status='assigned',updated_at=excluded.updated_at")
        .bind(&aid).bind(tid).bind(server_id).bind("assigned").bind(json!({"checked":false,"reason":"No agent check has run yet"}).to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"assignment":{"id":aid,"template_id":tid,"server_id":server_id,"status":"assigned"}}),
    )
}
pub async fn bulk_assign_template(
    pool: &SqlitePool,
    tid: &str,
    server_ids: Value,
) -> anyhow::Result<Value> {
    let mut assigned = Vec::new();
    if let Some(ids) = server_ids.as_array() {
        for sid in ids.iter().filter_map(Value::as_str) {
            assigned.push(assign_template(pool, tid, sid).await?);
        }
    }
    Ok(json!({"success":true,"results":assigned}))
}
pub async fn template_assignments(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_server_template_assignments WHERE template_id=? ORDER BY created_at DESC",
    )
    .bind(tid)
    .fetch_all(pool)
    .await?;
    Ok(
        json!({"assignments":assignment_values(rows),"count":assignment_values(sqlx::query("SELECT * FROM sk_server_template_assignments WHERE template_id=? ORDER BY created_at DESC").bind(tid).fetch_all(pool).await?).len()}),
    )
}
pub async fn server_template_assignments(
    pool: &SqlitePool,
    server_id: &str,
) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_server_template_assignments WHERE server_id=? ORDER BY created_at DESC",
    )
    .bind(server_id)
    .fetch_all(pool)
    .await?;
    let vals = assignment_values(rows);
    Ok(json!({"assignments":vals,"count":vals.len()}))
}
fn assignment_values(rows: Vec<sqlx::sqlite::SqliteRow>) -> Vec<Value> {
    rows.into_iter().map(|r| json!({"id":r.get::<String,_>("id"),"template_id":r.get::<String,_>("template_id"),"server_id":r.get::<String,_>("server_id"),"status":r.get::<String,_>("status"),"drift":j(Some(r.get("drift_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})).collect()
}
pub async fn unassign_template(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_server_template_assignments WHERE id=?")
        .bind(aid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn check_assignment(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    let drift = json!({"checked":true,"compliant":false,"reason":"Agent offline; desired-state check queued for next heartbeat","checked_at":now()});
    let n=sqlx::query("UPDATE sk_server_template_assignments SET status='pending_agent',drift_json=?,updated_at=? WHERE id=?").bind(drift.to_string()).bind(now()).bind(aid).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"status":"pending_agent","drift":drift}))
}
pub async fn remediate_assignment(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    let drift = json!({"remediation":"queued","configured":false,"reason":"Agent offline; remediation requires a connected fleet agent","queued_at":now()});
    let n=sqlx::query("UPDATE sk_server_template_assignments SET status='remediation_pending',drift_json=?,updated_at=? WHERE id=?").bind(drift.to_string()).bind(now()).bind(aid).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"configured":false,"code":"AGENT_OFFLINE","drift":drift}))
}
pub async fn template_compliance(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT status, COUNT(*) count FROM sk_server_template_assignments GROUP BY status",
    )
    .fetch_all(pool)
    .await?;
    let by_status: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({"status":r.get::<String,_>("status"),"count":r.get::<i64,_>("count")}))
        .collect();
    Ok(json!({"summary":by_status,"checked_by_agent":false}))
}

pub async fn heatmap(pool: &SqlitePool, _group: Option<&str>) -> anyhow::Result<Value> {
    Ok(
        json!({"servers":[server_ref("local")],"metrics":[{"server_id":"local","cpu":0,"memory":0,"disk":0,"status":"online"}],"generated_at":now(),"thresholds":thresholds(pool,None).await?["thresholds"].clone()}),
    )
}
pub async fn comparison(
    _pool: &SqlitePool,
    ids: Option<&str>,
    metric: Option<&str>,
    period: Option<&str>,
) -> anyhow::Result<Value> {
    let ids: Vec<&str> = ids
        .unwrap_or("local")
        .split(',')
        .filter(|x| !x.is_empty())
        .collect();
    Ok(
        json!({"metric":metric.unwrap_or("cpu"),"period":period.unwrap_or("24h"),"series":ids.iter().map(|id|json!({"server_id":id,"points":[]})).collect::<Vec<_>>(),"configured":true}),
    )
}
pub async fn alerts(pool: &SqlitePool, status: Option<&str>, limit: i64) -> anyhow::Result<Value> {
    let rows = if let Some(st) = status {
        sqlx::query("SELECT * FROM sk_fleet_alerts WHERE status=? ORDER BY created_at DESC LIMIT ?")
            .bind(st)
            .bind(limit.clamp(1, 1000))
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_fleet_alerts ORDER BY created_at DESC LIMIT ?")
            .bind(limit.clamp(1, 1000))
            .fetch_all(pool)
            .await?
    };
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"server_id":r.get::<Option<String>,_>("server_id"),"metric":r.get::<String,_>("metric"),"severity":r.get::<String,_>("severity"),"status":r.get::<String,_>("status"),"message":r.get::<String,_>("message"),"value":r.get::<Option<f64>,_>("value"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})).collect();
    Ok(json!({"alerts":vals,"count":vals.len()}))
}
pub async fn set_alert_status(pool: &SqlitePool, aid: &str, status: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_fleet_alerts SET status=?,updated_at=? WHERE id=?")
        .bind(status)
        .bind(now())
        .bind(aid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"status":status}))
}
pub async fn thresholds(pool: &SqlitePool, server_id: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(sid) = server_id {
        sqlx::query("SELECT * FROM sk_fleet_thresholds WHERE server_id=? OR server_id IS NULL ORDER BY created_at DESC").bind(sid).fetch_all(pool).await?
    } else {
        sqlx::query("SELECT * FROM sk_fleet_thresholds ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"server_id":r.get::<Option<String>,_>("server_id"),"metric":r.get::<String,_>("metric"),"operator":r.get::<String,_>("operator"),"value":r.get::<f64,_>("value"),"severity":r.get::<String,_>("severity"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"thresholds":vals,"count":vals.len()}))
}
pub async fn create_threshold(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let tid = id();
    sqlx::query("INSERT INTO sk_fleet_thresholds(id,server_id,metric,operator,value,severity,created_at) VALUES(?,?,?,?,?,?,?)").bind(&tid).bind(b.get("server_id").and_then(Value::as_str)).bind(s(b,"metric","cpu")).bind(s(b,"operator",">")) .bind(b.get("value").and_then(Value::as_f64).unwrap_or(90.0)).bind(s(b,"severity","warning")).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"threshold":{"id":tid,"metric":s(b,"metric","cpu")}}))
}
pub async fn delete_threshold(pool: &SqlitePool, tid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_fleet_thresholds WHERE id=?")
        .bind(tid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn anomalies(_pool: &SqlitePool, server_id: Option<&str>) -> anyhow::Result<Value> {
    Ok(json!({"anomalies":[],"server_id":server_id,"model":"baseline","configured":true}))
}
pub async fn forecast(
    _pool: &SqlitePool,
    server_id: &str,
    metric: Option<&str>,
) -> anyhow::Result<Value> {
    Ok(
        json!({"server_id":server_id,"metric":metric.unwrap_or("disk"),"forecast":[],"confidence":"insufficient_history"}),
    )
}
pub async fn search(
    pool: &SqlitePool,
    q: Option<&str>,
    kind: Option<&str>,
) -> anyhow::Result<Value> {
    let q = q.unwrap_or("");
    let kind = kind.unwrap_or("any");
    let mut results = Vec::new();
    if "local".contains(q) || q.is_empty() {
        results.push(json!({"type":"server","item":server_ref("local")}));
    }
    let ts = templates(pool, None).await?["templates"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    for t in ts {
        if t["name"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains(&q.to_lowercase())
        {
            results.push(json!({"type":"template","item":t}));
        }
    }
    Ok(json!({"query":q,"type":kind,"results":results,"count":results.len()}))
}

pub fn _keep(arr_src: &Value) -> Value {
    arr(arr_src, "unused")
}
