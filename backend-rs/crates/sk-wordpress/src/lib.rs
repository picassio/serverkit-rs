use anyhow::Context;
use chrono::Utc;
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
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
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
fn base_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SK_WORDPRESS_DIR")
            .unwrap_or_else(|_| "/var/lib/serverkit/wordpress-standalone".into()),
    )
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_wordpress_standalone(id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL, root_path TEXT NOT NULL, compose_path TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-wordpress schema")?;
    Ok(())
}

pub async fn requirements() -> anyhow::Result<Value> {
    let docker = exists("docker");
    let compose = std::process::Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    let disk = run(
        "df",
        &[
            "-h",
            base_dir()
                .parent()
                .and_then(Path::to_str)
                .unwrap_or("/var/lib/serverkit"),
        ],
    );
    Ok(
        json!({"requirements":[{"name":"docker","ok":docker},{"name":"docker_compose","ok":compose},{"name":"data_dir_parent","ok":base_dir().parent().map(Path::exists).unwrap_or(false)}],"ok":docker&&compose,"docker":docker,"compose":compose,"disk":disk}),
    )
}
fn compose_file(name: &str) -> String {
    format!(
        r#"services:
  wordpress:
    image: wordpress:latest
    restart: unless-stopped
    ports:
      - "8088:80"
    environment:
      WORDPRESS_DB_HOST: db
      WORDPRESS_DB_USER: wordpress
      WORDPRESS_DB_PASSWORD: wordpress
      WORDPRESS_DB_NAME: wordpress
    volumes:
      - ./html:/var/www/html
  db:
    image: mariadb:10.11
    restart: unless-stopped
    environment:
      MYSQL_DATABASE: wordpress
      MYSQL_USER: wordpress
      MYSQL_PASSWORD: wordpress
      MYSQL_ROOT_PASSWORD: wordpress-root
    volumes:
      - ./db:/var/lib/mysql
name: {name}
"#
    )
}
async fn row(pool: &SqlitePool) -> anyhow::Result<Option<sqlx::sqlite::SqliteRow>> {
    Ok(
        sqlx::query("SELECT * FROM sk_wordpress_standalone ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await?,
    )
}
fn row_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    let compose_path = r.get::<String, _>("compose_path");
    let docker = if Path::new(&compose_path).exists() {
        run(
            "docker",
            &["compose", "-f", &compose_path, "ps", "--format", "json"],
        )
    } else {
        json!({"success":false,"error":"compose file missing"})
    };
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"installed":true,"root_path":r.get::<String,_>("root_path"),"compose_path":compose_path,"config":serde_json::from_str::<Value>(&r.get::<String,_>("config_json")).unwrap_or(Value::Null),"docker":docker,"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    Ok(match row(pool).await? {
        Some(r) => row_json(&r),
        None => {
            json!({"installed":false,"status":"not_installed","requirements":requirements().await?})
        }
    })
}
pub async fn install(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let req = requirements().await?;
    if req["ok"].as_bool() != Some(true) {
        return Ok(
            json!({"success":false,"installed":false,"code":"REQUIREMENTS_NOT_MET","requirements":req}),
        );
    }
    let wid = id();
    let name = s(b, "name", "serverkit-wordpress");
    let root = base_dir();
    std::fs::create_dir_all(root.join("html"))?;
    std::fs::create_dir_all(root.join("db"))?;
    let compose = root.join("compose.yaml");
    std::fs::write(&compose, compose_file(name))?;
    let ts = now();
    sqlx::query("INSERT INTO sk_wordpress_standalone(id,name,status,root_path,compose_path,config_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(&wid).bind(name).bind("installed").bind(root.to_string_lossy().to_string()).bind(compose.to_string_lossy().to_string()).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"installed":true,"wordpress":status(pool).await?}))
}
async fn lifecycle(pool: &SqlitePool, action: &str) -> anyhow::Result<Value> {
    let Some(r) = row(pool).await? else {
        return Ok(json!({"success":false,"installed":false,"code":"WORDPRESS_NOT_INSTALLED"}));
    };
    let compose = r.get::<String, _>("compose_path");
    if !Path::new(&compose).exists() {
        return Ok(json!({"success":false,"code":"COMPOSE_MISSING","compose_path":compose}));
    }
    let args = match action {
        "start" => vec!["compose", "-f", &compose, "up", "-d"],
        "stop" => vec!["compose", "-f", &compose, "stop"],
        "restart" => vec!["compose", "-f", &compose, "restart"],
        _ => vec!["compose", "-f", &compose, "ps"],
    };
    let result = run("docker", &args);
    let status = if result["success"].as_bool() == Some(true) {
        if action == "stop" {
            "stopped"
        } else {
            "running"
        }
    } else {
        "error"
    };
    sqlx::query("UPDATE sk_wordpress_standalone SET status=?,updated_at=? WHERE id=?")
        .bind(status)
        .bind(now())
        .bind(r.get::<String, _>("id"))
        .execute(pool)
        .await?;
    Ok(
        json!({"success":result["success"],"action":action,"result":result,"wordpress":crate::status(pool).await?}),
    )
}
pub async fn start(pool: &SqlitePool) -> anyhow::Result<Value> {
    lifecycle(pool, "start").await
}
pub async fn stop(pool: &SqlitePool) -> anyhow::Result<Value> {
    lifecycle(pool, "stop").await
}
pub async fn restart(pool: &SqlitePool) -> anyhow::Result<Value> {
    lifecycle(pool, "restart").await
}
pub async fn uninstall(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let Some(r) = row(pool).await? else {
        return Ok(json!({"success":false,"installed":false,"code":"WORDPRESS_NOT_INSTALLED"}));
    };
    let compose = r.get::<String, _>("compose_path");
    let _ = run("docker", &["compose", "-f", &compose, "down"]);
    if b.get("removeData")
        .or_else(|| b.get("remove_data"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let root = r.get::<String, _>("root_path");
        if root.starts_with("/var/lib/serverkit/") {
            let _ = std::fs::remove_dir_all(root);
        }
    }
    let n = sqlx::query("DELETE FROM sk_wordpress_standalone WHERE id=?")
        .bind(r.get::<String, _>("id"))
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
