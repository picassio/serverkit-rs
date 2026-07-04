use sqlx::sqlite::{SqliteConnectOptions, SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::str::FromStr;

/// Open (and create if missing) the ServerKit SQLite database.
pub async fn connect(database_path: &str) -> anyhow::Result<SqlitePool> {
    if let Some(parent) = Path::new(database_path).parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    let opts = SqliteConnectOptions::from_str(&format!("sqlite://{database_path}"))?
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(std::time::Duration::from_secs(10))
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(opts)
        .await?;
    Ok(pool)
}

/// Apply the schema baseline if this is a fresh database.
///
/// A database provisioned by the Flask backend already has all tables (plus
/// `alembic_version`); running the sqlx migrator there would fail, so we only
/// migrate when the `users` table is absent.
pub async fn ensure_schema(
    pool: &SqlitePool,
    migrator: &sqlx::migrate::Migrator,
) -> anyhow::Result<()> {
    let existing: Option<(String,)> =
        sqlx::query_as("SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'users'")
            .fetch_optional(pool)
            .await?;

    if existing.is_none() {
        tracing::info!("fresh database — applying schema baseline");
        migrator.run(pool).await?;
    } else {
        tracing::info!("existing schema detected — skipping migrations");
    }
    Ok(())
}
