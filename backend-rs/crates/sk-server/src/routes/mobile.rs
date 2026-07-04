use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::Row;
use uuid::Uuid;

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn internal(e: anyhow::Error) -> ApiError {
    e.into()
}

pub async fn ensure_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS sk_mobile_push_devices(
            id TEXT PRIMARY KEY,
            user_id TEXT NOT NULL,
            endpoint TEXT NOT NULL UNIQUE,
            device_name TEXT,
            subscription_json TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            last_seen_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_sk_mobile_push_user ON sk_mobile_push_devices(user_id);
        "#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/push/register", post(register_push))
        .route("/push/unregister", post(unregister_push))
        .route("/quick-actions", get(quick_actions))
        .route("/quick-actions/{id}", post(execute_action))
        .route("/summary", get(summary))
        .route("/offline-cache", get(offline_cache))
}

fn endpoint(body: &Value) -> Option<String> {
    body.get("endpoint")
        .and_then(Value::as_str)
        .map(str::to_string)
        .or_else(|| {
            body.get("subscription")
                .and_then(|s| s.get("endpoint"))
                .and_then(Value::as_str)
                .map(str::to_string)
        })
}

async fn register_push(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let Some(endpoint) = endpoint(&body) else {
        return Err(ApiError::bad_request("endpoint is required"));
    };
    let sub = body
        .get("subscription")
        .cloned()
        .unwrap_or_else(|| body.clone());
    let device = body
        .get("device_name")
        .and_then(Value::as_str)
        .unwrap_or("Mobile device");
    let did = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_mobile_push_devices(id,user_id,endpoint,device_name,subscription_json,created_at,updated_at,last_seen_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(endpoint) DO UPDATE SET user_id=excluded.user_id,device_name=excluded.device_name,subscription_json=excluded.subscription_json,updated_at=excluded.updated_at,last_seen_at=excluded.last_seen_at")
        .bind(&did)
        .bind(u.id.to_string())
        .bind(&endpoint)
        .bind(device)
        .bind(sub.to_string())
        .bind(&ts)
        .bind(&ts)
        .bind(&ts)
        .execute(&s.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({
        "success": true,
        "device": {"endpoint": endpoint, "device_name": device, "last_seen_at": ts}
    })))
}

async fn unregister_push(
    State(s): State<SharedState>,
    AuthUser(u): AuthUser,
    Json(body): Json<Value>,
) -> ApiResult<Json<Value>> {
    let Some(ep) = endpoint(&body) else {
        return Err(ApiError::bad_request("endpoint is required"));
    };
    let r = sqlx::query("DELETE FROM sk_mobile_push_devices WHERE endpoint=? AND user_id=?")
        .bind(ep)
        .bind(u.id.to_string())
        .execute(&s.db)
        .await
        .map_err(ApiError::from)?;
    Ok(Json(json!({"success": true, "deleted": r.rows_affected()})))
}

fn actions() -> Value {
    json!([
        {"id":"refresh-summary","label":"Refresh summary","description":"Return the latest mobile dashboard summary","safe":true},
        {"id":"mark-notifications-read","label":"Mark notifications read","description":"Mark unread notifications as read for the panel","safe":true},
        {"id":"clear-managed-cache","label":"Clear managed cache","description":"Flush ServerKit managed cache via the performance subsystem","safe":true}
    ])
}

async fn quick_actions(AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(json!({"success": true, "actions": actions()})))
}

async fn summary_value(pool: &sqlx::SqlitePool) -> anyhow::Result<Value> {
    let jobs = sk_jobs::job_stats(pool).await.unwrap_or_else(|_| json!({}));
    let unread = sqlx::query("SELECT COUNT(*) n FROM sk_notifications WHERE status!='read'")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("n"))
        .unwrap_or(0);
    let devices = sqlx::query("SELECT COUNT(*) n FROM sk_mobile_push_devices")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("n"))
        .unwrap_or(0);
    let events = sqlx::query("SELECT COUNT(*) n FROM sk_telemetry_events")
        .fetch_one(pool)
        .await
        .map(|r| r.get::<i64, _>("n"))
        .unwrap_or(0);
    let metrics = sk_system::all_metrics().await;
    Ok(json!({
        "success": true,
        "summary": {
            "health": {"status":"ok","metrics": metrics},
            "jobs": jobs,
            "unread_notifications": unread,
            "push_devices": devices,
            "telemetry_events": events,
            "generated_at": now()
        }
    }))
}

async fn summary(State(s): State<SharedState>, AuthUser(_): AuthUser) -> ApiResult<Json<Value>> {
    Ok(Json(summary_value(&s.db).await.map_err(internal)?))
}

async fn execute_action(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(aid): Path<String>,
) -> ApiResult<Json<Value>> {
    match aid.as_str() {
        "refresh-summary" => Ok(Json(summary_value(&s.db).await.map_err(internal)?)),
        "mark-notifications-read" => {
            let ts = now();
            let r = sqlx::query("UPDATE sk_notifications SET status='read', read_at=COALESCE(read_at, ?), updated_at=? WHERE status!='read'")
                .bind(&ts)
                .bind(&ts)
                .execute(&s.db)
                .await
                .map_err(ApiError::from)?;
            Ok(Json(
                json!({"success": true, "marked_read": r.rows_affected()}),
            ))
        }
        "clear-managed-cache" => Ok(Json(json!({
            "success": false,
            "code": "USE_PERFORMANCE_CACHE_FLUSH",
            "error": "Use POST /performance/cache/flush for cache mutation"
        }))),
        _ => Err(ApiError::not_found("Quick action not found")),
    }
}

async fn offline_cache(
    State(s): State<SharedState>,
    AuthUser(_): AuthUser,
) -> ApiResult<Json<Value>> {
    let mut resources = Vec::new();
    for (name, sql) in [
        (
            "notifications",
            "SELECT COUNT(*) n, MAX(updated_at) ts FROM sk_notifications",
        ),
        (
            "telemetry",
            "SELECT COUNT(*) n, MAX(created_at) ts FROM sk_telemetry_events",
        ),
        ("jobs", "SELECT COUNT(*) n, MAX(updated_at) ts FROM sk_jobs"),
        (
            "push_devices",
            "SELECT COUNT(*) n, MAX(updated_at) ts FROM sk_mobile_push_devices",
        ),
    ] {
        if let Ok(r) = sqlx::query(sql).fetch_one(&s.db).await {
            resources.push(json!({
                "resource": name,
                "count": r.get::<i64, _>("n"),
                "updated_at": r.try_get::<Option<String>, _>("ts").ok().flatten()
            }));
        }
    }
    Ok(Json(json!({
        "success": true,
        "cache": {
            "version": 1,
            "generated_at": now(),
            "resources": resources,
            "routes": ["/dashboard", "/settings", "/notifications"]
        }
    })))
}
