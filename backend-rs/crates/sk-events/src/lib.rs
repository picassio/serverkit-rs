//! Durable events, notifications, subscriptions, webhooks, and API analytics.

use anyhow::Context;
use chrono::{Duration, Utc};
use rand::{distributions::Alphanumeric, Rng};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn id() -> String {
    Uuid::new_v4().to_string()
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
fn s<'a>(body: &'a Value, k: &str, d: &'a str) -> &'a str {
    body.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(body: &'a Value, k: &str) -> Option<&'a str> {
    body.get(k).and_then(Value::as_str)
}
fn j(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}
fn arr(v: Option<&Value>) -> Value {
    v.cloned().unwrap_or_else(|| json!([]))
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sk_telemetry_events (
            id TEXT PRIMARY KEY, source TEXT NOT NULL, event_type TEXT NOT NULL, severity TEXT NOT NULL,
            resource_type TEXT, resource_id TEXT, correlation_id TEXT, message TEXT, payload_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sk_telemetry_lookup ON sk_telemetry_events(source, event_type, severity, created_at);
        CREATE TABLE IF NOT EXISTS sk_notifications (
            id TEXT PRIMARY KEY, title TEXT NOT NULL, body TEXT, level TEXT NOT NULL, channel TEXT NOT NULL,
            status TEXT NOT NULL, read_at TEXT, payload_json TEXT NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_email_providers (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, kind TEXT NOT NULL, config_encrypted TEXT NOT NULL,
            is_default INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_notification_config (
            id TEXT PRIMARY KEY, settings_json TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_event_subscriptions (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, url TEXT NOT NULL, events_json TEXT NOT NULL,
            secret_encrypted TEXT, enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_event_deliveries (
            id TEXT PRIMARY KEY, subscription_id TEXT, endpoint_id TEXT, event_id TEXT, status TEXT NOT NULL,
            request_json TEXT, response_json TEXT, attempts INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_webhook_endpoints (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, url TEXT NOT NULL, events_json TEXT NOT NULL,
            secret_encrypted TEXT NOT NULL, enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_api_analytics (
            id TEXT PRIMARY KEY, method TEXT NOT NULL, path TEXT NOT NULL, status INTEGER NOT NULL,
            api_key_id TEXT, latency_ms INTEGER, error TEXT, created_at TEXT NOT NULL
        );
    "#).execute(pool).await.context("ensure sk-events schema")?;
    Ok(())
}

fn event(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "source": row.get::<String,_>("source"), "event_type": row.get::<String,_>("event_type"),
        "severity": row.get::<String,_>("severity"), "resource_type": row.try_get::<Option<String>,_>("resource_type").ok().flatten(),
        "resource_id": row.try_get::<Option<String>,_>("resource_id").ok().flatten(), "correlation_id": row.try_get::<Option<String>,_>("correlation_id").ok().flatten(),
        "message": row.try_get::<Option<String>,_>("message").ok().flatten(), "payload": j(row.try_get::<Option<String>,_>("payload_json").ok().flatten()),
        "created_at": row.get::<String,_>("created_at")
    })
}
fn notification(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "title": row.get::<String,_>("title"), "body": row.try_get::<Option<String>,_>("body").ok().flatten(),
        "level": row.get::<String,_>("level"), "channel": row.get::<String,_>("channel"), "status": row.get::<String,_>("status"),
        "read_at": row.try_get::<Option<String>,_>("read_at").ok().flatten(), "payload": j(row.try_get::<Option<String>,_>("payload_json").ok().flatten()),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn provider(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"), "kind": row.get::<String,_>("kind"),
        "is_default": row.get::<i64,_>("is_default") != 0, "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn subscription(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"), "url": row.get::<String,_>("url"),
        "events": j(row.try_get::<Option<String>,_>("events_json").ok().flatten()), "enabled": row.get::<i64,_>("enabled") != 0,
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn endpoint(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"), "url": row.get::<String,_>("url"),
        "events": j(row.try_get::<Option<String>,_>("events_json").ok().flatten()), "enabled": row.get::<i64,_>("enabled") != 0,
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn delivery(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "subscription_id": row.try_get::<Option<String>,_>("subscription_id").ok().flatten(),
        "endpoint_id": row.try_get::<Option<String>,_>("endpoint_id").ok().flatten(), "event_id": row.try_get::<Option<String>,_>("event_id").ok().flatten(),
        "status": row.get::<String,_>("status"), "request": j(row.try_get::<Option<String>,_>("request_json").ok().flatten()),
        "response": j(row.try_get::<Option<String>,_>("response_json").ok().flatten()), "attempts": row.get::<i64,_>("attempts"),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}

pub async fn emit_event(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let event_type = s(body, "event_type", "test.event").to_string();
    sqlx::query("INSERT INTO sk_telemetry_events (id,source,event_type,severity,resource_type,resource_id,correlation_id,message,payload_json,created_at) VALUES (?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(s(body,"source","serverkit")).bind(&event_type).bind(s(body,"severity","info")).bind(opt(body,"resource_type")).bind(opt(body,"resource_id")).bind(opt(body,"correlation_id")).bind(opt(body,"message")).bind(body.get("payload").cloned().unwrap_or_else(||body.clone()).to_string()).bind(&ts).execute(pool).await?;
    let event = get_event(pool, &id)
        .await?
        .context("created event missing")?;
    enqueue_event_deliveries(pool, &event_type, &event).await?;
    Ok(event)
}
pub async fn list_events(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_telemetry_events ORDER BY created_at DESC LIMIT 250")
        .fetch_all(pool)
        .await?;
    Ok(json!({"events":rows.iter().map(event).collect::<Vec<_>>(),"total":rows.len()}))
}
pub async fn get_event(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_telemetry_events WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(event))
}
pub async fn events_by_correlation(pool: &SqlitePool, cid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_telemetry_events WHERE correlation_id=? ORDER BY created_at DESC",
    )
    .bind(cid)
    .fetch_all(pool)
    .await?;
    Ok(json!({"events":rows.iter().map(event).collect::<Vec<_>>() }))
}
pub async fn event_sources(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT DISTINCT source FROM sk_telemetry_events ORDER BY source")
        .fetch_all(pool)
        .await?;
    Ok(json!({"sources":rows.into_iter().map(|r|r.get::<String,_>("source")).collect::<Vec<_>>() }))
}
pub async fn event_types(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT DISTINCT event_type FROM sk_telemetry_events ORDER BY event_type")
            .fetch_all(pool)
            .await?;
    Ok(
        json!({"event_types":rows.into_iter().map(|r|r.get::<String,_>("event_type")).collect::<Vec<_>>() }),
    )
}
pub async fn telemetry_stats(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT severity, COUNT(*) n FROM sk_telemetry_events GROUP BY severity")
            .fetch_all(pool)
            .await?;
    let mut out = json!({"total":0,"info":0,"warning":0,"error":0});
    for r in rows {
        let sev: String = r.get("severity");
        let n: i64 = r.get("n");
        out["total"] = json!(out["total"].as_i64().unwrap_or(0) + n);
        out[&sev] = json!(n);
    }
    Ok(out)
}
pub async fn cleanup_events(pool: &SqlitePool, days: i64) -> anyhow::Result<Value> {
    let cutoff = (Utc::now() - Duration::days(days)).to_rfc3339();
    let r = sqlx::query("DELETE FROM sk_telemetry_events WHERE created_at < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(json!({"deleted":r.rows_affected()}))
}

fn event_filter_matches(events_json: &str, event_type: &str) -> bool {
    let parsed: Value = serde_json::from_str(events_json).unwrap_or(Value::Null);
    parsed.as_array().is_none_or(|events| {
        events.is_empty() || events.iter().any(|e| e.as_str() == Some(event_type))
    })
}

async fn enqueue_event_deliveries(
    pool: &SqlitePool,
    event_type: &str,
    event: &Value,
) -> anyhow::Result<()> {
    let subs = sqlx::query("SELECT id, events_json FROM sk_event_subscriptions WHERE enabled = 1")
        .fetch_all(pool)
        .await?;
    for sub in subs {
        let events_json: String = sub.get("events_json");
        if event_filter_matches(&events_json, event_type) {
            let did = id();
            let ts = now();
            sqlx::query("INSERT INTO sk_event_deliveries (id,subscription_id,event_id,status,request_json,attempts,created_at,updated_at) VALUES (?,?,?,'queued',?,0,?,?)")
                .bind(&did)
                .bind(sub.get::<String, _>("id"))
                .bind(event["id"].as_str())
                .bind(event.to_string())
                .bind(&ts)
                .bind(&ts)
                .execute(pool)
                .await?;
        }
    }
    let endpoints =
        sqlx::query("SELECT id, events_json FROM sk_webhook_endpoints WHERE enabled = 1")
            .fetch_all(pool)
            .await?;
    for endpoint in endpoints {
        let events_json: String = endpoint.get("events_json");
        if event_filter_matches(&events_json, event_type) {
            let did = id();
            let ts = now();
            sqlx::query("INSERT INTO sk_event_deliveries (id,endpoint_id,event_id,status,request_json,attempts,created_at,updated_at) VALUES (?,?,?,'queued',?,0,?,?)")
                .bind(&did)
                .bind(endpoint.get::<String, _>("id"))
                .bind(event["id"].as_str())
                .bind(event.to_string())
                .bind(&ts)
                .bind(&ts)
                .execute(pool)
                .await?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeliveryTask {
    pub id: String,
    pub target_id: String,
    pub target_kind: String,
    pub url: String,
    pub secret: Option<String>,
    pub request: Value,
    pub attempts: i64,
}

pub async fn queued_delivery_tasks(
    pool: &SqlitePool,
    limit: i64,
) -> anyhow::Result<Vec<DeliveryTask>> {
    let rows = sqlx::query(
        r#"
        SELECT d.id, d.attempts, d.request_json,
               COALESCE(s.id, w.id) AS target_id,
               CASE WHEN s.id IS NOT NULL THEN 'subscription' ELSE 'webhook' END AS target_kind,
               COALESCE(s.url, w.url) AS url,
               COALESCE(s.secret_encrypted, w.secret_encrypted) AS secret_encrypted
          FROM sk_event_deliveries d
          LEFT JOIN sk_event_subscriptions s ON s.id = d.subscription_id
          LEFT JOIN sk_webhook_endpoints w ON w.id = d.endpoint_id
         WHERE d.status = 'queued'
         ORDER BY d.created_at ASC
         LIMIT ?
        "#,
    )
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let mut tasks = Vec::new();
    for row in rows {
        let secret = row
            .try_get::<Option<String>, _>("secret_encrypted")
            .ok()
            .flatten()
            .and_then(|v| sk_core::crypto::decrypt(&v));
        tasks.push(DeliveryTask {
            id: row.get("id"),
            target_id: row.get("target_id"),
            target_kind: row.get("target_kind"),
            url: row.get("url"),
            secret,
            request: j(row
                .try_get::<Option<String>, _>("request_json")
                .ok()
                .flatten()),
            attempts: row.get("attempts"),
        });
    }
    Ok(tasks)
}

pub async fn mark_delivery_result(
    pool: &SqlitePool,
    id: &str,
    success: bool,
    response: Value,
) -> anyhow::Result<()> {
    sqlx::query("UPDATE sk_event_deliveries SET status = ?, response_json = ?, attempts = attempts + 1, updated_at = ? WHERE id = ?")
        .bind(if success { "delivered" } else { "failed" })
        .bind(response.to_string())
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn create_notification(
    pool: &SqlitePool,
    title: &str,
    body: &str,
    level: &str,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_notifications (id,title,body,level,channel,status,payload_json,created_at,updated_at) VALUES (?,?,?,?, 'inbox','unread','{}',?,?)").bind(&id).bind(title).bind(body).bind(level).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"id":id,"success":true}))
}
pub async fn inbox(pool: &SqlitePool, unread_only: bool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_notifications ORDER BY created_at DESC LIMIT 250")
        .fetch_all(pool)
        .await?;
    let mut items: Vec<Value> = rows.iter().map(notification).collect();
    if unread_only {
        items.retain(|n| n["status"] == "unread");
    }
    let unread = items.iter().filter(|n| n["status"] == "unread").count();
    Ok(json!({"items":items,"notifications":items,"unread_count":unread}))
}
pub async fn unread_count(pool: &SqlitePool) -> anyhow::Result<Value> {
    let n: i64 = sqlx::query("SELECT COUNT(*) n FROM sk_notifications WHERE status='unread'")
        .fetch_one(pool)
        .await?
        .get("n");
    Ok(json!({"unread_count":n}))
}
pub async fn mark_read(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let ts = now();
    sqlx::query("UPDATE sk_notifications SET status='read', read_at=?, updated_at=? WHERE id=?")
        .bind(&ts)
        .bind(&ts)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(json!({"success":true}))
}
pub async fn mark_all_read(pool: &SqlitePool) -> anyhow::Result<Value> {
    let ts = now();
    let r=sqlx::query("UPDATE sk_notifications SET status='read', read_at=COALESCE(read_at,?), updated_at=? WHERE status='unread'").bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"updated":r.rows_affected()}))
}
pub async fn notification_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let inbox = unread_count(pool).await?;
    Ok(
        json!({"enabled":true,"unread_count":inbox["unread_count"],"channels":["inbox","email","webhook"]}),
    )
}
pub async fn notification_config(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT settings_json FROM sk_notification_config WHERE id='default'")
        .fetch_optional(pool)
        .await?;
    Ok(json!({"config":j(r.map(|r|r.get::<String,_>("settings_json"))) }))
}
pub async fn put_notification_config(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_notification_config (id,settings_json,updated_at) VALUES (?,?,?) ON CONFLICT(id) DO UPDATE SET settings_json=excluded.settings_json, updated_at=excluded.updated_at").bind(id).bind(body.to_string()).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"config":body}))
}
pub async fn preferences(pool: &SqlitePool) -> anyhow::Result<Value> {
    notification_config(pool).await
}

pub async fn delivery_log(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_event_deliveries ORDER BY created_at DESC LIMIT 250")
        .fetch_all(pool)
        .await?;
    Ok(json!({"deliveries":rows.iter().map(delivery).collect::<Vec<_>>() }))
}
pub async fn retry_delivery(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    sqlx::query("UPDATE sk_event_deliveries SET status='queued', attempts=attempts+1, updated_at=? WHERE id=?").bind(now()).bind(id).execute(pool).await?;
    Ok(json!({"success":true}))
}

pub async fn email_providers(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_email_providers ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"providers":rows.iter().map(provider).collect::<Vec<_>>() }))
}
pub async fn add_email_provider(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_email_providers (id,name,kind,config_encrypted,created_at,updated_at) VALUES (?,?,?,?,?,?)").bind(&id).bind(s(body,"name","Email Provider")).bind(s(body,"kind","smtp")).bind(sk_core::crypto::encrypt(&body.to_string())).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"provider":get_email_provider(pool,&id).await?}))
}
async fn get_email_provider(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_email_providers WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(provider(&r))
}
pub async fn delete_email_provider(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_email_providers WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}
pub async fn set_default_provider(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let mut tx = pool.begin().await?;
    sqlx::query("UPDATE sk_email_providers SET is_default=0")
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE sk_email_providers SET is_default=1, updated_at=? WHERE id=?")
        .bind(now())
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(json!({"success":true}))
}

pub fn available_events() -> Value {
    json!({"events":["test.event","project.created","job.completed","secret.created","webhook.received"]})
}
pub async fn subscriptions(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_event_subscriptions ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"subscriptions":rows.iter().map(subscription).collect::<Vec<_>>() }))
}
pub async fn create_subscription(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let secret = opt(body, "secret").map(sk_core::crypto::encrypt);
    sqlx::query("INSERT INTO sk_event_subscriptions (id,name,url,events_json,secret_encrypted,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)").bind(&id).bind(s(body,"name","Subscription")).bind(s(body,"url","http://localhost/unused")).bind(arr(body.get("events")).to_string()).bind(secret).bind(if body.get("enabled").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind(&ts).bind(&ts).execute(pool).await?;
    get_subscription(pool, &id)
        .await?
        .context("created subscription missing")
}
pub async fn get_subscription(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_event_subscriptions WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(subscription))
}
pub async fn update_subscription(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let events = body.get("events").map(Value::to_string);
    sqlx::query("UPDATE sk_event_subscriptions SET name=COALESCE(?,name), url=COALESCE(?,url), events_json=COALESCE(?,events_json), enabled=COALESCE(?,enabled), updated_at=? WHERE id=?").bind(opt(body,"name")).bind(opt(body,"url")).bind(events).bind(body.get("enabled").and_then(Value::as_bool).map(|b|if b{1}else{0})).bind(now()).bind(id).execute(pool).await?;
    get_subscription(pool, id).await
}
pub async fn delete_subscription(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_event_subscriptions WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}
pub async fn test_subscription(pool: &SqlitePool, sub_id: &str) -> anyhow::Result<Value> {
    let event=emit_event(pool,&json!({"source":"subscription","event_type":"test.event","message":"Test event","correlation_id":sub_id})).await?;
    let did = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_event_deliveries (id,subscription_id,event_id,status,request_json,response_json,attempts,created_at,updated_at) VALUES (?,?,?,'queued',?,NULL,0,?,?)").bind(&did).bind(sub_id).bind(event["id"].as_str()).bind(event.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"event":event,"delivery_id":did}))
}
pub async fn subscription_deliveries(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_event_deliveries WHERE subscription_id=? ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(json!({"deliveries":rows.iter().map(delivery).collect::<Vec<_>>() }))
}

pub async fn webhook_endpoints(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_webhook_endpoints ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"endpoints":rows.iter().map(endpoint).collect::<Vec<_>>() }))
}
pub async fn create_webhook_endpoint(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let raw = format!("whsec_{}", token());
    let ts = now();
    sqlx::query("INSERT INTO sk_webhook_endpoints (id,name,url,events_json,secret_encrypted,enabled,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)").bind(&id).bind(s(body,"name","Webhook")).bind(s(body,"url","http://localhost/unused")).bind(arr(body.get("events")).to_string()).bind(sk_core::crypto::encrypt(&raw)).bind(if body.get("enabled").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind(&ts).bind(&ts).execute(pool).await?;
    let mut v = get_webhook_endpoint(pool, &id).await?.unwrap();
    v["secret"] = json!(raw);
    Ok(v)
}
pub async fn get_webhook_endpoint(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_webhook_endpoints WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(endpoint))
}
pub async fn update_webhook_endpoint(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let events = body.get("events").map(Value::to_string);
    sqlx::query("UPDATE sk_webhook_endpoints SET name=COALESCE(?,name), url=COALESCE(?,url), events_json=COALESCE(?,events_json), enabled=COALESCE(?,enabled), updated_at=? WHERE id=?").bind(opt(body,"name")).bind(opt(body,"url")).bind(events).bind(body.get("enabled").and_then(Value::as_bool).map(|b|if b{1}else{0})).bind(now()).bind(id).execute(pool).await?;
    get_webhook_endpoint(pool, id).await
}
pub async fn delete_webhook_endpoint(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_webhook_endpoints WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}
pub async fn regenerate_webhook_secret(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let raw = format!("whsec_{}", token());
    sqlx::query("UPDATE sk_webhook_endpoints SET secret_encrypted=?, updated_at=? WHERE id=?")
        .bind(sk_core::crypto::encrypt(&raw))
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"secret":raw}))
}
pub async fn webhook_deliveries(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_event_deliveries WHERE endpoint_id=? ORDER BY created_at DESC",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;
    Ok(json!({"deliveries":rows.iter().map(delivery).collect::<Vec<_>>() }))
}

pub async fn record_api_request(
    pool: &SqlitePool,
    method: &str,
    path: &str,
    status: u16,
    latency_ms: i64,
    error: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sk_api_analytics (id, method, path, status, latency_ms, error, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
        .bind(id())
        .bind(method)
        .bind(path)
        .bind(status as i64)
        .bind(latency_ms)
        .bind(error)
        .bind(now())
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn api_analytics_overview(pool: &SqlitePool) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT COUNT(*) total, SUM(CASE WHEN status >= 400 THEN 1 ELSE 0 END) errors, AVG(latency_ms) avg_latency FROM sk_api_analytics")
        .fetch_one(pool)
        .await?;
    let total: i64 = row.get("total");
    let errors: i64 = row.get::<Option<i64>, _>("errors").unwrap_or(0);
    let avg_latency_ms = row.get::<Option<f64>, _>("avg_latency").unwrap_or(0.0);
    Ok(json!({
        "total_requests": total,
        "error_count": errors,
        "error_rate": if total > 0 { errors as f64 / total as f64 } else { 0.0 },
        "avg_latency_ms": avg_latency_ms,
        "period": "all"
    }))
}

pub async fn api_analytics_endpoints(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT path, method, COUNT(*) n, AVG(latency_ms) avg_latency, SUM(CASE WHEN status >= 400 THEN 1 ELSE 0 END) errors FROM sk_api_analytics GROUP BY path, method ORDER BY n DESC LIMIT 50")
        .fetch_all(pool)
        .await?;
    Ok(json!({"endpoints": rows.into_iter().map(|r|json!({
        "path": r.get::<String,_>("path"),
        "method": r.get::<String,_>("method"),
        "requests": r.get::<i64,_>("n"),
        "avg_latency_ms": r.get::<Option<f64>,_>("avg_latency").unwrap_or(0.0),
        "errors": r.get::<Option<i64>,_>("errors").unwrap_or(0)
    })).collect::<Vec<_>>() }))
}

pub async fn api_analytics_errors(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_api_analytics WHERE status >= 400 ORDER BY created_at DESC LIMIT 100",
    )
    .fetch_all(pool)
    .await?;
    Ok(json!({"errors": rows.into_iter().map(|r|json!({
        "method": r.get::<String,_>("method"),
        "path": r.get::<String,_>("path"),
        "status": r.get::<i64,_>("status"),
        "error": r.try_get::<Option<String>,_>("error").ok().flatten(),
        "latency_ms": r.try_get::<Option<i64>,_>("latency_ms").ok().flatten(),
        "created_at": r.get::<String,_>("created_at")
    })).collect::<Vec<_>>() }))
}

pub async fn api_analytics_timeseries(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT substr(created_at, 1, 13) bucket, COUNT(*) n, SUM(CASE WHEN status >= 400 THEN 1 ELSE 0 END) errors FROM sk_api_analytics GROUP BY bucket ORDER BY bucket")
        .fetch_all(pool)
        .await?;
    Ok(json!({"points": rows.into_iter().map(|r|json!({
        "bucket": r.get::<String,_>("bucket"),
        "requests": r.get::<i64,_>("n"),
        "errors": r.get::<Option<i64>,_>("errors").unwrap_or(0)
    })).collect::<Vec<_>>() }))
}

pub async fn api_key_usage(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT path, COUNT(*) n FROM sk_api_analytics WHERE api_key_id = ? GROUP BY path ORDER BY n DESC")
        .bind(id)
        .fetch_all(pool)
        .await?;
    let usage: Vec<Value> = rows
        .into_iter()
        .map(|r| json!({"path": r.get::<String,_>("path"), "requests": r.get::<i64,_>("n")}))
        .collect();
    Ok(
        json!({"total": usage.iter().filter_map(|u| u["requests"].as_i64()).sum::<i64>(), "usage": usage}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn event_and_notification_roundtrip() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let e = emit_event(
            &pool,
            &json!({"source":"test","event_type":"unit","severity":"info"}),
        )
        .await
        .unwrap();
        assert_eq!(e["source"], "test");
        create_notification(&pool, "Hi", "Body", "info")
            .await
            .unwrap();
        assert_eq!(unread_count(&pool).await.unwrap()["unread_count"], 1);
    }
}
