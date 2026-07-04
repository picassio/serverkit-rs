//! Port of `app/api/system.py` (dashboard-relevant endpoints) and
//! `/api/v1/metrics/history` from `app/api/metrics.py`.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/metrics", get(metrics))
        .route("/info", get(info))
        .route("/version", get(version))
        .route("/notices", get(notices))
        .route("/resource-tier", get(resource_tier))
}

pub fn metrics_router() -> Router<SharedState> {
    Router::new().route("/history", get(metrics_history))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

/// GET /system/metrics
async fn metrics(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    Ok(Json(sk_system::all_metrics().await))
}

/// GET /system/info
async fn info(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    Ok(Json(sk_system::system_info()))
}

/// GET /system/version
async fn version(AuthUser(_user): AuthUser) -> Json<Value> {
    Json(json!({
        "version": env!("CARGO_PKG_VERSION"),
        "name": "ServerKit",
        "install_dir": std::env::var("SERVERKIT_INSTALL_DIR")
            .unwrap_or_else(|_| "/opt/serverkit".into()),
    }))
}

/// GET /system/notices — TODO(P1): canonical-domain/misconfiguration checks
/// (see Flask `get_system_notices`). Empty list keeps the UI quiet.
async fn notices(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    Ok(Json(json!({ "notices": [] })))
}

/// GET /system/resource-tier
async fn resource_tier(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    Ok(Json(sk_system::resource_tier().await))
}

#[derive(Deserialize)]
struct HistoryQuery {
    period: Option<String>,
}

/// GET /metrics/history — reads the `metrics_history` table (same schema the
/// Flask collector writes). TODO(P1): background sampler to populate it.
async fn metrics_history(
    State(state): State<SharedState>,
    AuthUser(_user): AuthUser,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Value>> {
    let period = q.period.unwrap_or_else(|| "1h".into());
    let (level, hours_back) = match period.as_str() {
        "1h" => ("minute", 1),
        "6h" => ("minute", 6),
        "24h" => ("minute", 24),
        "7d" => ("hour", 24 * 7),
        "30d" => ("day", 24 * 30),
        _ => {
            return Err(ApiError::bad_request(
                "Invalid period. Must be one of: 1h, 6h, 24h, 7d, 30d",
            ))
        }
    };

    let cutoff = (sk_core::time::now_naive() - chrono::Duration::hours(hours_back))
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string();

    #[derive(sqlx::FromRow)]
    struct Row {
        timestamp: String,
        level: String,
        cpu_percent: f64,
        cpu_percent_min: Option<f64>,
        cpu_percent_max: Option<f64>,
        memory_percent: f64,
        memory_used_bytes: i64,
        memory_total_bytes: i64,
        disk_percent: f64,
        disk_used_bytes: i64,
        disk_total_bytes: i64,
    }

    let rows: Vec<Row> = sqlx::query_as(
        "SELECT timestamp, level, cpu_percent, cpu_percent_min, cpu_percent_max, \
         memory_percent, memory_used_bytes, memory_total_bytes, \
         disk_percent, disk_used_bytes, disk_total_bytes \
         FROM metrics_history WHERE level = ? AND timestamp >= ? ORDER BY timestamp ASC",
    )
    .bind(level)
    .bind(&cutoff)
    .fetch_all(&state.db)
    .await
    .map_err(anyhow::Error::from)?;

    let gb = 1024f64.powi(3);
    let data: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "timestamp": format!("{}Z", sk_core::time::to_isoformat(&r.timestamp)),
                "level": r.level,
                "cpu": {
                    "percent": round1(r.cpu_percent),
                    "min": r.cpu_percent_min.map(round1),
                    "max": r.cpu_percent_max.map(round1),
                },
                "memory": {
                    "percent": round1(r.memory_percent),
                    "used_bytes": r.memory_used_bytes,
                    "total_bytes": r.memory_total_bytes,
                    "used_gb": round2(r.memory_used_bytes as f64 / gb),
                    "total_gb": round2(r.memory_total_bytes as f64 / gb),
                },
                "disk": {
                    "percent": round1(r.disk_percent),
                    "used_bytes": r.disk_used_bytes,
                    "total_bytes": r.disk_total_bytes,
                },
            })
        })
        .collect();

    let avg = |f: fn(&Row) -> f64| -> f64 {
        if rows.is_empty() {
            0.0
        } else {
            round1(rows.iter().map(f).sum::<f64>() / rows.len() as f64)
        }
    };

    Ok(Json(json!({
        "period": period,
        "level": level,
        "points": data.len(),
        "data": data,
        "summary": {
            "cpu_avg": avg(|r| r.cpu_percent),
            "memory_avg": avg(|r| r.memory_percent),
            "disk_avg": avg(|r| r.disk_percent),
        }
    })))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
