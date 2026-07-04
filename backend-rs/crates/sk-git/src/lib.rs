use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
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
fn s2<'a>(v: &'a Value, a: &str, b: &str, d: &'a str) -> &'a str {
    v.get(a)
        .or_else(|| v.get(b))
        .and_then(Value::as_str)
        .unwrap_or(d)
}
fn b2(v: &Value, a: &str, b: &str, d: bool) -> bool {
    v.get(a)
        .or_else(|| v.get(b))
        .and_then(Value::as_bool)
        .unwrap_or(d)
}
fn i2(v: &Value, a: &str, b: &str, d: i64) -> i64 {
    v.get(a)
        .or_else(|| v.get(b))
        .and_then(Value::as_i64)
        .unwrap_or(d)
}
fn secret() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
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
    PathBuf::from(std::env::var("SK_GIT_DIR").unwrap_or_else(|_| "/var/lib/serverkit/gitea".into()))
}
fn parse_json(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}
fn compose_file(name: &str, http_port: i64, ssh_port: i64) -> String {
    format!(
        r#"services:
  gitea:
    image: gitea/gitea:1.22
    restart: unless-stopped
    environment:
      USER_UID: "1000"
      USER_GID: "1000"
      GITEA__database__DB_TYPE: postgres
      GITEA__database__HOST: db:5432
      GITEA__database__NAME: gitea
      GITEA__database__USER: gitea
      GITEA__database__PASSWD: gitea
      GITEA__server__SSH_PORT: "{ssh_port}"
      GITEA__server__SSH_LISTEN_PORT: "22"
    ports:
      - "{http_port}:3000"
      - "{ssh_port}:22"
    volumes:
      - ./gitea:/data
    depends_on:
      - db
  db:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: gitea
      POSTGRES_PASSWORD: gitea
      POSTGRES_DB: gitea
    volumes:
      - ./postgres:/var/lib/postgresql/data
name: {name}
"#
    )
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS sk_git_server(id TEXT PRIMARY KEY, name TEXT NOT NULL, status TEXT NOT NULL, root_path TEXT NOT NULL, compose_path TEXT NOT NULL, http_port INTEGER NOT NULL, ssh_port INTEGER NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_git_webhooks(id TEXT PRIMARY KEY, name TEXT NOT NULL, source TEXT NOT NULL, source_repo_url TEXT NOT NULL, source_branch TEXT NOT NULL, local_repo_name TEXT, sync_direction TEXT NOT NULL, auto_sync INTEGER NOT NULL, app_id TEXT, deploy_on_push INTEGER NOT NULL, zero_downtime INTEGER NOT NULL, pre_deploy_script TEXT, post_deploy_script TEXT, secret TEXT NOT NULL, is_active INTEGER NOT NULL, sync_count INTEGER NOT NULL DEFAULT 0, last_sync_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_git_webhook_logs(id TEXT PRIMARY KEY, webhook_id TEXT NOT NULL, event TEXT NOT NULL, status TEXT NOT NULL, message TEXT NOT NULL, payload_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_git_deployments(id TEXT PRIMARY KEY, app_id TEXT NOT NULL, webhook_id TEXT, branch TEXT, version TEXT NOT NULL, status TEXT NOT NULL, target_version TEXT, logs_json TEXT NOT NULL DEFAULT '[]', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#,
    )
    .execute(pool)
    .await
    .context("ensure sk-git schema")?;
    Ok(())
}

async fn server_row(pool: &SqlitePool) -> anyhow::Result<Option<sqlx::sqlite::SqliteRow>> {
    Ok(
        sqlx::query("SELECT * FROM sk_git_server ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await?,
    )
}
fn running_from_ps(ps: &Value) -> bool {
    ps.get("stdout")
        .and_then(Value::as_str)
        .map(|x| x.to_lowercase().contains("running"))
        .unwrap_or(false)
}
fn server_json(r: &sqlx::sqlite::SqliteRow) -> Value {
    let compose = r.get::<String, _>("compose_path");
    let ps = if Path::new(&compose).exists() {
        run(
            "docker",
            &["compose", "-f", &compose, "ps", "--format", "json"],
        )
    } else {
        json!({"success":false,"error":"compose file missing"})
    };
    let running = running_from_ps(&ps);
    let status = if running {
        "running".to_string()
    } else {
        r.get::<String, _>("status")
    };
    json!({"installed":true,"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"status":status,"running":running,"root_path":r.get::<String,_>("root_path"),"compose_path":compose,"http_port":r.get::<i64,_>("http_port"),"ssh_port":r.get::<i64,_>("ssh_port"),"url_path":Value::Null,"version":"unknown","docker":ps,"config":parse_json(Some(r.get::<String,_>("config_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}

pub async fn requirements() -> anyhow::Result<Value> {
    let docker = exists("docker");
    let compose = std::process::Command::new("docker")
        .args(["compose", "version"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    Ok(
        json!({"ok":docker&&compose,"requirements":[{"name":"docker","ok":docker},{"name":"docker_compose","ok":compose},{"name":"data_dir_parent","ok":base_dir().parent().map(Path::exists).unwrap_or(false)}],"docker":docker,"compose":compose}),
    )
}
pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    if let Some(r) = server_row(pool).await? {
        return Ok(server_json(&r));
    }
    if let Ok(url) = std::env::var("SK_GITEA_URL") {
        return Ok(
            json!({"installed":true,"external":true,"running":true,"status":"external","http_port":Value::Null,"ssh_port":Value::Null,"base_url":url,"version":"unknown"}),
        );
    }
    Ok(
        json!({"installed":false,"running":false,"status":"not_installed","requirements":requirements().await?}),
    )
}
pub async fn install(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let req = requirements().await?;
    if req["ok"].as_bool() != Some(true) {
        return Ok(
            json!({"success":false,"installed":false,"code":"REQUIREMENTS_NOT_MET","requirements":req}),
        );
    }
    if server_row(pool).await?.is_some() {
        return Ok(json!({"success":true,"installed":true,"git":status(pool).await?}));
    }
    let http_port = i2(body, "httpPort", "http_port", 3000);
    let ssh_port = i2(body, "sshPort", "ssh_port", 2222);
    let name = s(body, "name", "serverkit-gitea");
    let root = base_dir();
    std::fs::create_dir_all(root.join("gitea"))?;
    std::fs::create_dir_all(root.join("postgres"))?;
    let compose = root.join("compose.yaml");
    std::fs::write(&compose, compose_file(name, http_port, ssh_port))?;
    let ts = now();
    sqlx::query("INSERT INTO sk_git_server(id,name,status,root_path,compose_path,http_port,ssh_port,config_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
        .bind(id()).bind(name).bind("installed").bind(root.to_string_lossy().to_string()).bind(compose.to_string_lossy().to_string()).bind(http_port).bind(ssh_port).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"installed":true,"git":status(pool).await?,"admin_user":s2(body,"adminUser","admin_user","admin")}),
    )
}
async fn lifecycle(pool: &SqlitePool, action: &str) -> anyhow::Result<Value> {
    let Some(r) = server_row(pool).await? else {
        return Ok(json!({"success":false,"installed":false,"code":"GIT_NOT_INSTALLED"}));
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
    let new_status = if result["success"].as_bool() == Some(true) {
        if action == "stop" {
            "stopped"
        } else {
            "running"
        }
    } else {
        "error"
    };
    sqlx::query("UPDATE sk_git_server SET status=?,updated_at=? WHERE id=?")
        .bind(new_status)
        .bind(now())
        .bind(r.get::<String, _>("id"))
        .execute(pool)
        .await?;
    Ok(
        json!({"success":result["success"],"action":action,"result":result,"git":status(pool).await?}),
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
pub async fn uninstall(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let Some(r) = server_row(pool).await? else {
        return Ok(json!({"success":false,"installed":false,"code":"GIT_NOT_INSTALLED"}));
    };
    let compose = r.get::<String, _>("compose_path");
    let down = run("docker", &["compose", "-f", &compose, "down"]);
    if b2(body, "removeData", "remove_data", false) {
        let root = r.get::<String, _>("root_path");
        if root.starts_with("/var/lib/serverkit/") {
            let _ = std::fs::remove_dir_all(root);
        }
    }
    let n = sqlx::query("DELETE FROM sk_git_server WHERE id=?")
        .bind(r.get::<String, _>("id"))
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n,"docker":down}))
}

async fn api_base(pool: &SqlitePool) -> anyhow::Result<Option<String>> {
    if let Ok(url) = std::env::var("SK_GITEA_URL") {
        return Ok(Some(url.trim_end_matches('/').to_string()));
    }
    if let Some(r) = server_row(pool).await? {
        return Ok(Some(format!(
            "http://127.0.0.1:{}",
            r.get::<i64, _>("http_port")
        )));
    }
    Ok(None)
}
async fn gitea_get(pool: &SqlitePool, path: &str) -> anyhow::Result<Value> {
    let Some(base) = api_base(pool).await? else {
        return Ok(json!({"success":false,"configured":false,"code":"GIT_NOT_INSTALLED"}));
    };
    let token = std::env::var("SK_GITEA_TOKEN").ok();
    let client = reqwest::Client::new();
    let mut req = client.get(format!("{base}{path}"));
    if let Some(t) = token {
        req = req.bearer_auth(t);
    }
    match req.send().await {
        Ok(resp) => {
            let status = resp.status().as_u16();
            let text = resp.text().await.unwrap_or_default();
            let val: Value = serde_json::from_str(&text).unwrap_or_else(|_| json!({"raw":text}));
            Ok(json!({"success":status<400,"status":status,"data":val}))
        }
        Err(e) => Ok(
            json!({"success":false,"configured":true,"code":"GITEA_UNREACHABLE","error":e.to_string()}),
        ),
    }
}
pub async fn version(pool: &SqlitePool) -> anyhow::Result<Value> {
    let v = gitea_get(pool, "/api/v1/version").await?;
    if v["success"].as_bool() == Some(true) {
        Ok(json!({"success":true,"version":v["data"]["version"].clone(),"gitea":v["data"].clone()}))
    } else {
        Ok(v)
    }
}
pub async fn repos(pool: &SqlitePool, limit: i64) -> anyhow::Result<Value> {
    let v = gitea_get(pool, &format!("/api/v1/repos/search?limit={limit}")).await?;
    if v["success"].as_bool() == Some(true) {
        let data = v["data"]["data"].as_array().cloned().unwrap_or_default();
        Ok(json!({"success":true,"repositories":data,"count":data.len()}))
    } else {
        Ok(json!({"repositories":[],"count":0,"gitea":v,"success":false}))
    }
}
pub async fn repo(pool: &SqlitePool, owner: &str, repo: &str) -> anyhow::Result<Value> {
    let v = gitea_get(pool, &format!("/api/v1/repos/{owner}/{repo}")).await?;
    Ok(if v["success"].as_bool() == Some(true) {
        json!({"success":true,"repository":v["data"].clone()})
    } else {
        v
    })
}
pub async fn stats(pool: &SqlitePool, owner: &str, repo_name: &str) -> anyhow::Result<Value> {
    let r = repo(pool, owner, repo_name).await?;
    Ok(
        json!({"success":r["success"].as_bool().unwrap_or(false),"repository":r.get("repository").cloned().unwrap_or(Value::Null),"stats":{"source":"gitea_api","stars":r["repository"]["stars_count"].clone(),"forks":r["repository"]["forks_count"].clone(),"open_issues":r["repository"]["open_issues_count"].clone()}}),
    )
}
pub async fn branches(pool: &SqlitePool, owner: &str, repo: &str) -> anyhow::Result<Value> {
    let v = gitea_get(pool, &format!("/api/v1/repos/{owner}/{repo}/branches")).await?;
    Ok(if v["success"].as_bool() == Some(true) {
        json!({"success":true,"branches":v["data"].as_array().cloned().unwrap_or_default()})
    } else {
        v
    })
}
pub async fn branch(
    pool: &SqlitePool,
    owner: &str,
    repo: &str,
    branch: &str,
) -> anyhow::Result<Value> {
    let v = gitea_get(
        pool,
        &format!("/api/v1/repos/{owner}/{repo}/branches/{branch}"),
    )
    .await?;
    Ok(if v["success"].as_bool() == Some(true) {
        json!({"success":true,"branch":v["data"].clone()})
    } else {
        v
    })
}
pub async fn commits(
    pool: &SqlitePool,
    owner: &str,
    repo: &str,
    branch: Option<&str>,
    page: i64,
    limit: i64,
) -> anyhow::Result<Value> {
    let mut path = format!("/api/v1/repos/{owner}/{repo}/commits?page={page}&limit={limit}");
    if let Some(b) = branch {
        if !b.is_empty() {
            path.push_str(&format!("&sha={b}"));
        }
    }
    let v = gitea_get(pool, &path).await?;
    Ok(if v["success"].as_bool() == Some(true) {
        json!({"success":true,"commits":v["data"].as_array().cloned().unwrap_or_default()})
    } else {
        v
    })
}
pub async fn commit(
    pool: &SqlitePool,
    owner: &str,
    repo: &str,
    sha: &str,
) -> anyhow::Result<Value> {
    let v = gitea_get(
        pool,
        &format!("/api/v1/repos/{owner}/{repo}/git/commits/{sha}"),
    )
    .await?;
    Ok(if v["success"].as_bool() == Some(true) {
        json!({"success":true,"commit":v["data"].clone()})
    } else {
        v
    })
}
pub async fn contents(
    pool: &SqlitePool,
    owner: &str,
    repo: &str,
    path: &str,
    r#ref: Option<&str>,
) -> anyhow::Result<Value> {
    let suffix = if path.is_empty() {
        String::new()
    } else {
        format!("/{path}")
    };
    let refq = r#ref.unwrap_or("main");
    let v = gitea_get(
        pool,
        &format!("/api/v1/repos/{owner}/{repo}/contents{suffix}?ref={refq}"),
    )
    .await?;
    Ok(if v["success"].as_bool() == Some(true) {
        let d = v["data"].clone();
        let files = if let Some(a) = d.as_array() {
            a.clone()
        } else {
            vec![d.clone()]
        };
        json!({"success":true,"files":files,"content":d})
    } else {
        v
    })
}
pub async fn readme(
    pool: &SqlitePool,
    owner: &str,
    repo: &str,
    r#ref: Option<&str>,
) -> anyhow::Result<Value> {
    let mut path = format!("/api/v1/repos/{owner}/{repo}/raw/README.md");
    if let Some(r) = r#ref {
        path.push_str(&format!("?ref={r}"));
    }
    gitea_get(pool, &path).await
}

fn webhook_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"source":r.get::<String,_>("source"),"source_repo_url":r.get::<String,_>("source_repo_url"),"source_branch":r.get::<String,_>("source_branch"),"local_repo_name":r.get::<Option<String>,_>("local_repo_name"),"sync_direction":r.get::<String,_>("sync_direction"),"auto_sync":r.get::<i64,_>("auto_sync")!=0,"app_id":r.get::<Option<String>,_>("app_id"),"deploy_on_push":r.get::<i64,_>("deploy_on_push")!=0,"zero_downtime":r.get::<i64,_>("zero_downtime")!=0,"pre_deploy_script":r.get::<Option<String>,_>("pre_deploy_script"),"post_deploy_script":r.get::<Option<String>,_>("post_deploy_script"),"is_active":r.get::<i64,_>("is_active")!=0,"sync_count":r.get::<i64,_>("sync_count"),"last_sync_at":r.get::<Option<String>,_>("last_sync_at"),"webhook_url":format!("/v1/git/webhooks/{}/receive",r.get::<String,_>("id")),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn webhooks(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_git_webhooks ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let vals: Vec<Value> = rows.into_iter().map(webhook_row).collect();
    Ok(json!({"success":true,"webhooks":vals,"count":vals.len()}))
}
pub async fn webhook(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_git_webhooks WHERE id=?")
        .bind(wid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"success":true,"webhook":webhook_row(r)}),
        None => json!({"success":false,"code":"WEBHOOK_NOT_FOUND","error":"Webhook not found"}),
    })
}
pub async fn create_webhook(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    if s(body, "name", "").is_empty() || s2(body, "sourceRepoUrl", "source_repo_url", "").is_empty()
    {
        return Ok(json!({"success":false,"error":"name and sourceRepoUrl are required"}));
    }
    let wid = id();
    let sec = secret();
    let ts = now();
    sqlx::query("INSERT INTO sk_git_webhooks(id,name,source,source_repo_url,source_branch,local_repo_name,sync_direction,auto_sync,app_id,deploy_on_push,zero_downtime,pre_deploy_script,post_deploy_script,secret,is_active,sync_count,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&wid).bind(s(body,"name","Webhook")).bind(s(body,"source","github")).bind(s2(body,"sourceRepoUrl","source_repo_url","")).bind(s2(body,"sourceBranch","source_branch","main")).bind(s2(body,"localRepoName","local_repo_name","")).bind(s2(body,"syncDirection","sync_direction","pull")).bind(if b2(body,"autoSync","auto_sync",true){1}else{0}).bind(s2(body,"appId","app_id","")).bind(if b2(body,"deployOnPush","deploy_on_push",false){1}else{0}).bind(if b2(body,"zeroDowntime","zero_downtime",false){1}else{0}).bind(s2(body,"preDeployScript","pre_deploy_script","")).bind(s2(body,"postDeployScript","post_deploy_script","")).bind(&sec).bind(1).bind(0).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"secret":sec,"webhook":webhook(pool,&wid).await?["webhook"].clone()}))
}
pub async fn update_webhook(pool: &SqlitePool, wid: &str, body: &Value) -> anyhow::Result<Value> {
    if webhook(pool, wid).await?["success"].as_bool() != Some(true) {
        return Ok(json!({"success":false,"code":"WEBHOOK_NOT_FOUND"}));
    }
    sqlx::query("UPDATE sk_git_webhooks SET name=?,source=?,source_repo_url=?,source_branch=?,local_repo_name=?,sync_direction=?,auto_sync=?,app_id=?,deploy_on_push=?,zero_downtime=?,pre_deploy_script=?,post_deploy_script=?,updated_at=? WHERE id=?")
        .bind(s(body,"name","Webhook")).bind(s(body,"source","github")).bind(s2(body,"sourceRepoUrl","source_repo_url","")).bind(s2(body,"sourceBranch","source_branch","main")).bind(s2(body,"localRepoName","local_repo_name","")).bind(s2(body,"syncDirection","sync_direction","pull")).bind(if b2(body,"autoSync","auto_sync",true){1}else{0}).bind(s2(body,"appId","app_id","")).bind(if b2(body,"deployOnPush","deploy_on_push",false){1}else{0}).bind(if b2(body,"zeroDowntime","zero_downtime",false){1}else{0}).bind(s2(body,"preDeployScript","pre_deploy_script","")).bind(s2(body,"postDeployScript","post_deploy_script","")).bind(now()).bind(wid).execute(pool).await?;
    webhook(pool, wid).await
}
pub async fn delete_webhook(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_git_webhook_logs WHERE webhook_id=?")
        .bind(wid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_git_webhooks WHERE id=?")
        .bind(wid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn toggle_webhook(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    let Some(r) = sqlx::query("SELECT is_active FROM sk_git_webhooks WHERE id=?")
        .bind(wid)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(json!({"success":false,"code":"WEBHOOK_NOT_FOUND"}));
    };
    let next = if r.get::<i64, _>("is_active") != 0 {
        0
    } else {
        1
    };
    sqlx::query("UPDATE sk_git_webhooks SET is_active=?,updated_at=? WHERE id=?")
        .bind(next)
        .bind(now())
        .bind(wid)
        .execute(pool)
        .await?;
    Ok(
        json!({"success":true,"message":if next!=0{"Webhook enabled"}else{"Webhook disabled"},"webhook":webhook(pool,wid).await?["webhook"].clone()}),
    )
}
async fn add_log(
    pool: &SqlitePool,
    wid: &str,
    event: &str,
    status: &str,
    message: &str,
    payload: Value,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sk_git_webhook_logs(id,webhook_id,event,status,message,payload_json,created_at) VALUES(?,?,?,?,?,?,?)").bind(id()).bind(wid).bind(event).bind(status).bind(message).bind(payload.to_string()).bind(now()).execute(pool).await?;
    Ok(())
}
pub async fn test_webhook(pool: &SqlitePool, wid: &str) -> anyhow::Result<Value> {
    if webhook(pool, wid).await?["success"].as_bool() != Some(true) {
        return Ok(json!({"success":false,"code":"WEBHOOK_NOT_FOUND"}));
    }
    add_log(
        pool,
        wid,
        "test",
        "success",
        "Test event logged",
        json!({"source":"serverkit","kind":"test"}),
    )
    .await?;
    Ok(json!({"success":true,"message":"Test event logged"}))
}
pub async fn webhook_logs(pool: &SqlitePool, wid: &str, limit: i64) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_git_webhook_logs WHERE webhook_id=? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(wid)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let vals:Vec<Value>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"webhook_id":r.get::<String,_>("webhook_id"),"event":r.get::<String,_>("event"),"status":r.get::<String,_>("status"),"message":r.get::<String,_>("message"),"payload":parse_json(Some(r.get::<String,_>("payload_json"))),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"success":true,"logs":vals,"count":vals.len()}))
}
fn dep_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"app_id":r.get::<String,_>("app_id"),"webhook_id":r.get::<Option<String>,_>("webhook_id"),"branch":r.get::<Option<String>,_>("branch"),"version":r.get::<String,_>("version"),"status":r.get::<String,_>("status"),"target_version":r.get::<Option<String>,_>("target_version"),"logs":parse_json(Some(r.get::<String,_>("logs_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
async fn create_deployment(
    pool: &SqlitePool,
    app_id: &str,
    webhook_id: Option<&str>,
    branch: Option<&str>,
    target: Option<&str>,
    kind: &str,
) -> anyhow::Result<Value> {
    let did = id();
    let ts = now();
    let version = format!("{}", Utc::now().timestamp());
    let logs = json!([{"ts":ts,"level":"info","message":format!("Git {kind} request persisted by sk-git")}]);
    sqlx::query("INSERT INTO sk_git_deployments(id,app_id,webhook_id,branch,version,status,target_version,logs_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(&did).bind(app_id).bind(webhook_id).bind(branch).bind(&version).bind("queued").bind(target).bind(logs.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"version":version,"deployment":deployment(pool,&did,true).await?["deployment"].clone(),"message":"Deployment request persisted"}),
    )
}
pub async fn app_deployments(pool: &SqlitePool, app_id: &str, limit: i64) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_git_deployments WHERE app_id=? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(app_id)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let vals: Vec<Value> = rows.into_iter().map(dep_row).collect();
    Ok(json!({"success":true,"deployments":vals,"count":vals.len()}))
}
pub async fn webhook_deployments(
    pool: &SqlitePool,
    wid: &str,
    limit: i64,
) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_git_deployments WHERE webhook_id=? ORDER BY created_at DESC LIMIT ?",
    )
    .bind(wid)
    .bind(limit)
    .fetch_all(pool)
    .await?;
    let vals: Vec<Value> = rows.into_iter().map(dep_row).collect();
    Ok(json!({"success":true,"deployments":vals,"count":vals.len()}))
}
pub async fn deployment(pool: &SqlitePool, did: &str, include_logs: bool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_git_deployments WHERE id=?")
        .bind(did)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            let mut d = dep_row(r);
            if !include_logs {
                if let Some(o) = d.as_object_mut() {
                    o.remove("logs");
                }
            }
            json!({"success":true,"deployment":d})
        }
        None => json!({"success":false,"code":"DEPLOYMENT_NOT_FOUND"}),
    })
}
pub async fn trigger_deploy(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    create_deployment(
        pool,
        app_id,
        None,
        body.get("branch").and_then(Value::as_str),
        None,
        "deploy",
    )
    .await
}
pub async fn rollback(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    create_deployment(
        pool,
        app_id,
        None,
        None,
        body.get("targetVersion")
            .or_else(|| body.get("target_version"))
            .and_then(Value::as_str),
        "rollback",
    )
    .await
}
