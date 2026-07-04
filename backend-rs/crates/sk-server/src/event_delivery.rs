//! Background delivery worker for sk-events queued webhook/subscription tasks.

use sqlx::SqlitePool;
use std::time::Duration;

pub fn spawn(pool: SqlitePool) {
    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            if let Err(e) = deliver_once(&pool, &client).await {
                tracing::warn!(error = %e, "event delivery worker tick failed");
            }
        }
    });
}

async fn deliver_once(pool: &SqlitePool, client: &reqwest::Client) -> anyhow::Result<()> {
    let tasks = sk_events::queued_delivery_tasks(pool, 25).await?;
    for task in tasks {
        let mut req = client
            .post(&task.url)
            .header("content-type", "application/json")
            .header("x-serverkit-delivery", &task.id)
            .header("x-serverkit-target-kind", &task.target_kind)
            .json(&task.request);
        if let Some(secret) = &task.secret {
            req = req.header("x-serverkit-secret", secret);
        }
        let response = match req.send().await {
            Ok(resp) => {
                let status = resp.status().as_u16();
                let text = resp.text().await.unwrap_or_default();
                let ok = (200..300).contains(&status);
                (ok, serde_json::json!({ "status": status, "body": text }))
            }
            Err(e) => (false, serde_json::json!({ "error": e.to_string() })),
        };
        sk_events::mark_delivery_result(pool, &task.id, response.0, response.1).await?;
    }
    Ok(())
}
