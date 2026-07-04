//! Metrics history sampler — the Rust equivalent of Flask's
//! `MetricsHistoryService` background collector. Samples cpu/memory/disk
//! every 60s into `metrics_history` (level='minute'); `/metrics/history`
//! already reads that table, so this fills the dashboard chart.
//!
//! Retention matches upstream intent: minute rows kept 25h.
//! TODO(P4): hourly/daily aggregation for the 7d/30d ranges.

use sqlx::SqlitePool;

pub fn spawn(pool: SqlitePool) {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(60));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            if let Err(e) = sample(&pool).await {
                tracing::warn!(error = %e, "metrics sample failed");
            }
        }
    });
}

async fn sample(pool: &SqlitePool) -> anyhow::Result<()> {
    let cpu = sk_system::cpu_metrics().await;
    let mem = sk_system::memory_metrics().await;
    let disk = sk_system::disk_metrics();

    let cpu_percent = cpu["percent"].as_f64().unwrap_or(0.0);
    let mem_percent = mem["ram"]["percent"].as_f64().unwrap_or(0.0);
    let mem_used = mem["ram"]["used"].as_i64().unwrap_or(0);
    let mem_total = mem["ram"]["total"].as_i64().unwrap_or(0);

    // root filesystem, like the Flask collector
    let (disk_percent, disk_used, disk_total) = disk["partitions"]
        .as_array()
        .and_then(|parts| parts.iter().find(|p| p["mountpoint"] == "/"))
        .map(|p| {
            (
                p["percent"].as_f64().unwrap_or(0.0),
                p["used"].as_i64().unwrap_or(0),
                p["total"].as_i64().unwrap_or(0),
            )
        })
        .unwrap_or((0.0, 0, 0));

    let now = sk_core::time::now_sql();
    sqlx::query(
        r#"INSERT INTO metrics_history
           (timestamp, level, cpu_percent, memory_percent, memory_used_bytes,
            memory_total_bytes, disk_percent, disk_used_bytes, disk_total_bytes)
           VALUES (?, 'minute', ?, ?, ?, ?, ?, ?, ?)"#,
    )
    .bind(&now)
    .bind(cpu_percent)
    .bind(mem_percent)
    .bind(mem_used)
    .bind(mem_total)
    .bind(disk_percent)
    .bind(disk_used)
    .bind(disk_total)
    .execute(pool)
    .await?;

    // prune minute rows older than 25h
    let cutoff = (sk_core::time::now_naive() - chrono::Duration::hours(25))
        .format("%Y-%m-%d %H:%M:%S%.6f")
        .to_string();
    sqlx::query("DELETE FROM metrics_history WHERE level = 'minute' AND timestamp < ?")
        .bind(cutoff)
        .execute(pool)
        .await?;
    Ok(())
}
