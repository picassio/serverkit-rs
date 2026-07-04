//! One-shot admin bootstrap for automated installs.
//!
//! If no users exist yet and `SK_BOOTSTRAP_ADMIN_EMAIL` +
//! `SK_BOOTSTRAP_ADMIN_PASSWORD` are set, create an admin user and mark setup
//! complete. Idempotent: a no-op once any user exists, so it is safe to run on
//! every boot. This lets `install.sh` provision a login non-interactively.

use sqlx::SqlitePool;

pub async fn run(db: &SqlitePool) -> anyhow::Result<()> {
    let (Ok(email), Ok(password)) = (
        std::env::var("SK_BOOTSTRAP_ADMIN_EMAIL"),
        std::env::var("SK_BOOTSTRAP_ADMIN_PASSWORD"),
    ) else {
        return Ok(());
    };
    if email.is_empty() || password.is_empty() {
        return Ok(());
    }
    if sk_models::user::count(db).await? > 0 {
        return Ok(()); // already provisioned
    }
    if password.len() < 8 {
        tracing::warn!("SK_BOOTSTRAP_ADMIN_PASSWORD is shorter than 8 chars — skipping bootstrap");
        return Ok(());
    }
    let username = std::env::var("SK_BOOTSTRAP_ADMIN_USERNAME").unwrap_or_else(|_| "admin".into());
    let hash = sk_auth::password::hash_password(&password);
    let user_id =
        sk_models::user::insert(db, &email, &username, &hash, sk_models::user::ROLE_ADMIN).await?;
    sk_models::settings::set(db, "setup_completed", "true", "bool", Some(user_id)).await?;
    tracing::info!(%email, user_id, "bootstrapped admin user from environment");
    Ok(())
}
