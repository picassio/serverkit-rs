//! Alert evaluator — the background half of monitoring. When enabled, it
//! samples metrics on `check_interval`, checks thresholds, and for each
//! newly-breaching type (respecting the per-type cooldown) records history,
//! posts a webhook, and drops an in-app notification.

use serde_json::json;
use sqlx::SqlitePool;
use std::collections::HashMap;

pub fn spawn(db: SqlitePool) {
    tokio::spawn(async move {
        let mut last_fired: HashMap<String, i64> = HashMap::new();
        loop {
            let cfg = sk_monitor::get_config();
            let interval = cfg["check_interval"]
                .as_u64()
                .unwrap_or(sk_monitor::DEFAULT_CHECK_INTERVAL);
            tokio::time::sleep(std::time::Duration::from_secs(interval.max(10))).await;

            if cfg["enabled"].as_bool() != Some(true) {
                continue;
            }
            if let Err(e) = evaluate(&db, &mut last_fired).await {
                tracing::warn!(error = %e, "alert evaluation failed");
            }
        }
    });
}

async fn evaluate(db: &SqlitePool, last_fired: &mut HashMap<String, i64>) -> anyhow::Result<()> {
    let m = sk_system::all_metrics().await;
    let disk = m["disk"]["partitions"]
        .as_array()
        .and_then(|p| p.iter().find(|d| d["mountpoint"] == "/"))
        .and_then(|d| d["percent"].as_f64())
        .unwrap_or(0.0);
    let snap = sk_monitor::Snapshot {
        cpu_percent: m["cpu"]["percent"].as_f64().unwrap_or(0.0),
        memory_percent: m["memory"]["ram"]["percent"].as_f64().unwrap_or(0.0),
        disk_percent: disk,
        load_1min: m["load_average"]["1min"].as_f64().unwrap_or(0.0),
    };

    let alerts = sk_monitor::check_thresholds(&snap, &sk_monitor::thresholds());
    if alerts.is_empty() {
        return Ok(());
    }

    // cooldown filter per alert type
    let now = chrono::Utc::now().timestamp();
    let fresh: Vec<serde_json::Value> = alerts
        .into_iter()
        .filter(|a| {
            let ty = a["type"].as_str().unwrap_or("").to_string();
            let ok = last_fired
                .get(&ty)
                .map(|t| now - t >= sk_monitor::ALERT_COOLDOWN_SECS)
                .unwrap_or(true);
            if ok {
                last_fired.insert(ty, now);
            }
            ok
        })
        .collect();
    if fresh.is_empty() {
        return Ok(());
    }

    sk_monitor::record_alerts(&fresh);

    // in-app notifications (matches the notifications table schema)
    for a in &fresh {
        let _ = sqlx::query(
            "INSERT INTO notifications (event_key, category, severity, title, body, data_json, created_at) \
             VALUES (?, 'monitoring', ?, ?, ?, ?, ?)",
        )
        .bind(format!("alert.{}", a["type"].as_str().unwrap_or("system")))
        .bind(a["severity"].as_str().unwrap_or("warning"))
        .bind(format!("{} alert", a["type"].as_str().unwrap_or("system")))
        .bind(a["message"].as_str().unwrap_or(""))
        .bind(a.to_string())
        .bind(sk_core::time::now_sql())
        .execute(db)
        .await;
    }

    // webhook delivery
    let cfg = sk_monitor::get_config();
    if cfg["webhook"]["enabled"].as_bool() == Some(true) {
        if let Some(url) = cfg["webhook"]["url"].as_str().filter(|s| !s.is_empty()) {
            let host = sk_system::system_info()["hostname"]
                .as_str()
                .unwrap_or("")
                .to_string();
            let r = sk_monitor::deliver_webhook(url, &fresh, &host).await;
            tracing::info!(delivered = %json!(r), count = fresh.len(), "alerts delivered");
        }
    }
    tracing::info!(count = fresh.len(), "alerts fired");
    Ok(())
}
