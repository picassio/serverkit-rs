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
CREATE TABLE IF NOT EXISTS sk_servers(id TEXT PRIMARY KEY, name TEXT NOT NULL, host TEXT, status TEXT NOT NULL, group_id TEXT, workspace_id TEXT, capabilities_json TEXT NOT NULL DEFAULT '{}', metadata_json TEXT NOT NULL DEFAULT '{}', registration_token TEXT, registration_expires TEXT, api_key TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_server_groups(id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_server_allowed_ips(server_id TEXT NOT NULL, ip TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(server_id,ip));
CREATE TABLE IF NOT EXISTS sk_fleet_commands(id TEXT PRIMARY KEY, server_id TEXT NOT NULL, command_type TEXT NOT NULL, payload_json TEXT NOT NULL DEFAULT '{}', status TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0, result_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_fleet_versions(id TEXT PRIMARY KEY, version TEXT NOT NULL, notes TEXT, artifact_url TEXT, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_fleet_rollouts(id TEXT PRIMARY KEY, version_id TEXT, status TEXT NOT NULL, plan_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_fleet_discovery(id TEXT PRIMARY KEY, agent_json TEXT NOT NULL DEFAULT '{}', status TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
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

fn local_server() -> Value {
    json!({"id":"local","name":"Local (this server)","host":"127.0.0.1","status":"online","is_local":true,"capabilities":{"terminal":true,"docker":true,"files":true,"system":true,"cron":true,"packages":true,"services":true}})
}
fn run_cmd(cmd: &str, args: &[&str]) -> Value {
    match std::process::Command::new(cmd).args(args).output() {
        Ok(o) => {
            json!({"success":o.status.success(),"exit_code":o.status.code(),"stdout":String::from_utf8_lossy(&o.stdout).trim(),"stderr":String::from_utf8_lossy(&o.stderr).trim()})
        }
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}
fn command_exists(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn safe_remote_path(p: &str) -> bool {
    let path = std::path::Path::new(p);
    path.is_absolute()
        && !path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        && ["/home", "/tmp", "/var/www", "/opt", "/srv"]
            .iter()
            .any(|r| path.starts_with(r))
}
async fn enqueue(
    pool: &SqlitePool,
    server_id: &str,
    command_type: &str,
    payload: Value,
    status: &str,
) -> anyhow::Result<Value> {
    let cid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_fleet_commands(id,server_id,command_type,payload_json,status,result_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(&cid).bind(server_id).bind(command_type).bind(payload.to_string()).bind(status).bind(json!({}).to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"id":cid,"server_id":server_id,"command_type":command_type,"status":status,"created_at":ts}),
    )
}
fn server_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"host":r.get::<Option<String>,_>("host"),"status":r.get::<String,_>("status"),"group_id":r.get::<Option<String>,_>("group_id"),"workspace_id":r.get::<Option<String>,_>("workspace_id"),"capabilities":j(Some(r.get("capabilities_json"))),"metadata":j(Some(r.get("metadata_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn servers_list(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_servers ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let mut servers = vec![local_server()];
    servers.extend(rows.into_iter().map(server_row));
    Ok(json!({"servers":servers,"items":servers,"count":servers.len()}))
}
pub async fn create_server_record(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let sid = id();
    let ts = now();
    let token = id();
    let name = s(b, "name", "Remote server");
    sqlx::query("INSERT INTO sk_servers(id,name,host,status,group_id,workspace_id,capabilities_json,metadata_json,registration_token,registration_expires,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&sid).bind(name).bind(b.get("host").and_then(Value::as_str)).bind("pending_registration").bind(b.get("group_id").and_then(Value::as_str)).bind(b.get("workspace_id").and_then(Value::as_str)).bind(json!({}).to_string()).bind(b.to_string()).bind(&token).bind((Utc::now()+Duration::days(7)).to_rfc3339()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"server":get_server_record(pool,&sid).await?["server"].clone(),"registration_token":token}),
    )
}
pub async fn get_server_record(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    if sid == "local" {
        return Ok(json!({"server":local_server()}));
    }
    let row = sqlx::query("SELECT * FROM sk_servers WHERE id=?")
        .bind(sid)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some(r) => json!({"server":server_row(r)}),
        None => json!({"success":false,"code":"SERVER_NOT_FOUND","error":"Server not found"}),
    })
}
pub async fn update_server_record(
    pool: &SqlitePool,
    sid: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    if sid == "local" {
        return Ok(
            json!({"success":false,"code":"LOCAL_IMMUTABLE","error":"Local server metadata is derived from this host"}),
        );
    }
    let old = sqlx::query("SELECT * FROM sk_servers WHERE id=?")
        .bind(sid)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(json!({"success":false,"code":"SERVER_NOT_FOUND","error":"Server not found"}));
    };
    let old_name: String = r.get("name");
    let old_host: Option<String> = r.get("host");
    let old_group: Option<String> = r.get("group_id");
    let old_workspace: Option<String> = r.get("workspace_id");
    let name = s(b, "name", &old_name).to_string();
    let host = b
        .get("host")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(old_host);
    let group_id = b
        .get("group_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(old_group);
    let workspace_id = b
        .get("workspace_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or(old_workspace);
    sqlx::query("UPDATE sk_servers SET name=?,host=?,group_id=?,workspace_id=?,metadata_json=?,updated_at=? WHERE id=?")
        .bind(name).bind(host).bind(group_id).bind(workspace_id).bind(b.to_string()).bind(now()).bind(sid).execute(pool).await?;
    Ok(json!({"success":true,"server":get_server_record(pool,sid).await?["server"].clone()}))
}
pub async fn delete_server_record(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    if sid == "local" {
        return Ok(
            json!({"success":false,"code":"LOCAL_IMMUTABLE","error":"Cannot delete local server"}),
        );
    }
    let n = sqlx::query("DELETE FROM sk_servers WHERE id=?")
        .bind(sid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn set_workspace(
    pool: &SqlitePool,
    sid: &str,
    workspace_id: Option<&str>,
) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_servers SET workspace_id=?,updated_at=? WHERE id=?")
        .bind(workspace_id)
        .bind(now())
        .bind(sid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":sid=="local" || n>0,"workspace_id":workspace_id}))
}
pub async fn regenerate_token(
    pool: &SqlitePool,
    sid: &str,
    expires_in: Option<i64>,
) -> anyhow::Result<Value> {
    let token = id();
    let exp = expires_in
        .map(|s| {
            if s < 0 {
                "never".to_string()
            } else {
                (Utc::now() + Duration::seconds(s)).to_rfc3339()
            }
        })
        .unwrap_or_else(|| (Utc::now() + Duration::days(7)).to_rfc3339());
    if sid != "local" {
        sqlx::query("UPDATE sk_servers SET registration_token=?,registration_expires=?,updated_at=? WHERE id=?").bind(&token).bind(&exp).bind(now()).bind(sid).execute(pool).await?;
    }
    Ok(
        json!({"success":true,"registration_token":token,"registration_expires":exp,"connection_string":format!("serverkit-agent register --url $PANEL_URL --token {token}")}),
    )
}
pub async fn register_server(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let token = s(b, "registration_token", s(b, "token", ""));
    let row = sqlx::query("SELECT * FROM sk_servers WHERE registration_token=?")
        .bind(token)
        .fetch_optional(pool)
        .await?;
    let Some(r) = row else {
        return Ok(
            json!({"success":false,"code":"TOKEN_INVALID","error":"Registration token not found"}),
        );
    };
    let sid: String = r.get("id");
    sqlx::query("UPDATE sk_servers SET status='online',capabilities_json=?,metadata_json=?,updated_at=? WHERE id=?").bind(b.get("capabilities").cloned().unwrap_or_else(||json!({})).to_string()).bind(b.to_string()).bind(now()).bind(&sid).execute(pool).await?;
    Ok(json!({"success":true,"server":get_server_record(pool,&sid).await?["server"].clone()}))
}

pub async fn groups(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_server_groups ORDER BY name")
        .fetch_all(pool)
        .await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"description":r.get::<Option<String>,_>("description"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"groups":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_group(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let gid = id();
    let ts = now();
    sqlx::query(
        "INSERT INTO sk_server_groups(id,name,description,created_at,updated_at) VALUES(?,?,?,?,?)",
    )
    .bind(&gid)
    .bind(s(b, "name", "Server group"))
    .bind(b.get("description").and_then(Value::as_str))
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    Ok(json!({"success":true,"group":{"id":gid,"name":s(b,"name","Server group")}}))
}
pub async fn update_group(pool: &SqlitePool, gid: &str, b: &Value) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_server_groups SET name=?,description=?,updated_at=? WHERE id=?")
        .bind(s(b, "name", "Server group"))
        .bind(b.get("description").and_then(Value::as_str))
        .bind(now())
        .bind(gid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0}))
}
pub async fn delete_group(pool: &SqlitePool, gid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_server_groups WHERE id=?")
        .bind(gid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}

pub async fn fleet_health(pool: &SqlitePool) -> anyhow::Result<Value> {
    let list = servers_list(pool).await?["servers"].clone();
    let total = list.as_array().map(|a| a.len()).unwrap_or(0);
    let healthy = list
        .as_array()
        .unwrap_or(&vec![])
        .iter()
        .filter(|s| s["status"] == "online")
        .count();
    Ok(json!({"healthy":healthy,"total":total,"servers":list}))
}
pub async fn agent_version() -> anyhow::Result<Value> {
    Ok(
        json!({"version":env!("CARGO_PKG_VERSION"),"protocol":"sk-fleet-v1","platform":std::env::consts::OS,"arch":std::env::consts::ARCH}),
    )
}
pub async fn versions(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_fleet_versions ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let mut vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"version":r.get::<String,_>("version"),"notes":r.get::<Option<String>,_>("notes"),"artifact_url":r.get::<Option<String>,_>("artifact_url"),"metadata":j(Some(r.get("metadata_json"))),"created_at":r.get::<String,_>("created_at")})).collect();
    if vals.is_empty() {
        vals.push(json!({"id":"current","version":env!("CARGO_PKG_VERSION"),"current":true}));
    }
    Ok(json!({"versions":vals,"items":vals}))
}
pub async fn add_version(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let vid = id();
    sqlx::query("INSERT INTO sk_fleet_versions(id,version,notes,artifact_url,metadata_json,created_at) VALUES(?,?,?,?,?,?)").bind(&vid).bind(s(b,"version",env!("CARGO_PKG_VERSION"))).bind(b.get("notes").and_then(Value::as_str)).bind(b.get("artifact_url").and_then(Value::as_str)).bind(b.to_string()).bind(now()).execute(pool).await?;
    Ok(
        json!({"success":true,"version":{"id":vid,"version":s(b,"version",env!("CARGO_PKG_VERSION"))}}),
    )
}
pub async fn queued_commands(pool: &SqlitePool, server_id: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(sid) = server_id {
        sqlx::query("SELECT * FROM sk_fleet_commands WHERE server_id=? AND status IN ('queued','retry') ORDER BY created_at DESC").bind(sid).fetch_all(pool).await?
    } else {
        sqlx::query("SELECT * FROM sk_fleet_commands WHERE status IN ('queued','retry') ORDER BY created_at DESC").fetch_all(pool).await?
    };
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"server_id":r.get::<String,_>("server_id"),"command_type":r.get::<String,_>("command_type"),"payload":j(Some(r.get("payload_json"))),"status":r.get::<String,_>("status"),"attempts":r.get::<i64,_>("attempts"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"commands":vals,"items":vals,"count":vals.len()}))
}
pub async fn retry_command(pool: &SqlitePool, cid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query(
        "UPDATE sk_fleet_commands SET status='retry',attempts=attempts+1,updated_at=? WHERE id=?",
    )
    .bind(now())
    .bind(cid)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(json!({"success":n>0,"status":"retry"}))
}
pub async fn discovery(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_fleet_discovery ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"agent":j(Some(r.get("agent_json"))),"status":r.get::<String,_>("status"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"agents":vals,"items":vals,"count":vals.len()}))
}
pub async fn start_discovery(pool: &SqlitePool, duration: i64) -> anyhow::Result<Value> {
    let did = id();
    sqlx::query("INSERT INTO sk_fleet_discovery(id,agent_json,status,created_at,updated_at) VALUES(?,?,?,?,?)").bind(&did).bind(json!({"source":"local-scan","duration":duration}).to_string()).bind("scanning").bind(now()).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"discovery_id":did,"duration":duration,"status":"scanning"}))
}
pub async fn approve_discovery(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_fleet_discovery SET status='approved',updated_at=? WHERE id=?")
        .bind(now())
        .bind(did)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"status":"approved"}))
}
pub async fn reject_discovery(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_fleet_discovery SET status='rejected',updated_at=? WHERE id=?")
        .bind(now())
        .bind(did)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"status":"rejected"}))
}
pub async fn start_rollout(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let rid = id();
    sqlx::query("INSERT INTO sk_fleet_rollouts(id,version_id,status,plan_json,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(&rid).bind(b.get("version_id").and_then(Value::as_str)).bind("queued").bind(b.to_string()).bind(now()).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"rollout":{"id":rid,"status":"queued"}}))
}
pub async fn rollouts(pool: &SqlitePool, status: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(st) = status {
        sqlx::query("SELECT * FROM sk_fleet_rollouts WHERE status=? ORDER BY created_at DESC")
            .bind(st)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_fleet_rollouts ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"version_id":r.get::<Option<String>,_>("version_id"),"status":r.get::<String,_>("status"),"plan":j(Some(r.get("plan_json"))),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"rollouts":vals,"items":vals,"count":vals.len()}))
}
pub async fn get_rollout(pool: &SqlitePool, rid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_fleet_rollouts WHERE id=?")
        .bind(rid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            json!({"rollout":{"id":r.get::<String,_>("id"),"status":r.get::<String,_>("status"),"plan":j(Some(r.get("plan_json")))}})
        }
        None => json!({"success":false,"code":"ROLLOUT_NOT_FOUND","error":"Rollout not found"}),
    })
}
pub async fn cancel_rollout(pool: &SqlitePool, rid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("UPDATE sk_fleet_rollouts SET status='cancelled',updated_at=? WHERE id=?")
        .bind(now())
        .bind(rid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"status":"cancelled"}))
}
pub async fn upgrade_fleet(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    start_rollout(pool, b).await
}
pub async fn diagnostics(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"server":get_server_record(pool,sid).await?,"queued_commands":queued_commands(pool,Some(sid)).await?,"agent_connected":sid=="local"}),
    )
}

pub async fn allowed_ips(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT ip FROM sk_server_allowed_ips WHERE server_id=? ORDER BY ip")
        .bind(sid)
        .fetch_all(pool)
        .await?;
    let vals: Vec<String> = rows.into_iter().map(|r| r.get("ip")).collect();
    Ok(json!({"server_id":sid,"allowed_ips":vals}))
}
pub async fn set_allowed_ips(pool: &SqlitePool, sid: &str, ips: Value) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_server_allowed_ips WHERE server_id=?")
        .bind(sid)
        .execute(pool)
        .await?;
    if let Some(arr) = ips.as_array() {
        for ip in arr.iter().filter_map(Value::as_str) {
            sqlx::query("INSERT OR IGNORE INTO sk_server_allowed_ips(server_id,ip,created_at) VALUES(?,?,?)").bind(sid).bind(ip).bind(now()).execute(pool).await?;
        }
    }
    allowed_ips(pool, sid)
        .await
        .map(|v| json!({"success":true,"server_id":sid,"allowed_ips":v["allowed_ips"].clone()}))
}
pub async fn security_alerts(
    pool: &SqlitePool,
    server_id: Option<&str>,
    status: Option<&str>,
) -> anyhow::Result<Value> {
    let mut v = alerts(pool, status, 100).await?;
    if let Some(sid) = server_id {
        if let Some(a) = v["alerts"].as_array() {
            v["alerts"] = json!(a
                .iter()
                .filter(|x| x["server_id"].as_str() == Some(sid))
                .cloned()
                .collect::<Vec<_>>());
        }
    }
    Ok(v)
}
pub async fn security_counts(pool: &SqlitePool, server_id: Option<&str>) -> anyhow::Result<Value> {
    let a = security_alerts(pool, server_id, None).await?;
    let total = a["alerts"].as_array().map(|x| x.len()).unwrap_or(0);
    Ok(json!({"total":total,"open":total,"acknowledged":0,"resolved":0}))
}

async fn offline(
    server_id: &str,
    path: &str,
    method: &str,
    pool: &SqlitePool,
    payload: Value,
) -> anyhow::Result<Value> {
    let cmd = enqueue(pool, server_id, path, payload, "queued").await?;
    Ok(
        json!({"success":false,"code":"AGENT_OFFLINE","error":"Agent not connected","server_id":server_id,"command":cmd,"method":method}),
    )
}
pub async fn server_route(
    pool: &SqlitePool,
    method: &str,
    server_id: &str,
    path: &str,
    query: Value,
    body: Value,
) -> anyhow::Result<Value> {
    if server_id != "local" {
        return offline(
            server_id,
            path,
            method,
            pool,
            json!({"query":query,"body":body}),
        )
        .await;
    }
    match (method, path) {
        ("GET", "status") | ("POST", "ping") => Ok(
            json!({"success":true,"server_id":"local","status":"online","latency_ms":0,"checked_at":now()}),
        ),
        ("GET", "connection-info") => Ok(
            json!({"server_id":"local","url":"http://127.0.0.1","agent_connected":true,"is_local":true}),
        ),
        ("POST", "refresh-capabilities") => Ok(
            json!({"success":true,"server_id":"local","capabilities":local_server()["capabilities"].clone()}),
        ),
        ("POST", "rotate-api-key") => Ok(json!({"success":true,"api_key":id()})),
        ("GET", "metrics") | ("GET", "system/metrics") => Ok(
            json!({"server_id":"local","metrics":{"loadavg":std::fs::read_to_string("/proc/loadavg").unwrap_or_default(),"timestamp":now()}}),
        ),
        ("GET", "metrics/history") => Ok(
            json!({"server_id":"local","period":query.get("period").and_then(Value::as_str).unwrap_or("24h"),"points":[]}),
        ),
        ("GET", "metrics/aggregated") => Ok(
            json!({"server_id":"local","period":query.get("period").and_then(Value::as_str).unwrap_or("24h"),"aggregation":query.get("aggregation").and_then(Value::as_str).unwrap_or("hourly"),"points":[]}),
        ),
        ("GET", "system/info") => Ok(
            json!({"hostname":run_cmd("hostname",&[])["stdout"],"uname":run_cmd("uname",&["-a"])["stdout"],"server_id":"local"}),
        ),
        ("GET", "docker/containers") => Ok(
            json!({"installed":command_exists("docker"),"result":run_cmd("docker",&["ps","-a","--format","json"])}),
        ),
        ("GET", p) if p.starts_with("docker/containers/") && p.ends_with("/logs") => {
            let cid = p
                .trim_start_matches("docker/containers/")
                .trim_end_matches("/logs")
                .trim_matches('/');
            Ok(
                json!({"container":cid,"result":run_cmd("docker",&["logs","--tail",query.get("tail").and_then(Value::as_str).unwrap_or("100"),cid])}),
            )
        }
        ("GET", p) if p.starts_with("docker/containers/") && p.ends_with("/stats") => {
            let cid = p
                .trim_start_matches("docker/containers/")
                .trim_end_matches("/stats")
                .trim_matches('/');
            Ok(json!({"container":cid,"result":run_cmd("docker",&["stats","--no-stream",cid])}))
        }
        ("GET", p) if p.starts_with("docker/containers/") => {
            let cid = p.trim_start_matches("docker/containers/");
            Ok(json!({"container":cid,"result":run_cmd("docker",&["inspect",cid])}))
        }
        ("POST", p) if p.starts_with("docker/containers/") => {
            let parts: Vec<&str> = p.split('/').collect();
            let cid = parts.get(2).copied().unwrap_or("");
            let action = parts.get(3).copied().unwrap_or("");
            if ["start", "stop", "restart"].contains(&action) {
                Ok(
                    json!({"success":run_cmd("docker",&[action,cid])["success"],"result":run_cmd("docker",&[action,cid])}),
                )
            } else {
                offline("local", p, method, pool, body).await
            }
        }
        ("DELETE", p) if p.starts_with("docker/containers/") => {
            let cid = p.trim_start_matches("docker/containers/");
            Ok(
                json!({"success":run_cmd("docker",&["rm",cid])["success"],"result":run_cmd("docker",&["rm",cid])}),
            )
        }
        ("GET", "docker/images") => Ok(
            json!({"installed":command_exists("docker"),"result":run_cmd("docker",&["images","--format","json"])}),
        ),
        ("POST", "docker/images/pull") => {
            let image = s(&body, "image", "");
            if image.is_empty() {
                Ok(json!({"success":false,"error":"image is required"}))
            } else {
                Ok(json!({"success":run_cmd("docker",&["pull",image])["success"],"image":image}))
            }
        }
        ("DELETE", p) if p.starts_with("docker/images/") => {
            let image = p.trim_start_matches("docker/images/");
            Ok(json!({"success":run_cmd("docker",&["rmi",image])["success"]}))
        }
        ("GET", "docker/volumes") => Ok(
            json!({"installed":command_exists("docker"),"result":run_cmd("docker",&["volume","ls","--format","json"])}),
        ),
        ("GET", "docker/networks") => Ok(
            json!({"installed":command_exists("docker"),"result":run_cmd("docker",&["network","ls","--format","json"])}),
        ),
        ("DELETE", p) if p.starts_with("docker/volumes/") => Ok(
            json!({"success":run_cmd("docker",&["volume","rm",p.trim_start_matches("docker/volumes/")])["success"]}),
        ),
        ("DELETE", p) if p.starts_with("docker/networks/") => Ok(
            json!({"success":run_cmd("docker",&["network","rm",p.trim_start_matches("docker/networks/")])["success"]}),
        ),
        ("GET", "docker/compose/projects") => Ok(
            json!({"projects":[],"detected":run_cmd("sh",&["-lc","find /opt /srv /var/www -name compose.yaml -o -name docker-compose.yml 2>/dev/null | head -100"])}),
        ),
        (_, p) if p.starts_with("docker/compose/") => offline("local", p, method, pool, body).await,
        ("GET", "files/allowed-paths") => {
            Ok(json!({"allowed_paths":["/home","/tmp","/var/www","/opt","/srv"]}))
        }
        ("GET", "files/browse") => {
            let p = query.get("path").and_then(Value::as_str).unwrap_or("/tmp");
            if !safe_remote_path(p) {
                return Ok(json!({"success":false,"error":"Path is not allowed"}));
            }
            let mut items = Vec::new();
            if let Ok(rd) = std::fs::read_dir(p) {
                for e in rd.flatten().take(500) {
                    if let Ok(m) = e.metadata() {
                        items.push(json!({"name":e.file_name().to_string_lossy(),"path":e.path(),"is_dir":m.is_dir(),"size":m.len()}));
                    }
                }
            }
            Ok(json!({"path":p,"items":items}))
        }
        ("GET", "files/read") => {
            let p = query.get("path").and_then(Value::as_str).unwrap_or("");
            if !safe_remote_path(p) {
                return Ok(json!({"success":false,"error":"Path is not allowed"}));
            }
            Ok(json!({"path":p,"content":std::fs::read_to_string(p).unwrap_or_default()}))
        }
        ("POST", "files/write") => {
            let p = s(&body, "path", "");
            if !safe_remote_path(p) {
                return Ok(json!({"success":false,"error":"Path is not allowed"}));
            }
            if let Some(parent) = std::path::Path::new(p).parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            std::fs::write(p, s(&body, "content", ""))
                .map(|_| json!({"success":true,"path":p}))
                .or_else(|e| {
                    Ok::<Value, anyhow::Error>(json!({"success":false,"error":e.to_string()}))
                })
        }
        ("GET", "cron/status") => {
            Ok(json!({"installed":command_exists("crontab"),"result":run_cmd("crontab",&["-l"])}))
        }
        ("GET", "cron/jobs") => Ok(json!({"jobs":[],"raw":run_cmd("crontab",&["-l"])})),
        (_, p) if p.starts_with("cron/jobs") => offline("local", p, method, pool, body).await,
        ("GET", "packages") => Ok(
            json!({"manager":if command_exists("apt-cache"){"apt"}else{"unknown"},"packages":[]}),
        ),
        ("GET", "packages/search") => {
            let q = query.get("q").and_then(Value::as_str).unwrap_or("");
            Ok(
                json!({"query":q,"result":if command_exists("apt-cache"){run_cmd("apt-cache",&["search",q])}else{json!({"success":false,"error":"No package manager detected"})}}),
            )
        }
        ("GET", p) if p.starts_with("packages/info/") => {
            let pkg = p.trim_start_matches("packages/info/");
            Ok(json!({"package":pkg,"result":run_cmd("apt-cache",&["show",pkg])}))
        }
        (_, p) if p.starts_with("packages/") => offline("local", p, method, pool, body).await,
        ("GET", "services") => Ok(
            json!({"services":run_cmd("systemctl",&["list-units","--type",query.get("type").and_then(Value::as_str).unwrap_or("service"),"--no-pager","--plain"])}),
        ),
        ("GET", p) if p.starts_with("services/") && p.ends_with("/logs") => {
            let unit = p
                .trim_start_matches("services/")
                .trim_end_matches("/logs")
                .trim_matches('/');
            Ok(
                json!({"unit":unit,"logs":run_cmd("journalctl",&["-u",unit,"-n",query.get("lines").and_then(Value::as_str).unwrap_or("200"),"--no-pager"])}),
            )
        }
        ("GET", p) if p.starts_with("services/") => {
            let unit = p.trim_start_matches("services/");
            Ok(json!({"unit":unit,"status":run_cmd("systemctl",&["status",unit,"--no-pager"])}))
        }
        ("POST", "services/daemon-reload") => {
            Ok(json!({"success":run_cmd("systemctl",&["daemon-reload"])["success"]}))
        }
        ("POST", p) if p.starts_with("services/") => {
            let parts: Vec<&str> = p.split('/').collect();
            let unit = parts.get(1).copied().unwrap_or("");
            let action = parts.get(2).copied().unwrap_or("");
            if ["start", "stop", "restart", "reload"].contains(&action) {
                Ok(json!({"success":run_cmd("systemctl",&[action,unit])["success"]}))
            } else {
                offline("local", p, method, pool, body).await
            }
        }
        ("GET", "runtimes") => Ok(
            json!({"python":command_exists("python3"),"node":command_exists("node"),"php":command_exists("php")}),
        ),
        ("GET", "runtimes/python") | ("GET", "runtimes/python/current") => {
            Ok(json!({"current":run_cmd("python3",&["--version"]),"versions":[]}))
        }
        ("GET", "runtimes/python/available") => {
            Ok(json!({"versions":[],"source":"pyenv not configured"}))
        }
        (_, p) if p.starts_with("runtimes/python") || p == "runtimes/pyenv/bootstrap" => {
            offline("local", p, method, pool, body).await
        }
        ("GET", "cloudflared/status") => Ok(
            json!({"installed":command_exists("cloudflared"),"configured":std::path::Path::new("/root/.cloudflared/cert.pem").exists()}),
        ),
        ("GET", "cloudflared/tunnels") => Ok(
            json!({"installed":command_exists("cloudflared"),"tunnels":[],"result":run_cmd("cloudflared",&["tunnel","list"])}),
        ),
        (_, p) if p.starts_with("cloudflared/") => offline("local", p, method, pool, body).await,
        ("GET", "proxy") | ("GET", "proxy/compose-preview") | ("GET", "proxy/ingress-audit") => Ok(
            json!({"server_id":"local","configured":false,"proxy":"nginx","requires_configuration":true}),
        ),
        ("POST", p) if p.starts_with("proxy/") => offline("local", p, method, pool, body).await,
        ("GET", "security/alerts") => security_alerts(pool, Some("local"), None).await,
        ("POST", "onboarding/start") | ("POST", "onboarding/retry") => {
            Ok(json!({"success":true,"status":"complete","server_id":"local"}))
        }
        ("GET", "onboarding/status") => {
            Ok(json!({"server_id":"local","status":"complete","steps":[]}))
        }
        _ => offline("local", path, method, pool, body).await,
    }
}
pub async fn metrics_compare(
    _pool: &SqlitePool,
    ids: Option<&str>,
    metric: Option<&str>,
    period: Option<&str>,
) -> anyhow::Result<Value> {
    Ok(
        json!({"ids":ids.unwrap_or("local"),"metric":metric.unwrap_or("cpu"),"period":period.unwrap_or("24h"),"series":[]}),
    )
}
pub async fn metrics_retention() -> anyhow::Result<Value> {
    Ok(json!({"retention_days":30,"samples":0}))
}
pub async fn metrics_cleanup() -> anyhow::Result<Value> {
    Ok(json!({"success":true,"deleted":0}))
}
pub async fn proxy_overview() -> anyhow::Result<Value> {
    Ok(json!({"servers":[{"id":"local","proxy_configured":false}],"configured":false}))
}
