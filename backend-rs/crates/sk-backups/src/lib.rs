use anyhow::Context;
use chrono::{Duration, Utc};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use uuid::Uuid;

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn backup_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SK_BACKUP_DIR").unwrap_or_else(|_| "/var/backups/serverkit".into()),
    )
}
fn remote_dir() -> Option<PathBuf> {
    std::env::var("SK_BACKUP_REMOTE_DIR")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
fn size(path: &Path) -> u64 {
    std::fs::metadata(path).map(|m| m.len()).unwrap_or(0)
}
fn clean_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_backups(id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, path TEXT NOT NULL, size INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL, target_json TEXT NOT NULL DEFAULT '{}', remote_key TEXT, verified_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_backup_config(id INTEGER PRIMARY KEY CHECK(id=1), config_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_backup_storage(id INTEGER PRIMARY KEY CHECK(id=1), config_encrypted TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_backup_rates(id INTEGER PRIMARY KEY CHECK(id=1), rates_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_backup_schedules(id TEXT PRIMARY KEY, name TEXT NOT NULL, backup_type TEXT NOT NULL, target_json TEXT NOT NULL, schedule_time TEXT NOT NULL, days_json TEXT, upload_remote INTEGER NOT NULL DEFAULT 0, enabled INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-backups schema")?;
    Ok(())
}

fn backup_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"kind":r.get::<String,_>("kind"),"type":r.get::<String,_>("kind"),"name":r.get::<String,_>("name"),"path":r.get::<String,_>("path"),"size":r.get::<i64,_>("size"),"status":r.get::<String,_>("status"),"target":j(r.try_get::<Option<String>,_>("target_json").ok().flatten()),"remote_key":r.try_get::<Option<String>,_>("remote_key").ok().flatten(),"verified_at":r.try_get::<Option<String>,_>("verified_at").ok().flatten(),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn sched_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"backup_type":r.get::<String,_>("backup_type"),"target":j(Some(r.get::<String,_>("target_json"))),"schedule_time":r.get::<String,_>("schedule_time"),"days":j(r.try_get::<Option<String>,_>("days_json").ok().flatten()),"upload_remote":r.get::<i64,_>("upload_remote")!=0,"enabled":r.get::<i64,_>("enabled")!=0,"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}

async fn insert_record(
    pool: &SqlitePool,
    kind: &str,
    name: &str,
    path: &Path,
    target: &Value,
) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let p = path.to_string_lossy().to_string();
    sqlx::query("INSERT INTO sk_backups(id,kind,name,path,size,status,target_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)")
        .bind(&id).bind(kind).bind(name).bind(&p).bind(size(path) as i64).bind("completed").bind(target.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    get_backup(pool, &id)
        .await?
        .context("created backup missing")
}

pub async fn list(pool: &SqlitePool, kind: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(k) = kind {
        sqlx::query("SELECT * FROM sk_backups WHERE kind=? ORDER BY created_at DESC")
            .bind(k)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_backups ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    Ok(json!({"backups":rows.iter().map(backup_value).collect::<Vec<_>>() }))
}
pub async fn get_backup(pool: &SqlitePool, id_or_path: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_backups WHERE id=? OR path=?")
        .bind(id_or_path)
        .bind(id_or_path)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(backup_value))
}
pub async fn stats(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT kind, COUNT(*) n, COALESCE(SUM(size),0) size FROM sk_backups GROUP BY kind",
    )
    .fetch_all(pool)
    .await?;
    let mut total = 0i64;
    let mut count = 0i64;
    let mut by_kind = Vec::new();
    for r in rows {
        let n: r#i64 = r.get("n");
        let sz: i64 = r.get("size");
        count += n;
        total += sz;
        by_kind.push(json!({"kind":r.get::<String,_>("kind"),"count":n,"size":sz}));
    }
    Ok(json!({"total_backups":count,"total_size":total,"by_kind":by_kind}))
}

pub async fn config(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT config_json FROM sk_backup_config WHERE id=1")
        .fetch_optional(pool)
        .await?;
    if let Some(row) = r {
        Ok(j(Some(row.get::<String, _>("config_json"))))
    } else {
        Ok(json!({"enabled":true,"retention_days":7,"targets":["local"],"backup_dir":backup_dir()}))
    }
}
pub async fn set_config(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let mut cfg = b.clone();
    if cfg.get("backup_dir").is_none() {
        cfg["backup_dir"] = json!(backup_dir());
    }
    sqlx::query("INSERT INTO sk_backup_config(id,config_json,updated_at) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET config_json=excluded.config_json, updated_at=excluded.updated_at").bind(cfg.to_string()).bind(now()).execute(pool).await?;
    config(pool).await
}
pub async fn rates(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT rates_json FROM sk_backup_rates WHERE id=1")
        .fetch_optional(pool)
        .await?;
    if let Some(row) = r {
        Ok(j(Some(row.get::<String, _>("rates_json"))))
    } else {
        Ok(json!({"rates":{"local":0.0,"s3":0.023,"b2":0.006}}))
    }
}
pub async fn set_rates(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let payload = b.get("rates").cloned().unwrap_or_else(|| b.clone());
    sqlx::query("INSERT INTO sk_backup_rates(id,rates_json,updated_at) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET rates_json=excluded.rates_json, updated_at=excluded.updated_at").bind(json!({"rates":payload}).to_string()).bind(now()).execute(pool).await?;
    rates(pool).await
}
pub async fn cost_summary(pool: &SqlitePool) -> anyhow::Result<Value> {
    let stats = stats(pool).await?;
    Ok(json!({"total_usd":0.0,"local_size":stats["total_size"],"items":stats["by_kind"]}))
}

async fn persisted_storage_config(pool: &SqlitePool) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT config_encrypted FROM sk_backup_storage WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(r.and_then(|r| {
        let raw: String = r.get("config_encrypted");
        sk_core::crypto::decrypt(&raw).and_then(|s| serde_json::from_str::<Value>(&s).ok())
    }))
}

async fn configured_remote_dir(pool: &SqlitePool) -> anyhow::Result<Option<PathBuf>> {
    if let Some(dir) = remote_dir() {
        return Ok(Some(dir));
    }
    Ok(persisted_storage_config(pool)
        .await?
        .as_ref()
        .and_then(|cfg| opt(cfg, "remote_dir"))
        .filter(|s| !s.is_empty())
        .map(PathBuf::from))
}

pub async fn storage(pool: &SqlitePool) -> anyhow::Result<Value> {
    let cfg = persisted_storage_config(pool).await?;
    let dir = configured_remote_dir(pool).await?;
    Ok(json!({
        "configured": dir.is_some(),
        "provider": dir.as_ref().map(|_| "local"),
        "remote_dir": dir,
        "config": cfg.unwrap_or(Value::Null),
    }))
}
pub async fn set_storage(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_backup_storage(id,config_encrypted,updated_at) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET config_encrypted=excluded.config_encrypted, updated_at=excluded.updated_at").bind(sk_core::crypto::encrypt(&b.to_string())).bind(now()).execute(pool).await?;
    storage(pool).await
}
pub async fn test_storage(pool: &SqlitePool, b: Option<&Value>) -> anyhow::Result<Value> {
    let dir = if let Some(dir) = b.and_then(|v| opt(v, "remote_dir")).map(PathBuf::from) {
        Some(dir)
    } else {
        configured_remote_dir(pool).await?
    };
    let Some(dir) = dir else {
        return Ok(json!({"success":false,"error":"Remote storage is not configured"}));
    };
    std::fs::create_dir_all(&dir)?;
    let test = dir.join(".serverkit-test");
    std::fs::write(&test, b"ok")?;
    let _ = std::fs::remove_file(test);
    Ok(json!({"success":true,"provider":"local","remote_dir":dir}))
}

fn tar_create(output: &Path, inputs: &[PathBuf]) -> Value {
    if let Some(p) = output.parent() {
        if let Err(e) = std::fs::create_dir_all(p) {
            return json!({"success":false,"error":e.to_string()});
        }
    }
    let mut cmd = std::process::Command::new("tar");
    cmd.arg("-czf").arg(output);
    for p in inputs {
        cmd.arg(p);
    }
    match cmd.output() {
        Ok(o) if o.status.success() => json!({"success":true,"path":output,"size":size(output)}),
        Ok(o) => json!({"success":false,"error":String::from_utf8_lossy(&o.stderr).trim()}),
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}
fn tar_extract(archive: &Path, dest: &Path) -> Value {
    if let Err(e) = std::fs::create_dir_all(dest) {
        return json!({"success":false,"error":e.to_string()});
    }
    match std::process::Command::new("tar")
        .arg("-xzf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .output()
    {
        Ok(o) if o.status.success() => json!({"success":true,"restore_path":dest}),
        Ok(o) => json!({"success":false,"error":String::from_utf8_lossy(&o.stderr).trim()}),
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}

pub async fn backup_application(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let app_id = s(b, "application_id", s(b, "app_id", ""));
    let app = sk_apps::get(pool, app_id).await?.context("app not found")?;
    let root = app
        .get("root_path")
        .and_then(Value::as_str)
        .context("app has no root_path to archive")?;
    let out = backup_dir().join("applications").join(format!(
        "app_{}_{}.tar.gz",
        clean_name(app_id),
        Utc::now().format("%Y%m%d_%H%M%S")
    ));
    let mut target = b.clone();
    if b.get("include_db")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let Some(db_config) = b.get("db_config") else {
            return Ok(
                json!({"success":false,"error":"db_config is required when include_db is true"}),
            );
        };
        let db_backup = backup_database(pool, db_config).await?;
        if !db_backup["status"]
            .as_str()
            .map(|s| s == "completed")
            .unwrap_or(false)
            && !db_backup["success"].as_bool().unwrap_or(false)
        {
            return Ok(
                json!({"success":false,"error":"included database backup failed","database_backup":db_backup}),
            );
        }
        target["database_backup"] = db_backup;
    }
    let r = tar_create(&out, &[PathBuf::from(root)]);
    if !r["success"].as_bool().unwrap_or(false) {
        return Ok(r);
    }
    insert_record(
        pool,
        "application",
        &format!("application-{app_id}"),
        &out,
        &target,
    )
    .await
}
pub async fn backup_files(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let paths = b
        .get("paths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut inputs = Vec::new();
    for p in paths.iter().filter_map(Value::as_str) {
        if !sk_files::is_path_allowed(p) {
            return Ok(json!({"success":false,"error":format!("Access denied: {p}")}));
        }
        inputs.push(PathBuf::from(p));
    }
    if inputs.is_empty() {
        return Ok(json!({"success":false,"error":"paths are required"}));
    }
    let name = clean_name(opt(b, "name").unwrap_or("files"));
    let out = backup_dir().join("files").join(format!(
        "{name}_{}.tar.gz",
        Utc::now().format("%Y%m%d_%H%M%S")
    ));
    let r = tar_create(&out, &inputs);
    if !r["success"].as_bool().unwrap_or(false) {
        return Ok(r);
    }
    insert_record(pool, "files", &name, &out, b).await
}
pub async fn backup_database(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let db_type = s(b, "db_type", "mysql");
    let db = s(b, "db_name", s(b, "database", ""));
    if db.is_empty() {
        return Ok(json!({"success":false,"error":"db_name is required"}));
    }
    let r = match db_type {
        "mysql" => sk_db::backup(db, opt(b, "password")).await,
        "sqlite" => {
            let src = PathBuf::from(db);
            if !sk_files::is_path_allowed(db) {
                json!({"success":false,"error":"Access denied"})
            } else {
                let out = backup_dir().join("databases").join(format!(
                    "sqlite_{}_{}.db",
                    clean_name(
                        src.file_name()
                            .and_then(|x| x.to_str())
                            .unwrap_or("database")
                    ),
                    Utc::now().format("%Y%m%d_%H%M%S")
                ));
                if let Some(p) = out.parent() {
                    std::fs::create_dir_all(p)?;
                }
                match std::fs::copy(&src, &out) {
                    Ok(_) => json!({"success":true,"path":out,"size":size(&out)}),
                    Err(e) => json!({"success":false,"error":e.to_string()}),
                }
            }
        }
        _ => {
            json!({"success":false,"error":"database backup engine is not available through the general backup subsystem yet; use the engine-specific database backup route"})
        }
    };
    if !r["success"].as_bool().unwrap_or(false) {
        return Ok(r);
    }
    let path = PathBuf::from(r["path"].as_str().unwrap_or(""));
    insert_record(pool, "database", db, &path, b).await
}

pub async fn restore_application(_pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let path = s(b, "backup_path", "");
    if path.is_empty() {
        return Ok(json!({"success":false,"error":"backup_path is required"}));
    }
    let dest = PathBuf::from(opt(b, "restore_path").unwrap_or("/var/www/serverkit-restore"));
    if !sk_files::is_path_allowed(dest.to_string_lossy().as_ref()) {
        return Ok(json!({"success":false,"error":"Access denied"}));
    }
    Ok(tar_extract(&PathBuf::from(path), &dest))
}
pub async fn restore_database(_pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let db_type = s(b, "db_type", "mysql");
    let path = s(b, "backup_path", "");
    let db = s(b, "db_name", s(b, "database", ""));
    if path.is_empty() || db.is_empty() {
        return Ok(json!({"success":false,"error":"backup_path and db_name are required"}));
    }
    Ok(match db_type {
        "mysql" => sk_db::restore(db, path, opt(b, "password")).await,
        "sqlite" => {
            if !sk_files::is_path_allowed(db) {
                json!({"success":false,"error":"Access denied"})
            } else {
                match std::fs::copy(path, db) {
                    Ok(_) => json!({"success":true}),
                    Err(e) => json!({"success":false,"error":e.to_string()}),
                }
            }
        }
        _ => {
            json!({"success":false,"error":"restore engine not available through general backup subsystem"})
        }
    })
}

pub async fn delete(pool: &SqlitePool, id_or_path: &str) -> anyhow::Result<Value> {
    let rec = get_backup(pool, id_or_path).await?;
    let path = rec
        .as_ref()
        .and_then(|r| r.get("path"))
        .and_then(Value::as_str)
        .unwrap_or(id_or_path);
    let removed = std::fs::remove_file(path).is_ok();
    sqlx::query("DELETE FROM sk_backups WHERE id=? OR path=?")
        .bind(id_or_path)
        .bind(id_or_path)
        .execute(pool)
        .await?;
    Ok(json!({"success":removed,"path":path}))
}
pub async fn cleanup(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let days = b.get("retention_days").and_then(Value::as_i64).unwrap_or(7);
    let cutoff = Utc::now() - Duration::days(days);
    let rows = sqlx::query("SELECT * FROM sk_backups")
        .fetch_all(pool)
        .await?;
    let mut deleted = 0;
    for r in rows {
        let created: String = r.get("created_at");
        if chrono::DateTime::parse_from_rfc3339(&created)
            .map(|d| d.with_timezone(&Utc) < cutoff)
            .unwrap_or(false)
        {
            let p: String = r.get("path");
            let _ = std::fs::remove_file(&p);
            sqlx::query("DELETE FROM sk_backups WHERE id=?")
                .bind(r.get::<String, _>("id"))
                .execute(pool)
                .await?;
            deleted += 1;
        }
    }
    Ok(json!({"success":true,"deleted":deleted,"retention_days":days}))
}

pub async fn schedules(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_backup_schedules ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"schedules":rows.iter().map(sched_value).collect::<Vec<_>>() }))
}
pub async fn add_schedule(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_backup_schedules(id,name,backup_type,target_json,schedule_time,days_json,upload_remote,enabled,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(s(b,"name","Backup schedule")).bind(s(b,"backup_type","files")).bind(b.get("target").cloned().unwrap_or(Value::Null).to_string()).bind(s(b,"schedule_time","02:00")).bind(b.get("days").map(Value::to_string)).bind(if b.get("upload_remote").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(if b.get("enabled").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind(&ts).bind(&ts).execute(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_backup_schedules WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    Ok(sched_value(&r))
}
pub async fn update_schedule(pool: &SqlitePool, id: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_backup_schedules WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(old) = old else {
        return Ok(json!({"success":false,"error":"schedule not found"}));
    };
    sqlx::query("UPDATE sk_backup_schedules SET name=?, backup_type=?, target_json=?, schedule_time=?, days_json=?, upload_remote=?, enabled=?, updated_at=? WHERE id=?").bind(opt(b,"name").unwrap_or_else(||old.get::<String,_>("name").leak())).bind(opt(b,"backup_type").unwrap_or_else(||old.get::<String,_>("backup_type").leak())).bind(b.get("target").map(Value::to_string).unwrap_or_else(||old.get("target_json"))).bind(opt(b,"schedule_time").unwrap_or_else(||old.get::<String,_>("schedule_time").leak())).bind(b.get("days").map(Value::to_string).or_else(||old.try_get::<Option<String>,_>("days_json").ok().flatten())).bind(if b.get("upload_remote").and_then(Value::as_bool).unwrap_or(old.get::<i64,_>("upload_remote")!=0){1}else{0}).bind(if b.get("enabled").and_then(Value::as_bool).unwrap_or(old.get::<i64,_>("enabled")!=0){1}else{0}).bind(now()).bind(id).execute(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_backup_schedules WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    Ok(sched_value(&r))
}
pub async fn delete_schedule(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_backup_schedules WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}

async fn remote_path(pool: &SqlitePool, key: &str) -> Result<PathBuf, String> {
    if key.contains("..") {
        return Err("Invalid remote key".into());
    }
    let root = configured_remote_dir(pool)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "Remote storage is not configured".to_string())?;
    Ok(root.join(key.trim_start_matches('/')))
}
pub async fn upload(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let backup_path = s(b, "backup_path", "");
    let Some(rec) = get_backup(pool, backup_path).await? else {
        return Ok(json!({"success":false,"error":"backup not found"}));
    };
    let local = PathBuf::from(rec["path"].as_str().unwrap_or(""));
    let key = local
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("backup");
    let dest = match remote_path(pool, key).await {
        Ok(p) => p,
        Err(e) => return Ok(json!({"success":false,"error":e})),
    };
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::copy(&local, &dest)?;
    sqlx::query("UPDATE sk_backups SET remote_key=?, updated_at=? WHERE id=?")
        .bind(key)
        .bind(now())
        .bind(rec["id"].as_str())
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"remote_key":key,"size":size(&dest)}))
}
pub async fn verify(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let key = s(b, "remote_key", "");
    let local = s(b, "local_path", "");
    let remote = match remote_path(pool, key).await {
        Ok(p) => p,
        Err(e) => return Ok(json!({"success":false,"error":e})),
    };
    let ok = remote.exists() && (local.is_empty() || Path::new(local).exists());
    if ok {
        sqlx::query(
            "UPDATE sk_backups SET verified_at=?, updated_at=? WHERE remote_key=? OR path=?",
        )
        .bind(now())
        .bind(now())
        .bind(key)
        .bind(local)
        .execute(pool)
        .await?;
    }
    Ok(
        json!({"success":ok,"remote_exists":remote.exists(),"local_exists":if local.is_empty(){Value::Null}else{json!(Path::new(local).exists())}}),
    )
}
pub async fn remote_list(pool: &SqlitePool, prefix: Option<&str>) -> anyhow::Result<Value> {
    let Some(root) = configured_remote_dir(pool).await? else {
        return Ok(json!({"success":false,"error":"Remote storage is not configured","items":[]}));
    };
    let dir = root.join(prefix.unwrap_or("").trim_start_matches('/'));
    let mut items = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for e in entries.flatten() {
            let meta = e.metadata().ok();
            items.push(json!({"key":e.path().strip_prefix(&root).unwrap_or(e.path().as_path()).to_string_lossy(),"size":meta.as_ref().map(|m|m.len()).unwrap_or(0),"is_dir":meta.as_ref().map(|m|m.is_dir()).unwrap_or(false)}));
        }
    }
    Ok(json!({"success":true,"items":items,"backups":items}))
}
pub async fn remote_download(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let key = s(b, "remote_key", "");
    let src = match remote_path(pool, key).await {
        Ok(p) => p,
        Err(e) => return Ok(json!({"success":false,"error":e})),
    };
    let dest = PathBuf::from(
        opt(b, "local_path")
            .unwrap_or_else(|| src.file_name().and_then(|n| n.to_str()).unwrap_or("backup")),
    );
    let dest = if dest.is_absolute() {
        dest
    } else {
        backup_dir().join("remote-downloads").join(dest)
    };
    if let Some(p) = dest.parent() {
        std::fs::create_dir_all(p)?;
    }
    std::fs::copy(&src, &dest)?;
    Ok(json!({"success":true,"path":dest,"size":size(&dest)}))
}
