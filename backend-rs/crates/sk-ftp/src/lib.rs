use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
fn now() -> String {
    Utc::now().to_rfc3339()
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
fn exists(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn run(cmd: &str, args: &[&str]) -> Value {
    match std::process::Command::new(cmd).args(args).output() {
        Ok(o) => {
            json!({"success":o.status.success(),"stdout":String::from_utf8_lossy(&o.stdout).trim(),"stderr":String::from_utf8_lossy(&o.stderr).trim(),"code":o.status.code()})
        }
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}
fn installed(service: &str) -> bool {
    exists(service)
        || std::path::Path::new(&format!("/etc/{service}.conf")).exists()
        || run(
            "systemctl",
            &[
                "list-unit-files",
                &format!("{service}.service"),
                "--no-pager",
            ],
        )["success"]
            .as_bool()
            == Some(true)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_ftp_config(service TEXT PRIMARY KEY, config_json TEXT NOT NULL DEFAULT '{}', updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_ftp_users(username TEXT PRIMARY KEY, password_encrypted TEXT, home_dir TEXT, enabled INTEGER NOT NULL DEFAULT 1, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-ftp schema")?;
    Ok(())
}

pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let service = config(pool, None).await?["service"]
        .as_str()
        .unwrap_or("vsftpd")
        .to_string();
    let active = run("systemctl", &["is-active", &service]);
    Ok(
        json!({"service":service,"installed":installed(&service),"active":active["stdout"].as_str()==Some("active"),"systemd":active,"configured_users":users(pool).await?["count"],"config":config(pool,Some(&service)).await?}),
    )
}
pub async fn config(pool: &SqlitePool, service: Option<&str>) -> anyhow::Result<Value> {
    let service = service.unwrap_or("vsftpd");
    let row = sqlx::query("SELECT config_json FROM sk_ftp_config WHERE service=?")
        .bind(service)
        .fetch_optional(pool)
        .await?;
    let cfg=row.map(|r|j(Some(r.get("config_json")))).unwrap_or_else(||json!({"anonymous_enable":false,"local_enable":true,"write_enable":true,"chroot_local_user":true,"pasv_min_port":40000,"pasv_max_port":40100}));
    Ok(json!({"service":service,"installed":installed(service),"config":cfg}))
}
pub async fn set_config(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let service = s(b, "service", "vsftpd");
    let cfg = b.get("config").cloned().unwrap_or_else(|| b.clone());
    sqlx::query("INSERT INTO sk_ftp_config(service,config_json,updated_at) VALUES(?,?,?) ON CONFLICT(service) DO UPDATE SET config_json=excluded.config_json,updated_at=excluded.updated_at").bind(service).bind(cfg.to_string()).bind(now()).execute(pool).await?;
    Ok(
        json!({"success":true,"service":service,"config":cfg,"applied_to_system":false,"reason":"Desired state persisted; apply requires an installed FTP daemon"}),
    )
}
fn user_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"username":r.get::<String,_>("username"),"home_dir":r.get::<Option<String>,_>("home_dir"),"enabled":r.get::<i64,_>("enabled")!=0,"metadata":j(Some(r.get("metadata_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn users(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_ftp_users ORDER BY username")
        .fetch_all(pool)
        .await?;
    let vals: Vec<Value> = rows.into_iter().map(user_row).collect();
    Ok(json!({"users":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_user(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let username = s(b, "username", "");
    if username.is_empty() {
        return Ok(
            json!({"success":false,"code":"VALIDATION_ERROR","error":"username is required"}),
        );
    }
    let pw = opt(b, "password").map(sk_core::crypto::encrypt);
    let home = opt(b, "home_dir").unwrap_or("/home");
    let ts = now();
    sqlx::query("INSERT INTO sk_ftp_users(username,password_encrypted,home_dir,enabled,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?) ON CONFLICT(username) DO UPDATE SET password_encrypted=excluded.password_encrypted,home_dir=excluded.home_dir,enabled=1,metadata_json=excluded.metadata_json,updated_at=excluded.updated_at").bind(username).bind(pw).bind(home).bind(1).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"user":{"username":username,"home_dir":home,"enabled":true},"system_user_created":false,"reason":"Virtual FTP user desired state persisted"}),
    )
}
pub async fn delete_user(
    pool: &SqlitePool,
    username: &str,
    delete_home: bool,
) -> anyhow::Result<Value> {
    let home = sqlx::query("SELECT home_dir FROM sk_ftp_users WHERE username=?")
        .bind(username)
        .fetch_optional(pool)
        .await?
        .and_then(|r| r.get::<Option<String>, _>("home_dir"));
    let n = sqlx::query("DELETE FROM sk_ftp_users WHERE username=?")
        .bind(username)
        .execute(pool)
        .await?
        .rows_affected();
    if delete_home {
        if let Some(h) = home {
            if h.starts_with("/home/") || h.starts_with("/srv/") {
                let _ = std::fs::remove_dir_all(h);
            }
        }
    }
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn change_password(
    pool: &SqlitePool,
    username: &str,
    b: &Value,
) -> anyhow::Result<Value> {
    let pw = opt(b, "password").map(sk_core::crypto::encrypt);
    let n =
        sqlx::query("UPDATE sk_ftp_users SET password_encrypted=?,updated_at=? WHERE username=?")
            .bind(pw)
            .bind(now())
            .bind(username)
            .execute(pool)
            .await?
            .rows_affected();
    Ok(json!({"success":n>0,"password_changed":n>0}))
}
pub async fn toggle_user(pool: &SqlitePool, username: &str, b: &Value) -> anyhow::Result<Value> {
    let enabled = b.get("enabled").and_then(Value::as_bool).unwrap_or(true);
    let n = sqlx::query("UPDATE sk_ftp_users SET enabled=?,updated_at=? WHERE username=?")
        .bind(if enabled { 1 } else { 0 })
        .bind(now())
        .bind(username)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"enabled":enabled}))
}
pub async fn connections() -> anyhow::Result<Value> {
    let ss = if exists("ss") {
        run("ss", &["-tnp", "sport = :21"])
    } else {
        json!({"success":false,"error":"ss not installed"})
    };
    Ok(json!({"connections":[],"raw":ss,"source":"ss"}))
}
pub async fn disconnect(pid: &str) -> anyhow::Result<Value> {
    if pid.parse::<i64>().is_ok() {
        let r = run("kill", &[pid]);
        Ok(json!({"success":r["success"],"result":r}))
    } else {
        Ok(
            json!({"success":false,"code":"INVALID_PID","error":"Connection id must be a process id"}),
        )
    }
}
pub async fn logs(lines: i64) -> anyhow::Result<Value> {
    let n = lines.clamp(1, 1000).to_string();
    Ok(
        json!({"logs":run("journalctl",&["-u","vsftpd","-u","proftpd","-n",&n,"--no-pager"]),"lines":n}),
    )
}
pub async fn install(b: &Value) -> anyhow::Result<Value> {
    let service = s(b, "service", "vsftpd");
    if installed(service) {
        Ok(json!({"success":true,"installed":true,"service":service}))
    } else {
        Ok(
            json!({"success":false,"installed":false,"service":service,"code":"FTP_DAEMON_NOT_INSTALLED","error":"FTP daemon is not installed; install the package with the system package manager before enabling"}),
        )
    }
}
pub async fn service(action: &str, b: &Value) -> anyhow::Result<Value> {
    let service = s(b, "service", "vsftpd");
    if !installed(service) {
        return Ok(
            json!({"success":false,"installed":false,"code":"FTP_DAEMON_NOT_INSTALLED","service":service}),
        );
    }
    if !["start", "stop", "restart", "reload", "status"].contains(&action) {
        return Ok(
            json!({"success":false,"code":"INVALID_ACTION","error":"Unsupported service action"}),
        );
    }
    let r = if action == "status" {
        run("systemctl", &["status", service, "--no-pager"])
    } else {
        run("systemctl", &[action, service])
    };
    Ok(json!({"success":r["success"],"service":service,"action":action,"result":r}))
}
pub async fn test(b: &Value) -> anyhow::Result<Value> {
    let host = s(b, "host", "localhost");
    let port = b.get("port").and_then(Value::as_u64).unwrap_or(21);
    let addr = format!("{host}:{port}");
    let ok = addr
        .to_socket_addrs()
        .ok()
        .and_then(|mut a| a.next())
        .map(|a| TcpStream::connect_timeout(&a, Duration::from_secs(5)).is_ok())
        .unwrap_or(false);
    Ok(
        json!({"success":ok,"host":host,"port":port,"authenticated":false,"message":if ok{"TCP connection succeeded"}else{"TCP connection failed"}}),
    )
}
