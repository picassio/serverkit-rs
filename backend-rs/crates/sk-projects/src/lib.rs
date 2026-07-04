//! Persisted projects/workspaces/API keys/vaults/shared resources.

use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn body_str<'a>(body: &'a Value, key: &str, default: &'a str) -> &'a str {
    body.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn body_opt<'a>(body: &'a Value, key: &str) -> Option<&'a str> {
    body.get(key).and_then(Value::as_str)
}
fn parse_json(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or_else(|| json!([]))
}
fn token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
        CREATE TABLE IF NOT EXISTS sk_workspaces (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
            archived INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_workspace_members (
            id TEXT PRIMARY KEY, workspace_id TEXT NOT NULL, user_id TEXT NOT NULL, role TEXT NOT NULL,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            UNIQUE(workspace_id, user_id), FOREIGN KEY(workspace_id) REFERENCES sk_workspaces(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS sk_projects (
            id TEXT PRIMARY KEY, workspace_id TEXT, name TEXT NOT NULL, description TEXT, color TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_environments (
            id TEXT PRIMARY KEY, project_id TEXT NOT NULL, name TEXT NOT NULL, slug TEXT, sort_order INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY(project_id) REFERENCES sk_projects(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS sk_api_keys (
            id TEXT PRIMARY KEY, workspace_id TEXT, name TEXT NOT NULL, scopes_json TEXT NOT NULL,
            token_encrypted TEXT NOT NULL, revoked INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_used_at TEXT
        );
        CREATE TABLE IF NOT EXISTS sk_vaults (
            id TEXT PRIMARY KEY, name TEXT NOT NULL, description TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_secrets (
            id TEXT PRIMARY KEY, vault_id TEXT NOT NULL, name TEXT NOT NULL, value_encrypted TEXT NOT NULL,
            description TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY(vault_id) REFERENCES sk_vaults(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS sk_resource_tags (
            resource_type TEXT NOT NULL, resource_id TEXT NOT NULL, tag TEXT NOT NULL, created_at TEXT NOT NULL,
            PRIMARY KEY(resource_type, resource_id, tag)
        );
        CREATE TABLE IF NOT EXISTS sk_variable_groups (
            id TEXT PRIMARY KEY, scope_type TEXT NOT NULL, scope_id TEXT NOT NULL, name TEXT NOT NULL, description TEXT,
            created_at TEXT NOT NULL, updated_at TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS sk_group_variables (
            id TEXT PRIMARY KEY, group_id TEXT NOT NULL, key TEXT NOT NULL, value_encrypted TEXT NOT NULL,
            is_secret INTEGER NOT NULL DEFAULT 0, target_service TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL,
            FOREIGN KEY(group_id) REFERENCES sk_variable_groups(id) ON DELETE CASCADE
        );
        CREATE TABLE IF NOT EXISTS sk_variable_group_attachments (
            group_id TEXT NOT NULL, resource_type TEXT NOT NULL, resource_id TEXT NOT NULL, created_at TEXT NOT NULL,
            PRIMARY KEY(group_id, resource_type, resource_id),
            FOREIGN KEY(group_id) REFERENCES sk_variable_groups(id) ON DELETE CASCADE
        );
    "#).execute(pool).await.context("ensure sk-projects schema")?;
    Ok(())
}

fn workspace(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"),
        "description": row.try_get::<Option<String>,_>("description").ok().flatten(),
        "archived": row.get::<i64,_>("archived") != 0,
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn project(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "workspace_id": row.try_get::<Option<String>,_>("workspace_id").ok().flatten(),
        "name": row.get::<String,_>("name"), "description": row.try_get::<Option<String>,_>("description").ok().flatten(),
        "color": row.try_get::<Option<String>,_>("color").ok().flatten(),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn environment(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "project_id": row.get::<String,_>("project_id"),
        "name": row.get::<String,_>("name"), "slug": row.try_get::<Option<String>,_>("slug").ok().flatten(),
        "sort_order": row.get::<i64,_>("sort_order"), "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn api_key(row: &sqlx::sqlite::SqliteRow, include_token: Option<String>) -> Value {
    let mut v = json!({
        "id": row.get::<String,_>("id"), "workspace_id": row.try_get::<Option<String>,_>("workspace_id").ok().flatten(),
        "name": row.get::<String,_>("name"), "scopes": parse_json(row.try_get::<Option<String>,_>("scopes_json").ok().flatten()),
        "revoked": row.get::<i64,_>("revoked") != 0, "created_at": row.get::<String,_>("created_at"),
        "updated_at": row.get::<String,_>("updated_at"), "last_used_at": row.try_get::<Option<String>,_>("last_used_at").ok().flatten()
    });
    if let Some(t) = include_token {
        v["token"] = json!(t);
    }
    v
}
fn vault(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "name": row.get::<String,_>("name"),
        "description": row.try_get::<Option<String>,_>("description").ok().flatten(),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn secret(row: &sqlx::sqlite::SqliteRow, reveal: bool) -> Value {
    let enc = row.get::<String, _>("value_encrypted");
    let mut v = json!({
        "id": row.get::<String,_>("id"), "vault_id": row.get::<String,_>("vault_id"), "name": row.get::<String,_>("name"),
        "description": row.try_get::<Option<String>,_>("description").ok().flatten(), "has_value": true,
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    });
    if reveal {
        v["value"] = json!(sk_core::crypto::decrypt_or_plain(&enc));
    }
    v
}
fn variable_group(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"), "scope_type": row.get::<String,_>("scope_type"), "scope_id": row.get::<String,_>("scope_id"),
        "name": row.get::<String,_>("name"), "description": row.try_get::<Option<String>,_>("description").ok().flatten(),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}
fn variable(row: &sqlx::sqlite::SqliteRow, reveal: bool) -> Value {
    let is_secret = row.get::<i64, _>("is_secret") != 0;
    let enc = row.get::<String, _>("value_encrypted");
    json!({
        "id": row.get::<String,_>("id"), "group_id": row.get::<String,_>("group_id"), "key": row.get::<String,_>("key"),
        "value": if is_secret && !reveal { Value::Null } else { json!(sk_core::crypto::decrypt_or_plain(&enc)) },
        "is_secret": is_secret, "target_service": row.try_get::<Option<String>,_>("target_service").ok().flatten(),
        "created_at": row.get::<String,_>("created_at"), "updated_at": row.get::<String,_>("updated_at")
    })
}

pub async fn list_workspaces(pool: &SqlitePool, include_archived: bool) -> anyhow::Result<Value> {
    let rows = sqlx::query(if include_archived {
        "SELECT * FROM sk_workspaces ORDER BY name"
    } else {
        "SELECT * FROM sk_workspaces WHERE archived = 0 ORDER BY name"
    })
    .fetch_all(pool)
    .await?;
    Ok(json!({"workspaces": rows.iter().map(workspace).collect::<Vec<_>>() }))
}
pub async fn create_workspace(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query(
        "INSERT INTO sk_workspaces (id,name,description,created_at,updated_at) VALUES (?,?,?,?,?)",
    )
    .bind(&id)
    .bind(body_str(body, "name", "Workspace"))
    .bind(body_opt(body, "description"))
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    get_workspace(pool, &id)
        .await?
        .context("created workspace missing")
}
pub async fn get_workspace(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_workspaces WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(workspace))
}
pub async fn update_workspace(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_workspaces SET name=COALESCE(?,name), description=COALESCE(?,description), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(body_opt(body,"description")).bind(now()).bind(id).execute(pool).await?;
    get_workspace(pool, id).await
}
pub async fn archive_workspace(
    pool: &SqlitePool,
    id: &str,
    archived: bool,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_workspaces SET archived=?, updated_at=? WHERE id=?")
        .bind(if archived { 1 } else { 0 })
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    get_workspace(pool, id).await
}
pub async fn delete_workspace(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_workspaces WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

pub async fn workspace_members(pool: &SqlitePool, workspace_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_workspace_members WHERE workspace_id=? ORDER BY created_at")
            .bind(workspace_id)
            .fetch_all(pool)
            .await?;
    Ok(
        json!({"members": rows.iter().map(|r| json!({"id":r.get::<String,_>("id"),"workspace_id":r.get::<String,_>("workspace_id"),"user_id":r.get::<String,_>("user_id"),"role":r.get::<String,_>("role"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn add_workspace_member(
    pool: &SqlitePool,
    workspace_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_workspace_members (id,workspace_id,user_id,role,created_at,updated_at) VALUES (?,?,?,?,?,?)").bind(&id).bind(workspace_id).bind(body_str(body,"user_id","0")).bind(body_str(body,"role","member")).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"id":id}))
}
pub async fn update_member_role(
    pool: &SqlitePool,
    member_id: &str,
    role: &str,
) -> anyhow::Result<bool> {
    Ok(
        sqlx::query("UPDATE sk_workspace_members SET role=?, updated_at=? WHERE id=?")
            .bind(role)
            .bind(now())
            .bind(member_id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}
pub async fn delete_member(pool: &SqlitePool, member_id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_workspace_members WHERE id=?")
        .bind(member_id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

pub async fn list_projects(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_projects ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"projects":rows.iter().map(project).collect::<Vec<_>>() }))
}
pub async fn create_project(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_projects (id,workspace_id,name,description,color,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(&id).bind(body_opt(body,"workspace_id")).bind(body_str(body,"name","Project")).bind(body_opt(body,"description")).bind(body_opt(body,"color")).bind(&ts).bind(&ts).execute(pool).await?;
    get_project(pool, &id)
        .await?
        .context("created project missing")
}
pub async fn get_project(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_projects WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(project))
}
pub async fn update_project(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_projects SET name=COALESCE(?,name), description=COALESCE(?,description), color=COALESCE(?,color), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(body_opt(body,"description")).bind(body_opt(body,"color")).bind(now()).bind(id).execute(pool).await?;
    get_project(pool, id).await
}
pub async fn delete_project(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_projects WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

pub async fn create_environment(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_environments (id,project_id,name,slug,sort_order,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(&id).bind(body_str(body,"project_id","")).bind(body_str(body,"name","Environment")).bind(body_opt(body,"slug")).bind(body.get("sort_order").and_then(Value::as_i64).unwrap_or(0)).bind(&ts).bind(&ts).execute(pool).await?;
    get_environment(pool, &id)
        .await?
        .context("created environment missing")
}
pub async fn get_environment(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_environments WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(environment))
}
pub async fn update_environment(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_environments SET name=COALESCE(?,name), slug=COALESCE(?,slug), sort_order=COALESCE(?,sort_order), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(body_opt(body,"slug")).bind(body.get("sort_order").and_then(Value::as_i64)).bind(now()).bind(id).execute(pool).await?;
    get_environment(pool, id).await
}
pub async fn delete_environment(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_environments WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
pub async fn reorder_environments(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    if let Some(ids) = body.get("ordered_ids").and_then(Value::as_array) {
        for (i, v) in ids.iter().enumerate() {
            if let Some(id) = v.as_str() {
                sqlx::query("UPDATE sk_environments SET sort_order=?, updated_at=? WHERE id=?")
                    .bind(i as i64)
                    .bind(now())
                    .bind(id)
                    .execute(pool)
                    .await?;
            }
        }
    }
    Ok(json!({"success":true}))
}

pub async fn list_api_keys(pool: &SqlitePool, workspace_id: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(w) = workspace_id {
        sqlx::query("SELECT * FROM sk_api_keys WHERE workspace_id=? ORDER BY created_at DESC")
            .bind(w)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_api_keys ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    Ok(json!({"api_keys": rows.iter().map(|r| api_key(r,None)).collect::<Vec<_>>() }))
}
pub fn api_key_scopes() -> Value {
    json!({"scopes":["read","write","admin","apps:read","apps:write","deploy","secrets:read","secrets:write"]})
}
pub async fn create_api_key(
    pool: &SqlitePool,
    body: &Value,
    workspace_id: Option<&str>,
) -> anyhow::Result<Value> {
    let id = id();
    let raw = format!("sk_{}", token());
    let enc = sk_core::crypto::encrypt(&raw);
    let ts = now();
    let scopes = body
        .get("scopes")
        .cloned()
        .unwrap_or_else(|| json!(["read"]));
    sqlx::query("INSERT INTO sk_api_keys (id,workspace_id,name,scopes_json,token_encrypted,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(&id).bind(workspace_id.or_else(|| body_opt(body,"workspace_id"))).bind(body_str(body,"name","API Key")).bind(scopes.to_string()).bind(enc).bind(&ts).bind(&ts).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_api_keys WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    Ok(api_key(&row, Some(raw)))
}
pub async fn get_api_key(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_api_keys WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(|r| api_key(r, None)))
}
pub async fn update_api_key(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let scopes = body.get("scopes").map(Value::to_string);
    sqlx::query("UPDATE sk_api_keys SET name=COALESCE(?,name), scopes_json=COALESCE(?,scopes_json), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(scopes).bind(now()).bind(id).execute(pool).await?;
    get_api_key(pool, id).await
}
pub async fn revoke_api_key(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(
        sqlx::query("UPDATE sk_api_keys SET revoked=1, updated_at=? WHERE id=?")
            .bind(now())
            .bind(id)
            .execute(pool)
            .await?
            .rows_affected()
            > 0,
    )
}
pub async fn rotate_api_key(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let raw = format!("sk_{}", token());
    let enc = sk_core::crypto::encrypt(&raw);
    sqlx::query("UPDATE sk_api_keys SET token_encrypted=?, updated_at=? WHERE id=?")
        .bind(enc)
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?;
    let row = sqlx::query("SELECT * FROM sk_api_keys WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(|r| api_key(r, Some(raw))))
}

pub async fn list_vaults(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_vaults ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"vaults":rows.iter().map(vault).collect::<Vec<_>>() }))
}
pub async fn create_vault(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query(
        "INSERT INTO sk_vaults (id,name,description,created_at,updated_at) VALUES (?,?,?,?,?)",
    )
    .bind(&id)
    .bind(body_str(body, "name", "Vault"))
    .bind(body_opt(body, "description"))
    .bind(&ts)
    .bind(&ts)
    .execute(pool)
    .await?;
    get_vault(pool, &id).await?.context("created vault missing")
}
pub async fn get_vault(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_vaults WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(vault))
}
pub async fn update_vault(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_vaults SET name=COALESCE(?,name), description=COALESCE(?,description), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(body_opt(body,"description")).bind(now()).bind(id).execute(pool).await?;
    get_vault(pool, id).await
}
pub async fn delete_vault(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_vaults WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
pub async fn list_secrets(pool: &SqlitePool, vault_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_secrets WHERE vault_id=? ORDER BY name")
        .bind(vault_id)
        .fetch_all(pool)
        .await?;
    Ok(json!({"secrets":rows.iter().map(|r|secret(r,false)).collect::<Vec<_>>() }))
}
pub async fn create_secret(
    pool: &SqlitePool,
    vault_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let enc = sk_core::crypto::encrypt(body_str(body, "value", ""));
    sqlx::query("INSERT INTO sk_secrets (id,vault_id,name,value_encrypted,description,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(&id).bind(vault_id).bind(body_str(body,"name","SECRET")).bind(enc).bind(body_opt(body,"description")).bind(&ts).bind(&ts).execute(pool).await?;
    get_secret(pool, &id, false)
        .await?
        .context("created secret missing")
}
pub async fn bulk_create_secrets(
    pool: &SqlitePool,
    vault_id: &str,
    secrets: &[Value],
) -> anyhow::Result<Value> {
    let mut created = Vec::new();
    for s in secrets {
        created.push(create_secret(pool, vault_id, s).await?);
    }
    Ok(json!({"secrets":created}))
}
pub async fn get_secret(
    pool: &SqlitePool,
    id: &str,
    reveal: bool,
) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_secrets WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(|r| secret(r, reveal)))
}
pub async fn update_secret(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let enc = body_opt(body, "value").map(sk_core::crypto::encrypt);
    sqlx::query("UPDATE sk_secrets SET name=COALESCE(?,name), value_encrypted=COALESCE(?,value_encrypted), description=COALESCE(?,description), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(enc).bind(body_opt(body,"description")).bind(now()).bind(id).execute(pool).await?;
    get_secret(pool, id, false).await
}
pub async fn delete_secret(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_secrets WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}

pub fn shared_resource_types() -> Value {
    json!({"resource_types":["workspace","project","environment","app","database","server"]})
}
pub async fn list_tags(
    pool: &SqlitePool,
    resource_type: Option<&str>,
    resource_id: Option<&str>,
    tag: Option<&str>,
) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_resource_tags ORDER BY tag")
        .fetch_all(pool)
        .await?;
    let mut tags:Vec<Value>=rows.iter().map(|r|json!({"resource_type":r.get::<String,_>("resource_type"),"resource_id":r.get::<String,_>("resource_id"),"tag":r.get::<String,_>("tag"),"created_at":r.get::<String,_>("created_at")})).collect();
    if let Some(x) = resource_type {
        tags.retain(|t| t["resource_type"] == json!(x));
    }
    if let Some(x) = resource_id {
        tags.retain(|t| t["resource_id"] == json!(x));
    }
    if let Some(x) = tag {
        tags.retain(|t| t["tag"] == json!(x));
    }
    Ok(json!({"tags":tags}))
}
pub async fn add_tag(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT OR IGNORE INTO sk_resource_tags (resource_type,resource_id,tag,created_at) VALUES (?,?,?,?)").bind(body_str(body,"resource_type","")).bind(body_str(body,"resource_id","")).bind(body_str(body,"tag","")).bind(now()).execute(pool).await?;
    Ok(json!({"success":true}))
}
pub async fn remove_tag(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_resource_tags WHERE resource_type=? AND resource_id=? AND tag=?")
        .bind(body_str(body, "resource_type", ""))
        .bind(body_str(body, "resource_id", ""))
        .bind(body_str(body, "tag", ""))
        .execute(pool)
        .await?;
    Ok(json!({"success":true}))
}
pub async fn list_variable_groups(
    pool: &SqlitePool,
    scope_type: Option<&str>,
    scope_id: Option<&str>,
) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_variable_groups ORDER BY name")
        .fetch_all(pool)
        .await?;
    let mut groups: Vec<Value> = rows.iter().map(variable_group).collect();
    if let Some(x) = scope_type {
        groups.retain(|g| g["scope_type"] == json!(x));
    }
    if let Some(x) = scope_id {
        groups.retain(|g| g["scope_id"] == json!(x));
    }
    Ok(json!({"groups":groups}))
}
pub async fn create_variable_group(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_variable_groups (id,scope_type,scope_id,name,description,created_at,updated_at) VALUES (?,?,?,?,?,?,?)").bind(&id).bind(body_str(body,"scope_type","workspace")).bind(body_str(body,"scope_id","default")).bind(body_str(body,"name","Variables")).bind(body_opt(body,"description")).bind(&ts).bind(&ts).execute(pool).await?;
    get_variable_group(pool, &id)
        .await?
        .context("created variable group missing")
}
pub async fn get_variable_group(pool: &SqlitePool, id: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_variable_groups WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(variable_group))
}
pub async fn update_variable_group(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    sqlx::query("UPDATE sk_variable_groups SET name=COALESCE(?,name), description=COALESCE(?,description), updated_at=? WHERE id=?").bind(body_opt(body,"name")).bind(body_opt(body,"description")).bind(now()).bind(id).execute(pool).await?;
    get_variable_group(pool, id).await
}
pub async fn delete_variable_group(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_variable_groups WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
pub async fn group_variables(pool: &SqlitePool, group_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_group_variables WHERE group_id=? ORDER BY key")
        .bind(group_id)
        .fetch_all(pool)
        .await?;
    Ok(json!({"variables":rows.iter().map(|r|variable(r,false)).collect::<Vec<_>>() }))
}
pub async fn add_variable(
    pool: &SqlitePool,
    group_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let enc = sk_core::crypto::encrypt(body_str(body, "value", ""));
    sqlx::query("INSERT INTO sk_group_variables (id,group_id,key,value_encrypted,is_secret,target_service,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)").bind(&id).bind(group_id).bind(body_str(body,"key","KEY")).bind(enc).bind(if body.get("is_secret").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(body_opt(body,"target_service")).bind(&ts).bind(&ts).execute(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_group_variables WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    Ok(variable(&r, false))
}
pub async fn update_variable(
    pool: &SqlitePool,
    id: &str,
    body: &Value,
) -> anyhow::Result<Option<Value>> {
    let enc = body_opt(body, "value").map(sk_core::crypto::encrypt);
    sqlx::query("UPDATE sk_group_variables SET value_encrypted=COALESCE(?,value_encrypted), is_secret=COALESCE(?,is_secret), target_service=COALESCE(?,target_service), updated_at=? WHERE id=?").bind(enc).bind(body.get("is_secret").and_then(Value::as_bool).map(|b|if b{1}else{0})).bind(body_opt(body,"target_service")).bind(now()).bind(id).execute(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_group_variables WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(|r| variable(r, false)))
}
pub async fn delete_variable(pool: &SqlitePool, id: &str) -> anyhow::Result<bool> {
    Ok(sqlx::query("DELETE FROM sk_group_variables WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected()
        > 0)
}
pub async fn attach_variable_group(
    pool: &SqlitePool,
    group_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    sqlx::query("INSERT OR IGNORE INTO sk_variable_group_attachments (group_id,resource_type,resource_id,created_at) VALUES (?,?,?,?)").bind(group_id).bind(body_str(body,"resource_type","")).bind(body_str(body,"resource_id","")).bind(now()).execute(pool).await?;
    Ok(json!({"success":true}))
}
pub async fn detach_variable_group(
    pool: &SqlitePool,
    group_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_variable_group_attachments WHERE group_id=? AND resource_type=? AND resource_id=?").bind(group_id).bind(body_str(body,"resource_type","")).bind(body_str(body,"resource_id","")).execute(pool).await?;
    Ok(json!({"success":true}))
}
pub async fn resolved_variables(
    pool: &SqlitePool,
    resource_type: &str,
    resource_id: &str,
) -> anyhow::Result<Value> {
    let rows=sqlx::query("SELECT v.* FROM sk_group_variables v JOIN sk_variable_group_attachments a ON a.group_id=v.group_id WHERE a.resource_type=? AND a.resource_id=? ORDER BY v.key").bind(resource_type).bind(resource_id).fetch_all(pool).await?;
    let vars: Vec<Value> = rows.iter().map(|r| variable(r, true)).collect();
    Ok(json!({"variables":vars}))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn secret_roundtrip() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let v = create_vault(&pool, &json!({"name":"v"})).await.unwrap();
        let s = create_secret(
            &pool,
            v["id"].as_str().unwrap(),
            &json!({"name":"TOKEN","value":"abc"}),
        )
        .await
        .unwrap();
        assert!(s.get("value").is_none());
        let r = get_secret(&pool, s["id"].as_str().unwrap(), true)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(r["value"], "abc");
    }
}
