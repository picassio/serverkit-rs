//! Scheduled store DB backups. Every 30 min, back up any store whose
//! `backup_schedule` interval has elapsed since its newest backup, then
//! prune to `backup_retention`.

use sqlx::SqlitePool;

const TICK_SECS: u64 = 30 * 60;

fn interval_secs(schedule: &str) -> Option<i64> {
    match schedule {
        "hourly" => Some(3600),
        "daily" => Some(24 * 3600),
        "weekly" => Some(7 * 24 * 3600),
        _ => None,
    }
}

pub fn spawn(pool: SqlitePool) {
    tokio::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(90)).await;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(TICK_SECS));
        loop {
            tick.tick().await;
            if let Err(e) = sweep(&pool).await {
                tracing::warn!(error = %e, "scheduled backup sweep failed");
            }
        }
    });
}

async fn sweep(pool: &SqlitePool) -> anyhow::Result<()> {
    for s in sk_magento::store::list(pool).await? {
        if s.status != "running" {
            continue;
        }
        let Some(interval) = interval_secs(&s.backup_schedule) else {
            continue;
        };
        let due = sk_magento::backup::newest_backup_age_secs(&s)
            .map(|age| age >= interval)
            .unwrap_or(true); // no backup yet -> due
        if !due {
            continue;
        }
        let result = sk_magento::backup::backup_db(&s).await;
        if result["success"].as_bool().unwrap_or(false) {
            let pruned = sk_magento::backup::prune(&s, s.backup_retention.max(1) as usize);
            tracing::info!(store = %s.name, pruned, "scheduled DB backup complete");
        } else {
            tracing::warn!(store = %s.name, error = %result["error"], "scheduled DB backup failed");
        }
    }
    Ok(())
}
