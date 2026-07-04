//! Minimal port of `SettingsService` (key/value rows in `system_settings`).

use sqlx::SqlitePool;

/// Read a setting as a loose boolean (`SettingsService.get(key, False)`).
pub async fn get_bool(pool: &SqlitePool, key: &str, default: bool) -> anyhow::Result<bool> {
    let row: Option<(Option<String>,)> =
        sqlx::query_as("SELECT value FROM system_settings WHERE \"key\" = ?")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(match row {
        Some((Some(v),)) => matches!(v.trim(), "true" | "True" | "1"),
        _ => default,
    })
}

/// Upsert a setting (`SettingsService.set`). Values are stored as JSON-ish
/// text the Python side parses back (`true`/`false` for bools).
pub async fn set(
    pool: &SqlitePool,
    key: &str,
    value: &str,
    value_type: &str,
    user_id: Option<i64>,
) -> anyhow::Result<()> {
    sqlx::query(
        r#"INSERT INTO system_settings ("key", value, value_type, updated_at, updated_by)
           VALUES (?, ?, ?, ?, ?)
           ON CONFLICT("key") DO UPDATE SET
             value = excluded.value,
             value_type = excluded.value_type,
             updated_at = excluded.updated_at,
             updated_by = excluded.updated_by"#,
    )
    .bind(key)
    .bind(value)
    .bind(value_type)
    .bind(sk_core::time::now_sql())
    .bind(user_id)
    .execute(pool)
    .await?;
    Ok(())
}
