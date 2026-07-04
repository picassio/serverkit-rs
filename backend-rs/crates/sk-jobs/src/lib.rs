//! SQLite-backed jobs and queue bus for serverkit-rs.
//!
//! This crate is intentionally small but real: empty lists mean the database has
//! no jobs/queues/messages yet, not that the subsystem is missing.

use anyhow::Context;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn parse_i64(v: Option<&Value>, default: i64) -> i64 {
    v.and_then(Value::as_i64).unwrap_or(default)
}

fn parse_str<'a>(v: Option<&'a Value>, default: &'a str) -> &'a str {
    v.and_then(Value::as_str).unwrap_or(default)
}

fn parse_json(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}

fn parse_dt(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sk_jobs (
            id TEXT PRIMARY KEY,
            kind TEXT NOT NULL,
            status TEXT NOT NULL,
            owner_type TEXT,
            owner_id TEXT,
            input_json TEXT,
            result_json TEXT,
            error TEXT,
            attempts INTEGER NOT NULL DEFAULT 0,
            max_attempts INTEGER NOT NULL DEFAULT 1,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_sk_jobs_status ON sk_jobs(status);
        CREATE INDEX IF NOT EXISTS idx_sk_jobs_kind ON sk_jobs(kind);
        CREATE INDEX IF NOT EXISTS idx_sk_jobs_owner ON sk_jobs(owner_type, owner_id);
        CREATE TABLE IF NOT EXISTS sk_scheduled_jobs (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            kind TEXT NOT NULL,
            cron TEXT,
            enabled INTEGER NOT NULL DEFAULT 1,
            payload_json TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_run_at TEXT
        );
        CREATE TABLE IF NOT EXISTS sk_queue_groups (
            slug TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            owner_type TEXT,
            owner_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_queues (
            group_slug TEXT NOT NULL,
            slug TEXT NOT NULL,
            name TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (group_slug, slug),
            FOREIGN KEY(group_slug) REFERENCES sk_queue_groups(slug) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS sk_queue_messages (
            id TEXT PRIMARY KEY,
            group_slug TEXT NOT NULL,
            queue_slug TEXT NOT NULL,
            status TEXT NOT NULL,
            payload_json TEXT NOT NULL,
            priority INTEGER NOT NULL DEFAULT 0,
            attempts INTEGER NOT NULL DEFAULT 0,
            max_attempts INTEGER NOT NULL DEFAULT 3,
            available_at TEXT NOT NULL,
            locked_until TEXT,
            error_message TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            FOREIGN KEY(group_slug, queue_slug) REFERENCES sk_queues(group_slug, slug) ON DELETE CASCADE
        );
        CREATE INDEX IF NOT EXISTS idx_sk_queue_messages_lookup ON sk_queue_messages(group_slug, queue_slug, status, available_at, priority);
        "#,
    )
    .execute(pool)
    .await
    .context("ensure sk-jobs schema")?;
    Ok(())
}

fn job_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "kind": row.get::<String, _>("kind"),
        "status": row.get::<String, _>("status"),
        "owner_type": row.try_get::<Option<String>, _>("owner_type").ok().flatten(),
        "owner_id": row.try_get::<Option<String>, _>("owner_id").ok().flatten(),
        "input": parse_json(row.try_get::<Option<String>, _>("input_json").ok().flatten()),
        "result": parse_json(row.try_get::<Option<String>, _>("result_json").ok().flatten()),
        "error": row.try_get::<Option<String>, _>("error").ok().flatten(),
        "attempts": row.get::<i64, _>("attempts"),
        "max_attempts": row.get::<i64, _>("max_attempts"),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
        "started_at": row.try_get::<Option<String>, _>("started_at").ok().flatten(),
        "finished_at": row.try_get::<Option<String>, _>("finished_at").ok().flatten(),
    })
}

pub async fn list_jobs(pool: &SqlitePool, filters: &Value) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_jobs ORDER BY created_at DESC LIMIT ? OFFSET ?")
        .bind(parse_i64(filters.get("limit"), 100))
        .bind(parse_i64(filters.get("offset"), 0))
        .fetch_all(pool)
        .await?;
    let mut jobs: Vec<Value> = rows.iter().map(job_value).collect();
    if let Some(status) = filters.get("status").and_then(Value::as_str) {
        jobs.retain(|j| j["status"] == json!(status));
    }
    if let Some(kind) = filters.get("kind").and_then(Value::as_str) {
        jobs.retain(|j| j["kind"] == json!(kind));
    }
    if let Some(owner_type) = filters.get("owner_type").and_then(Value::as_str) {
        jobs.retain(|j| j["owner_type"] == json!(owner_type));
    }
    if let Some(owner_id) = filters.get("owner_id").and_then(Value::as_str) {
        jobs.retain(|j| j["owner_id"] == json!(owner_id));
    }
    Ok(json!({ "jobs": jobs, "total": jobs.len() }))
}

pub async fn get_job(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(job_value))
}

pub async fn insert_job(pool: &SqlitePool, kind: &str, input: Value) -> anyhow::Result<Value> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    sqlx::query("INSERT INTO sk_jobs (id, kind, status, input_json, created_at, updated_at) VALUES (?, ?, 'queued', ?, ?, ?)")
        .bind(&id)
        .bind(kind)
        .bind(input.to_string())
        .bind(&ts)
        .bind(&ts)
        .execute(pool)
        .await?;
    Ok(get_job(pool, &id).await?.expect("inserted job exists"))
}

pub async fn cancel_job(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let ts = now();
    sqlx::query("UPDATE sk_jobs SET status = 'cancelled', updated_at = ?, finished_at = COALESCE(finished_at, ?) WHERE id = ? AND status NOT IN ('completed','failed','cancelled')")
        .bind(&ts)
        .bind(&ts)
        .bind(id)
        .execute(pool)
        .await?;
    get_job(pool, id).await
}

pub async fn retry_job(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let ts = now();
    sqlx::query("UPDATE sk_jobs SET status = 'queued', error = NULL, updated_at = ?, finished_at = NULL WHERE id = ?")
        .bind(&ts)
        .bind(id)
        .execute(pool)
        .await?;
    get_job(pool, id).await
}

pub async fn job_stats(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT status, COUNT(*) AS n FROM sk_jobs GROUP BY status")
        .fetch_all(pool)
        .await?;
    let mut out = json!({ "total": 0, "queued": 0, "running": 0, "completed": 0, "failed": 0, "cancelled": 0 });
    for row in rows {
        let status: String = row.get("status");
        let n: i64 = row.get("n");
        out["total"] = json!(out["total"].as_i64().unwrap_or(0) + n);
        out[&status] = json!(n);
    }
    Ok(out)
}

pub async fn job_kinds(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT DISTINCT kind FROM sk_jobs UNION SELECT DISTINCT kind FROM sk_scheduled_jobs ORDER BY kind")
        .fetch_all(pool)
        .await?;
    Ok(json!({ "kinds": rows.into_iter().map(|r| r.get::<String, _>("kind")).collect::<Vec<_>>() }))
}

fn scheduled_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "kind": row.get::<String, _>("kind"),
        "cron": row.try_get::<Option<String>, _>("cron").ok().flatten(),
        "enabled": row.get::<i64, _>("enabled") != 0,
        "payload": parse_json(row.try_get::<Option<String>, _>("payload_json").ok().flatten()),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
        "last_run_at": row.try_get::<Option<String>, _>("last_run_at").ok().flatten(),
    })
}

pub async fn scheduled_jobs(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_scheduled_jobs ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({ "scheduled": rows.iter().map(scheduled_value).collect::<Vec<_>>() }))
}

pub async fn set_scheduled_enabled(
    pool: &SqlitePool,
    id: &str,
    enabled: bool,
) -> anyhow::Result<Option<Value>> {
    let ts = now();
    sqlx::query("UPDATE sk_scheduled_jobs SET enabled = ?, updated_at = ? WHERE id = ?")
        .bind(if enabled { 1 } else { 0 })
        .bind(&ts)
        .bind(id)
        .execute(pool)
        .await?;
    let row = sqlx::query("SELECT * FROM sk_scheduled_jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(scheduled_value))
}

pub async fn run_scheduled(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_scheduled_jobs WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let kind: String = row.get("kind");
    let payload = parse_json(
        row.try_get::<Option<String>, _>("payload_json")
            .ok()
            .flatten(),
    );
    let job = insert_job(pool, &kind, payload).await?;
    sqlx::query("UPDATE sk_scheduled_jobs SET last_run_at = ?, updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(Some(job))
}

fn group_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "slug": row.get::<String, _>("slug"),
        "name": row.get::<String, _>("name"),
        "owner_type": row.try_get::<Option<String>, _>("owner_type").ok().flatten(),
        "owner_id": row.try_get::<Option<String>, _>("owner_id").ok().flatten(),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
    })
}

fn queue_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "group_slug": row.get::<String, _>("group_slug"),
        "slug": row.get::<String, _>("slug"),
        "name": row.get::<String, _>("name"),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
    })
}

fn message_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String, _>("id"),
        "group_slug": row.get::<String, _>("group_slug"),
        "queue_slug": row.get::<String, _>("queue_slug"),
        "status": row.get::<String, _>("status"),
        "payload": parse_json(Some(row.get::<String, _>("payload_json"))),
        "priority": row.get::<i64, _>("priority"),
        "attempts": row.get::<i64, _>("attempts"),
        "max_attempts": row.get::<i64, _>("max_attempts"),
        "available_at": row.get::<String, _>("available_at"),
        "locked_until": row.try_get::<Option<String>, _>("locked_until").ok().flatten(),
        "error_message": row.try_get::<Option<String>, _>("error_message").ok().flatten(),
        "created_at": row.get::<String, _>("created_at"),
        "updated_at": row.get::<String, _>("updated_at"),
    })
}

pub async fn queue_groups(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_queue_groups ORDER BY slug")
        .fetch_all(pool)
        .await?;
    Ok(json!({ "groups": rows.iter().map(group_value).collect::<Vec<_>>() }))
}

pub async fn create_group(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let slug = parse_str(body.get("slug"), parse_str(body.get("name"), "default")).to_string();
    let name = parse_str(body.get("name"), &slug).to_string();
    let ts = now();
    sqlx::query("INSERT INTO sk_queue_groups (slug, name, owner_type, owner_id, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&slug)
        .bind(&name)
        .bind(body.get("owner_type").and_then(Value::as_str))
        .bind(body.get("owner_id").and_then(Value::as_str))
        .bind(&ts)
        .bind(&ts)
        .execute(pool)
        .await?;
    get_group(pool, &slug)
        .await?
        .context("created group missing")
}

pub async fn get_group(pool: &SqlitePool, slug: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_queue_groups WHERE slug = ?")
        .bind(slug)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(group_value))
}

pub async fn update_group(
    pool: &SqlitePool,
    slug: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let name = body.get("name").and_then(Value::as_str).unwrap_or(slug);
    sqlx::query("UPDATE sk_queue_groups SET name = ?, updated_at = ? WHERE slug = ?")
        .bind(name)
        .bind(now())
        .bind(slug)
        .execute(pool)
        .await?;
    get_group(pool, slug).await
}

pub async fn delete_group(pool: &SqlitePool, slug: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM sk_queue_groups WHERE slug = ?")
        .bind(slug)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn queues(pool: &SqlitePool, group: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_queues WHERE group_slug = ? ORDER BY slug")
        .bind(group)
        .fetch_all(pool)
        .await?;
    Ok(json!({ "queues": rows.iter().map(queue_value).collect::<Vec<_>>() }))
}

pub async fn create_queue(pool: &SqlitePool, group: &str, body: &Value) -> anyhow::Result<Value> {
    let slug = parse_str(body.get("slug"), parse_str(body.get("name"), "default")).to_string();
    let name = parse_str(body.get("name"), &slug).to_string();
    let ts = now();
    sqlx::query("INSERT INTO sk_queues (group_slug, slug, name, created_at, updated_at) VALUES (?, ?, ?, ?, ?)")
        .bind(group).bind(&slug).bind(&name).bind(&ts).bind(&ts).execute(pool).await?;
    get_queue(pool, group, &slug)
        .await?
        .context("created queue missing")
}

pub async fn get_queue(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_queues WHERE group_slug = ? AND slug = ?")
        .bind(group)
        .bind(queue)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(queue_value))
}

pub async fn update_queue(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let name = body.get("name").and_then(Value::as_str).unwrap_or(queue);
    sqlx::query("UPDATE sk_queues SET name = ?, updated_at = ? WHERE group_slug = ? AND slug = ?")
        .bind(name)
        .bind(now())
        .bind(group)
        .bind(queue)
        .execute(pool)
        .await?;
    get_queue(pool, group, queue).await
}

pub async fn delete_queue(pool: &SqlitePool, group: &str, queue: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM sk_queues WHERE group_slug = ? AND slug = ?")
        .bind(group)
        .bind(queue)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn messages(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
    status: Option<&str>,
) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_queue_messages WHERE group_slug = ? AND queue_slug = ? ORDER BY priority DESC, created_at ASC")
        .bind(group).bind(queue).fetch_all(pool).await?;
    let mut messages: Vec<Value> = rows.iter().map(message_value).collect();
    if let Some(status) = status {
        messages.retain(|m| m["status"] == json!(status));
    }
    Ok(json!({ "messages": messages, "total": messages.len() }))
}

pub async fn send_message(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let id = Uuid::new_v4().to_string();
    let ts = now();
    let delay = parse_i64(body.get("delay_ms"), 0);
    let available = (Utc::now() + Duration::milliseconds(delay)).to_rfc3339();
    let payload = body.get("payload").cloned().unwrap_or(Value::Null);
    sqlx::query("INSERT INTO sk_queue_messages (id, group_slug, queue_slug, status, payload_json, priority, max_attempts, available_at, created_at, updated_at) VALUES (?, ?, ?, 'pending', ?, ?, ?, ?, ?, ?)")
        .bind(&id).bind(group).bind(queue).bind(payload.to_string())
        .bind(parse_i64(body.get("priority"), 0))
        .bind(parse_i64(body.get("max_attempts"), 3))
        .bind(available).bind(&ts).bind(&ts).execute(pool).await?;
    get_message(pool, group, queue, &id)
        .await?
        .context("created message missing")
}

pub async fn receive_messages(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let max = parse_i64(body.get("max_messages"), 1).clamp(1, 50);
    let visibility = parse_i64(body.get("visibility_timeout_ms"), 30_000);
    let now_dt = Utc::now();
    let lock_until = (now_dt + Duration::milliseconds(visibility)).to_rfc3339();
    let rows = sqlx::query("SELECT * FROM sk_queue_messages WHERE group_slug = ? AND queue_slug = ? AND status = 'pending' ORDER BY priority DESC, created_at ASC LIMIT ?")
        .bind(group).bind(queue).bind(max).fetch_all(pool).await?;
    let ids: Vec<String> = rows.iter().map(|r| r.get::<String, _>("id")).collect();
    let mut received = Vec::new();
    for id in ids {
        if let Some(msg) = get_message(pool, group, queue, &id).await? {
            if parse_dt(msg["available_at"].as_str().unwrap_or("")) <= Some(now_dt) {
                sqlx::query("UPDATE sk_queue_messages SET status = 'in_progress', attempts = attempts + 1, locked_until = ?, updated_at = ? WHERE id = ?")
                    .bind(&lock_until).bind(now()).bind(&id).execute(pool).await?;
                if let Some(updated) = get_message(pool, group, queue, &id).await? {
                    received.push(updated);
                }
            }
        }
    }
    Ok(json!({ "messages": received }))
}

pub async fn get_message(
    pool: &SqlitePool,
    group: &str,
    queue: &str,
    id: &str,
) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query(
        "SELECT * FROM sk_queue_messages WHERE group_slug = ? AND queue_slug = ? AND id = ?",
    )
    .bind(group)
    .bind(queue)
    .bind(id)
    .fetch_optional(pool)
    .await?;
    Ok(row.as_ref().map(message_value))
}

pub async fn complete_message(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE sk_queue_messages SET status = 'completed', updated_at = ? WHERE id = ?")
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn fail_message(pool: &SqlitePool, id: &str, body: &Value) -> anyhow::Result<()> {
    let requeue = body
        .get("requeue")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let status = if requeue { "pending" } else { "failed" };
    sqlx::query("UPDATE sk_queue_messages SET status = ?, error_message = ?, locked_until = NULL, updated_at = ? WHERE id = ?")
        .bind(status)
        .bind(body.get("error_message").and_then(Value::as_str))
        .bind(now()).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn requeue_message(pool: &SqlitePool, id: &str) -> anyhow::Result<()> {
    sqlx::query("UPDATE sk_queue_messages SET status = 'pending', locked_until = NULL, updated_at = ? WHERE id = ?")
        .bind(now()).bind(id).execute(pool).await?;
    Ok(())
}

pub async fn delete_message(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    let r = sqlx::query("DELETE FROM sk_queue_messages WHERE id = ?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(r.rows_affected() > 0)
}

pub async fn queue_stats(
    pool: &SqlitePool,
    group: Option<&str>,
    queue: Option<&str>,
) -> anyhow::Result<Value> {
    let rows = match (group, queue) {
        (Some(g), Some(q)) => sqlx::query("SELECT status, COUNT(*) AS n FROM sk_queue_messages WHERE group_slug = ? AND queue_slug = ? GROUP BY status").bind(g).bind(q).fetch_all(pool).await?,
        (Some(g), None) => sqlx::query("SELECT status, COUNT(*) AS n FROM sk_queue_messages WHERE group_slug = ? GROUP BY status").bind(g).fetch_all(pool).await?,
        _ => sqlx::query("SELECT status, COUNT(*) AS n FROM sk_queue_messages GROUP BY status").fetch_all(pool).await?,
    };
    let mut out =
        json!({ "total": 0, "pending": 0, "in_progress": 0, "completed": 0, "failed": 0 });
    for row in rows {
        let status: String = row.get("status");
        let n: i64 = row.get("n");
        out["total"] = json!(out["total"].as_i64().unwrap_or(0) + n);
        out[&status] = json!(n);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn queue_message_lifecycle() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        create_group(&pool, &json!({"slug":"g","name":"Group"}))
            .await
            .unwrap();
        create_queue(&pool, "g", &json!({"slug":"q","name":"Queue"}))
            .await
            .unwrap();
        let msg = send_message(&pool, "g", "q", &json!({"payload":{"a":1}}))
            .await
            .unwrap();
        assert_eq!(msg["status"], "pending");
        let got = receive_messages(&pool, "g", "q", &json!({"max_messages":1}))
            .await
            .unwrap();
        let id = got["messages"][0]["id"].as_str().unwrap();
        complete_message(&pool, id).await.unwrap();
        let stats = queue_stats(&pool, Some("g"), Some("q")).await.unwrap();
        assert_eq!(stats["completed"], 1);
    }
}
