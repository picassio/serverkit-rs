//! Let's Encrypt auto-renewal — the certbot-timer equivalent. Once a day,
//! scan `letsencrypt` stores and re-issue any cert within 30 days of expiry.

use sqlx::SqlitePool;

const CHECK_INTERVAL_SECS: u64 = 24 * 3600;
const RENEW_THRESHOLD_DAYS: i64 = 30;

pub fn spawn(pool: SqlitePool) {
    tokio::spawn(async move {
        // small initial delay so startup isn't competing with provisioning
        tokio::time::sleep(std::time::Duration::from_secs(120)).await;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(CHECK_INTERVAL_SECS));
        loop {
            tick.tick().await;
            if let Err(e) = sweep(&pool).await {
                tracing::warn!(error = %e, "LE renewal sweep failed");
            }
        }
    });
}

async fn sweep(pool: &SqlitePool) -> anyhow::Result<()> {
    let stores = sk_magento::store::list(pool).await?;
    for s in stores.into_iter().filter(|s| s.ssl_mode == "letsencrypt") {
        match sk_magento::provision::renew_cert(&s, false, RENEW_THRESHOLD_DAYS).await {
            Ok(v) if v["renewed"].as_bool() == Some(true) => {
                tracing::info!(store = %s.name, "Let's Encrypt cert renewed");
            }
            Ok(_) => {}
            Err(e) => tracing::warn!(store = %s.name, error = %e, "LE renewal failed"),
        }
    }
    Ok(())
}
