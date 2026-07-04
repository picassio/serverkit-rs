//! Port of `app/api/monitoring.py` — status, metrics, alert check/history,
//! config + thresholds, start/stop, test webhook.

use crate::error::{ApiError, ApiResult};
use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::Query;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        .route("/status", get(status))
        .route("/metrics", get(metrics))
        .route("/alerts/check", get(alerts_check))
        .route("/alerts/history", get(alerts_history).delete(clear_history))
        .route("/config", get(get_config).put(put_config))
        .route("/thresholds", get(get_thresholds).put(put_thresholds))
        .route("/start", post(start))
        .route("/stop", post(stop))
        .route("/test/webhook", post(test_webhook))
        .route("/test/email", post(test_email))
}

fn require_admin(u: &sk_models::user::User) -> ApiResult<()> {
    if !u.is_admin() {
        return Err(ApiError::forbidden("Admin access required"));
    }
    Ok(())
}

/// Extract the (cpu%, mem%, root-disk%, load1) snapshot from sk-system.
async fn snapshot() -> sk_monitor::Snapshot {
    let m = sk_system::all_metrics().await;
    let disk = m["disk"]["partitions"]
        .as_array()
        .and_then(|p| p.iter().find(|d| d["mountpoint"] == "/"))
        .and_then(|d| d["percent"].as_f64())
        .unwrap_or(0.0);
    sk_monitor::Snapshot {
        cpu_percent: m["cpu"]["percent"].as_f64().unwrap_or(0.0),
        memory_percent: m["memory"]["ram"]["percent"].as_f64().unwrap_or(0.0),
        disk_percent: disk,
        load_1min: m["load_average"]["1min"].as_f64().unwrap_or(0.0),
    }
}

fn mask_config(mut c: Value) -> Value {
    if let Some(p) = c["email"]["smtp_password"].as_str() {
        c["email"]["smtp_password"] = json!(if p.is_empty() { "" } else { "***" });
    }
    c
}

async fn status(AuthUser(_u): AuthUser) -> Json<Value> {
    let config = sk_monitor::get_config();
    let snap = snapshot().await;
    let t = sk_monitor::thresholds();
    Json(json!({
        "enabled": config["enabled"],
        "check_interval": config["check_interval"],
        "thresholds": config["thresholds"],
        "alerts": sk_monitor::check_thresholds(&snap, &t),
        "metrics": {
            "cpu": snap.cpu_percent, "memory": snap.memory_percent,
            "disk": snap.disk_percent, "load": snap.load_1min,
        },
    }))
}

async fn metrics(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(sk_system::all_metrics().await)
}

async fn alerts_check(AuthUser(_u): AuthUser) -> Json<Value> {
    let snap = snapshot().await;
    Json(json!({ "alerts": sk_monitor::check_thresholds(&snap, &sk_monitor::thresholds()) }))
}

#[derive(Deserialize)]
struct HistoryQuery {
    limit: Option<usize>,
}

async fn alerts_history(AuthUser(_u): AuthUser, Query(q): Query<HistoryQuery>) -> Json<Value> {
    Json(json!({ "alerts": sk_monitor::history(q.limit.unwrap_or(100)) }))
}

async fn clear_history(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(sk_monitor::clear_history()))
}

async fn get_config(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    Ok(Json(mask_config(sk_monitor::get_config())))
}

/// PUT /config — merge like Flask (preserve masked smtp password).
async fn put_config(AuthUser(u): AuthUser, Json(data): Json<Value>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut cfg = sk_monitor::get_config();
    if let Some(v) = data.get("enabled") {
        cfg["enabled"] = json!(v.as_bool().unwrap_or(false));
    }
    if let Some(v) = data.get("check_interval") {
        cfg["check_interval"] = v.clone();
    }
    if let Some(t) = data.get("thresholds").and_then(|v| v.as_object()) {
        for (k, v) in t {
            cfg["thresholds"][k] = v.clone();
        }
    }
    if let Some(w) = data.get("webhook").and_then(|v| v.as_object()) {
        for (k, v) in w {
            cfg["webhook"][k] = v.clone();
        }
    }
    if let Some(e) = data.get("email").and_then(|v| v.as_object()) {
        let prev_pw = cfg["email"]["smtp_password"]
            .as_str()
            .unwrap_or("")
            .to_string();
        for (k, v) in e {
            cfg["email"][k] = v.clone();
        }
        // don't overwrite the stored password with a masked/blank value
        if matches!(
            cfg["email"]["smtp_password"].as_str(),
            Some("") | Some("***") | None
        ) {
            cfg["email"]["smtp_password"] = json!(prev_pw);
        }
    }
    Ok(Json(sk_monitor::save_config(&cfg)))
}

async fn get_thresholds(AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "thresholds": sk_monitor::get_config()["thresholds"] }))
}

async fn put_thresholds(AuthUser(u): AuthUser, Json(data): Json<Value>) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut cfg = sk_monitor::get_config();
    if let Some(t) = data.as_object() {
        for (k, v) in t {
            cfg["thresholds"][k] = v.clone();
        }
    }
    Ok(Json(sk_monitor::save_config(&cfg)))
}

async fn start(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut cfg = sk_monitor::get_config();
    cfg["enabled"] = json!(true);
    sk_monitor::save_config(&cfg);
    Ok(Json(json!({ "success": true, "enabled": true })))
}

async fn stop(AuthUser(u): AuthUser) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let mut cfg = sk_monitor::get_config();
    cfg["enabled"] = json!(false);
    sk_monitor::save_config(&cfg);
    Ok(Json(json!({ "success": true, "enabled": false })))
}

#[derive(Deserialize)]
struct EmailTest {
    email: Option<String>,
}

async fn test_email(
    AuthUser(u): AuthUser,
    body: Option<Json<EmailTest>>,
) -> ApiResult<(axum::http::StatusCode, Json<Value>)> {
    require_admin(&u)?;
    let cfg = sk_monitor::get_config();
    let to = body.and_then(|b| b.0.email).unwrap_or_default();
    let host = cfg["email"]["smtp_host"].as_str().unwrap_or("").to_string();
    let port = cfg["email"]["smtp_port"].as_u64().unwrap_or(587) as u16;
    if to.is_empty() {
        return Err(ApiError::bad_request("email is required"));
    }
    if !cfg["email"]["enabled"].as_bool().unwrap_or(false) || host.is_empty() {
        return Ok((
            axum::http::StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "code": "EMAIL_ALERTS_NOT_CONFIGURED" })),
        ));
    }
    let result = tokio::task::spawn_blocking(move || {
        use std::net::{TcpStream, ToSocketAddrs};
        use std::time::Duration;
        let addr = (host, port).to_socket_addrs().ok().and_then(|mut a| a.next());
        match addr {
            Some(addr) => match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
                Ok(_) => json!({ "success": true, "message": "SMTP TCP connection succeeded", "to": to }),
                Err(e) => json!({ "success": false, "code": "SMTP_CONNECT_FAILED", "error": e.to_string() }),
            },
            None => json!({ "success": false, "code": "SMTP_RESOLVE_FAILED" }),
        }
    })
    .await
    .unwrap_or_else(|e| json!({ "success": false, "error": e.to_string() }));
    let status = if result["success"].as_bool().unwrap_or(false) {
        axum::http::StatusCode::OK
    } else {
        axum::http::StatusCode::BAD_REQUEST
    };
    Ok((status, Json(result)))
}

#[derive(Deserialize)]
struct WebhookTest {
    url: Option<String>,
}

async fn test_webhook(
    AuthUser(u): AuthUser,
    body: Option<Json<WebhookTest>>,
) -> ApiResult<Json<Value>> {
    require_admin(&u)?;
    let cfg = sk_monitor::get_config();
    let url = body
        .and_then(|b| b.0.url)
        .or_else(|| cfg["webhook"]["url"].as_str().map(str::to_string))
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ApiError::bad_request("no webhook url configured or provided"))?;
    let test = json!({
        "type": "test", "severity": "info",
        "message": "ServerKit monitoring test alert", "value": 0, "threshold": 0,
    });
    let host = std::fs::read_to_string("/etc/hostname")
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(Json(
        sk_monitor::deliver_webhook(&url, &[test], &host).await,
    ))
}
