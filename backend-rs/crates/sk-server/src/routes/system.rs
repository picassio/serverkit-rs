//! Port of `app/api/system.py` (dashboard-relevant endpoints) and
//! `/api/v1/metrics/history` from `app/api/metrics.py`.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Query, State};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub async fn ensure_schema(pool: &sqlx::SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS sk_metrics_collection_state(id INTEGER PRIMARY KEY CHECK(id=1), collecting INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_metrics_aggregates(id TEXT PRIMARY KEY, summary_json TEXT NOT NULL, created_at TEXT NOT NULL);
INSERT OR IGNORE INTO sk_metrics_collection_state(id, collecting, updated_at) VALUES(1, 1, datetime('now'));
"#,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/metrics", get(metrics))
        .route("/info", get(info))
        .route("/health", get(health))
        .route("/processes", get(system_processes))
        .route("/services", get(system_services))
        .route("/time", get(time))
        .route("/timezones", get(timezones))
        .route("/timezone", put(set_timezone))
        .route("/version", get(version))
        .route("/check-update", get(check_update))
        .route("/upgrade", post(upgrade))
        .route("/notices", get(notices))
        .route("/resource-tier", get(resource_tier))
}

pub fn metrics_router() -> Router<SharedState> {
    Router::new()
        .route("/history", get(metrics_history))
        .route("/stats", get(metrics_stats))
        .route("/collection/start", post(metrics_collection_start))
        .route("/collection/stop", post(metrics_collection_stop))
        .route("/aggregate", post(metrics_aggregate))
}

fn require_admin(user: &sk_models::user::User) -> ApiResult<()> {
    if !user.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

fn status_of(v: &Value) -> axum::http::StatusCode {
    if v.get("success").and_then(|s| s.as_bool()).unwrap_or(true) {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::BAD_REQUEST
    }
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

/// GET /system/health
async fn health(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let metrics = sk_system::all_metrics().await;
    let disk_ok = metrics["disk"]["partitions"]
        .as_array()
        .map(|parts| {
            parts
                .iter()
                .all(|p| p["percent"].as_f64().unwrap_or(0.0) < 95.0)
        })
        .unwrap_or(true);
    let mem_ok = metrics["memory"]["ram"]["percent"].as_f64().unwrap_or(0.0) < 95.0;
    let cpu_ok = metrics["cpu"]["percent"].as_f64().unwrap_or(0.0) < 95.0;
    Ok(Json(json!({
        "status": if disk_ok && mem_ok && cpu_ok { "ok" } else { "degraded" },
        "checks": { "cpu": cpu_ok, "memory": mem_ok, "disk": disk_ok },
        "metrics": metrics,
    })))
}

/// GET /system/processes
async fn system_processes(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let processes = sk_system::processes::list(50, "cpu").await;
    Ok(Json(json!({ "processes": processes })))
}

/// GET /system/services
async fn system_services(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let running = sk_system::processes::running_names().await;
    Ok(Json(
        json!({ "services": sk_ops::services::services_status(&running) }),
    ))
}

/// GET /system/time
async fn time(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    Ok(Json(sk_system::server_time()))
}

/// GET /system/timezones
async fn timezones(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let zones = std::fs::read_to_string("/usr/share/zoneinfo/zone1970.tab")
        .or_else(|_| std::fs::read_to_string("/usr/share/zoneinfo/zone.tab"))
        .map(|content| {
            content
                .lines()
                .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
                .filter_map(|l| l.split_whitespace().nth(2).map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_else(|_| vec!["UTC".to_string()]);
    Ok(Json(json!({ "timezones": zones })))
}

#[derive(Deserialize)]
struct TimezoneBody {
    timezone: String,
}

/// PUT /system/timezone
async fn set_timezone(
    AuthUser(user): AuthUser,
    Json(body): Json<TimezoneBody>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    require_admin(&user)?;
    let zone_path = std::path::Path::new("/usr/share/zoneinfo").join(&body.timezone);
    if body.timezone.contains("..") || body.timezone.starts_with('/') || !zone_path.exists() {
        return Err(ApiError::bad_request("Invalid timezone"));
    }
    let result = std::process::Command::new("timedatectl")
        .args(["set-timezone", &body.timezone])
        .output();
    let value = match result {
        Ok(o) if o.status.success() => {
            json!({ "success": true, "timezone": body.timezone, "time": sk_system::server_time() })
        }
        Ok(o) => {
            json!({ "success": false, "code": "TIMEZONE_SET_FAILED", "stderr": String::from_utf8_lossy(&o.stderr).trim() })
        }
        Err(e) => {
            json!({ "success": false, "code": "TIMEZONE_SET_UNAVAILABLE", "error": e.to_string() })
        }
    };
    Ok((status_of(&value), Json(value)))
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

/// Compare dotted versions: is `latest` newer than `current`?
fn is_newer(latest: &str, current: &str) -> bool {
    let parse = |s: &str| -> Vec<u64> {
        s.trim_start_matches('v')
            .split(['.', '-'])
            .map(|p| {
                p.chars()
                    .take_while(|c| c.is_ascii_digit())
                    .collect::<String>()
            })
            .map(|p| p.parse().unwrap_or(0))
            .collect()
    };
    let (a, b) = (parse(latest), parse(current));
    for i in 0..a.len().max(b.len()) {
        let (x, y) = (
            a.get(i).copied().unwrap_or(0),
            b.get(i).copied().unwrap_or(0),
        );
        if x != y {
            return x > y;
        }
    }
    false
}

fn serverkit_repo() -> String {
    std::env::var("SERVERKIT_REPO").unwrap_or_else(|_| "picassio/serverkit-rs".into())
}

/// GET /system/check-update — compare the running version with the latest
/// GitHub release.
async fn check_update(AuthUser(_user): AuthUser) -> Json<Value> {
    let current = env!("CARGO_PKG_VERSION");
    let repo = serverkit_repo();
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let resp = reqwest::Client::new()
        .get(&url)
        .header("User-Agent", "serverkit-rs")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await;
    match resp {
        Ok(r) if r.status().is_success() => {
            let v: Value = r.json().await.unwrap_or(json!({}));
            let tag = v["tag_name"].as_str().unwrap_or("").to_string();
            let latest = tag.trim_start_matches('v').to_string();
            Json(json!({
                "current_version": current,
                "latest_version": latest,
                "update_available": !latest.is_empty() && is_newer(&latest, current),
                "release_url": v["html_url"].as_str().unwrap_or(""),
                "notes": v["body"].as_str().unwrap_or(""),
                "repo": repo,
            }))
        }
        _ => Json(json!({
            "current_version": current,
            "latest_version": null,
            "update_available": false,
            "error": "Could not reach GitHub",
        })),
    }
}

/// POST /system/upgrade — launch the installer in a detached transient unit so
/// it survives our own restart, pulling + installing the latest release.
async fn upgrade(AuthUser(user): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let repo = serverkit_repo();
    let script = format!(
        "curl -fsSL https://raw.githubusercontent.com/{repo}/main/install.sh | SK_SKIP_PREPARE=1 bash"
    );
    let logged = format!("{script} > /var/log/serverkit-upgrade.log 2>&1");
    // Prefer systemd-run (separate cgroup: survives `systemctl restart serverkit`).
    let via_systemd = std::process::Command::new("systemd-run")
        .args([
            "--collect",
            "--unit=serverkit-upgrade",
            "--property=Type=oneshot",
            "bash",
            "-lc",
            &logged,
        ])
        .spawn()
        .is_ok();
    if !via_systemd {
        // Fallback: detach with setsid so a plain restart doesn't kill it.
        let _ = std::process::Command::new("setsid")
            .args(["bash", "-lc", &format!("nohup {logged} &")])
            .spawn();
    }
    Ok(Json(json!({
        "started": true,
        "method": if via_systemd { "systemd-run" } else { "setsid" },
        "log": "/var/log/serverkit-upgrade.log",
        "note": "The panel will restart when the upgrade completes."
    })))
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

async fn metrics_stats(
    State(state): State<SharedState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let total: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM metrics_history")
        .fetch_one(&state.db)
        .await
        .unwrap_or(0);
    let minute: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM metrics_history WHERE level='minute'")
            .fetch_one(&state.db)
            .await
            .unwrap_or(0);
    let latest: Option<String> =
        sqlx::query_scalar("SELECT timestamp FROM metrics_history ORDER BY timestamp DESC LIMIT 1")
            .fetch_optional(&state.db)
            .await
            .unwrap_or(None);
    let collecting: i64 =
        sqlx::query_scalar("SELECT collecting FROM sk_metrics_collection_state WHERE id=1")
            .fetch_one(&state.db)
            .await
            .unwrap_or(1);
    Ok(Json(json!({
        "success": true,
        "collection": { "running": collecting != 0, "interval_seconds": 60 },
        "history": { "total_points": total, "minute_points": minute, "latest": latest },
        "live": sk_system::all_metrics().await,
    })))
}

async fn set_collection(
    state: SharedState,
    user: sk_models::user::User,
    running: bool,
) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    let ts = sk_core::time::now_sql();
    sqlx::query("INSERT INTO sk_metrics_collection_state(id, collecting, updated_at) VALUES(1, ?, ?) ON CONFLICT(id) DO UPDATE SET collecting=excluded.collecting, updated_at=excluded.updated_at")
        .bind(if running { 1 } else { 0 })
        .bind(&ts)
        .execute(&state.db)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(
        json!({ "success": true, "running": running, "updated_at": ts }),
    ))
}

async fn metrics_collection_start(
    State(state): State<SharedState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Value>> {
    set_collection(state, user, true).await
}

async fn metrics_collection_stop(
    State(state): State<SharedState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Value>> {
    set_collection(state, user, false).await
}

async fn metrics_aggregate(
    State(state): State<SharedState>,
    AuthUser(user): AuthUser,
) -> ApiResult<Json<Value>> {
    require_admin(&user)?;
    #[derive(sqlx::FromRow)]
    struct Agg {
        sample_count: i64,
        cpu_avg: Option<f64>,
        memory_avg: Option<f64>,
        disk_avg: Option<f64>,
    }
    let agg: Agg = sqlx::query_as(
        "SELECT COUNT(*) sample_count, AVG(cpu_percent) cpu_avg, AVG(memory_percent) memory_avg, AVG(disk_percent) disk_avg FROM metrics_history WHERE level='minute'",
    )
    .fetch_one(&state.db)
    .await
    .map_err(anyhow::Error::from)?;
    let summary = json!({
        "sample_count": agg.sample_count,
        "cpu_avg": agg.cpu_avg.map(round1).unwrap_or(0.0),
        "memory_avg": agg.memory_avg.map(round1).unwrap_or(0.0),
        "disk_avg": agg.disk_avg.map(round1).unwrap_or(0.0),
    });
    let id = uuid::Uuid::new_v4().to_string();
    let ts = sk_core::time::now_sql();
    sqlx::query("INSERT INTO sk_metrics_aggregates(id, summary_json, created_at) VALUES(?, ?, ?)")
        .bind(&id)
        .bind(summary.to_string())
        .bind(&ts)
        .execute(&state.db)
        .await
        .map_err(anyhow::Error::from)?;
    Ok(Json(
        json!({ "success": true, "aggregate_id": id, "created_at": ts, "summary": summary }),
    ))
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}
