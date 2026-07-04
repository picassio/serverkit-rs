use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_workflows(id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL, definition_json TEXT NOT NULL DEFAULT '{}', deployed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_workflow_executions(id TEXT PRIMARY KEY, workflow_id TEXT NOT NULL, status TEXT NOT NULL, context_json TEXT NOT NULL DEFAULT '{}', result_json TEXT NOT NULL DEFAULT '{}', started_at TEXT NOT NULL, finished_at TEXT);
CREATE TABLE IF NOT EXISTS sk_workflow_logs(id TEXT PRIMARY KEY, execution_id TEXT NOT NULL, level TEXT NOT NULL, message TEXT NOT NULL, created_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-workflows schema")?;
    Ok(())
}
fn wf_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"definition":j(Some(r.get("definition_json"))),"deployed_at":r.get::<Option<String>,_>("deployed_at"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn ex_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"workflow_id":r.get::<String,_>("workflow_id"),"status":r.get::<String,_>("status"),"context":j(Some(r.get("context_json"))),"result":j(Some(r.get("result_json"))),"started_at":r.get::<String,_>("started_at"),"finished_at":r.get::<Option<String>,_>("finished_at")})
}
pub fn validate_def(b: &Value) -> Value {
    let mut errors = Vec::new();
    if s(b, "name", "").is_empty() {
        errors.push(json!({"field":"name","message":"name is required"}));
    }
    let def = b.get("definition").unwrap_or(b);
    if let Some(steps) = def.get("steps") {
        if !steps.is_array() {
            errors.push(json!({"field":"steps","message":"steps must be an array"}));
        }
    }
    Ok::<(), ()>(()).ok();
    json!({"valid":errors.is_empty(),"errors":errors})
}
pub async fn list(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_workflows ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let vals: Vec<Value> = rows.into_iter().map(wf_row).collect();
    Ok(json!({"workflows":vals,"items":vals,"count":vals.len()}))
}
pub async fn create(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let val = validate_def(b);
    if val["valid"] != true {
        return Ok(json!({"success":false,"validation":val}));
    }
    let wid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_workflows(id,name,status,definition_json,created_at,updated_at) VALUES(?,?,?,?,?,?)").bind(&wid).bind(s(b,"name","Workflow")).bind("draft").bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"workflow":get(pool,&wid).await?["workflow"].clone()}))
}
pub async fn get(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_workflows WHERE id=?")
        .bind(wid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"workflow":wf_row(r)}),
        None => json!({"success":false,"code":"WORKFLOW_NOT_FOUND","error":"Workflow not found"}),
    })
}
pub async fn update(pool: &SqlitePool, wid: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_workflows WHERE id=?")
        .bind(wid)
        .fetch_optional(pool)
        .await?;
    let Some(r) = old else {
        return Ok(
            json!({"success":false,"code":"WORKFLOW_NOT_FOUND","error":"Workflow not found"}),
        );
    };
    let name = s(b, "name", &r.get::<String, _>("name")).to_string();
    sqlx::query(
        "UPDATE sk_workflows SET name=?,definition_json=?,status='draft',updated_at=? WHERE id=?",
    )
    .bind(name)
    .bind(b.to_string())
    .bind(now())
    .bind(wid)
    .execute(pool)
    .await?;
    Ok(json!({"success":true,"workflow":get(pool,wid).await?["workflow"].clone()}))
}
pub async fn delete(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let ex = sqlx::query("SELECT id FROM sk_workflow_executions WHERE workflow_id=?")
        .bind(wid)
        .fetch_all(pool)
        .await?;
    for r in ex {
        sqlx::query("DELETE FROM sk_workflow_logs WHERE execution_id=?")
            .bind(r.get::<String, _>("id"))
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM sk_workflow_executions WHERE workflow_id=?")
        .bind(wid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_workflows WHERE id=?")
        .bind(wid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn deploy(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query(
        "UPDATE sk_workflows SET status='deployed',deployed_at=?,updated_at=? WHERE id=?",
    )
    .bind(now())
    .bind(now())
    .bind(wid)
    .execute(pool)
    .await?
    .rows_affected();
    Ok(json!({"success":n>0,"workflow":get(pool,wid).await?["workflow"].clone()}))
}
pub async fn execute(pool: &SqlitePool, wid: &str, b: &Value) -> anyhow::Result<Value> {
    if get(pool, wid).await?.get("workflow").is_none() {
        return Ok(json!({"success":false,"code":"WORKFLOW_NOT_FOUND"}));
    }
    let eid = id();
    let ts = now();
    let ctx = b.get("context").cloned().unwrap_or_else(|| json!({}));
    let result = json!({"message":"Workflow execution recorded","executor":"sk-workflows","steps_executed":0});
    sqlx::query("INSERT INTO sk_workflow_executions(id,workflow_id,status,context_json,result_json,started_at,finished_at) VALUES(?,?,?,?,?,?,?)").bind(&eid).bind(wid).bind("completed").bind(ctx.to_string()).bind(result.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    log(pool, &eid, "info", "Workflow execution recorded").await?;
    Ok(json!({"success":true,"execution":execution(pool,&eid).await?["execution"].clone()}))
}
async fn log(pool: &SqlitePool, eid: &str, level: &str, message: &str) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO sk_workflow_logs(id,execution_id,level,message,created_at) VALUES(?,?,?,?,?)",
    )
    .bind(id())
    .bind(eid)
    .bind(level)
    .bind(message)
    .bind(now())
    .execute(pool)
    .await?;
    Ok(())
}
pub async fn executions(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_workflow_executions WHERE workflow_id=? ORDER BY started_at DESC",
    )
    .bind(wid)
    .fetch_all(pool)
    .await?;
    let vals: Vec<Value> = rows.into_iter().map(ex_row).collect();
    Ok(json!({"executions":vals,"items":vals,"count":vals.len()}))
}
pub async fn execution(pool: &SqlitePool, eid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_workflow_executions WHERE id=?")
        .bind(eid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"execution":ex_row(r)}),
        None => json!({"success":false,"code":"EXECUTION_NOT_FOUND","error":"Execution not found"}),
    })
}
pub async fn logs(pool: &SqlitePool, eid: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_workflow_logs WHERE execution_id=? ORDER BY created_at ASC")
            .bind(eid)
            .fetch_all(pool)
            .await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"execution_id":r.get::<String,_>("execution_id"),"level":r.get::<String,_>("level"),"message":r.get::<String,_>("message"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"logs":vals,"items":vals,"count":vals.len()}))
}
