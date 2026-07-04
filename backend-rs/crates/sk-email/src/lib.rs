use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::time::Duration;
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
fn b(v: &Value, k: &str, d: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(d)
}
fn i(v: &Value, k: &str, d: i64) -> i64 {
    v.get(k).and_then(Value::as_i64).unwrap_or(d)
}
fn base_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SK_EMAIL_DIR").unwrap_or_else(|_| "/var/lib/serverkit/email".into()),
    )
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
fn service_status(name: &str) -> Value {
    let installed = exists("systemctl")
        && run("systemctl", &["list-unit-files", name])["success"].as_bool() == Some(true);
    let active = if installed {
        run("systemctl", &["is-active", name])
    } else {
        json!({"success":false})
    };
    json!({"installed":installed,"running":active["success"].as_bool()==Some(true),"status":active["stdout"].as_str().unwrap_or("not-installed")})
}
fn compose_file(hostname: &str, webmail: bool) -> String {
    let roundcube = if webmail {
        r#"
  roundcube:
    image: roundcube/roundcubemail:latest
    restart: unless-stopped
    ports:
      - "8089:80"
    environment:
      ROUNDCUBEMAIL_DEFAULT_HOST: mailserver
      ROUNDCUBEMAIL_SMTP_SERVER: mailserver
    depends_on:
      - mailserver
"#
    } else {
        ""
    };
    format!(
        r#"services:
  mailserver:
    image: mailserver/docker-mailserver:latest
    hostname: {hostname}
    restart: unless-stopped
    ports:
      - "25:25"
      - "143:143"
      - "465:465"
      - "587:587"
      - "993:993"
    volumes:
      - ./mail-data:/var/mail
      - ./mail-state:/var/mail-state
      - ./mail-logs:/var/log/mail
      - ./config:/tmp/docker-mailserver
    environment:
      ENABLE_SPAMASSASSIN: "1"
      ENABLE_CLAMAV: "0"
      ENABLE_OPENDKIM: "1"
      ENABLE_POSTGREY: "0"
      ONE_DIR: "1"
{roundcube}name: serverkit-email
"#
    )
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_email_server(id TEXT PRIMARY KEY, hostname TEXT NOT NULL, status TEXT NOT NULL, root_path TEXT NOT NULL, compose_path TEXT NOT NULL, webmail_installed INTEGER NOT NULL DEFAULT 0, webmail_running INTEGER NOT NULL DEFAULT 0, webmail_proxy_domain TEXT, config_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_domains(id TEXT PRIMARY KEY, name TEXT NOT NULL UNIQUE, dns_provider_id TEXT, dns_zone_id TEXT, dkim_public_key TEXT, spf_record TEXT, dmarc_record TEXT, is_active INTEGER NOT NULL DEFAULT 1, dns_verified_at TEXT, dns_deployed_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_accounts(id TEXT PRIMARY KEY, domain_id TEXT NOT NULL, username TEXT NOT NULL, email TEXT NOT NULL UNIQUE, password_enc TEXT NOT NULL, quota_mb INTEGER NOT NULL, is_active INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_aliases(id TEXT PRIMARY KEY, domain_id TEXT NOT NULL, source TEXT NOT NULL, destination TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_forwarding(id TEXT PRIMARY KEY, account_id TEXT NOT NULL, destination TEXT NOT NULL, keep_copy INTEGER NOT NULL DEFAULT 1, is_active INTEGER NOT NULL DEFAULT 1, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_dns_providers(id TEXT PRIMARY KEY, name TEXT NOT NULL, provider TEXT NOT NULL, api_key_enc TEXT NOT NULL, api_secret_enc TEXT, api_email TEXT, is_default INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_relay(id INTEGER PRIMARY KEY CHECK(id=1), enabled INTEGER NOT NULL DEFAULT 0, host TEXT, port INTEGER, username TEXT, password_enc TEXT, from_email TEXT, use_tls INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_email_spam_config(id INTEGER PRIMARY KEY CHECK(id=1), required_score REAL NOT NULL DEFAULT 5.0, rewrite_subject TEXT NOT NULL DEFAULT '***SPAM***', use_bayes INTEGER NOT NULL DEFAULT 1, bayes_auto_learn INTEGER NOT NULL DEFAULT 1, updated_at TEXT NOT NULL);
INSERT OR IGNORE INTO sk_email_spam_config(id,updated_at) VALUES(1, datetime('now'));
"#).execute(pool).await.context("ensure sk-email schema")?;
    Ok(())
}
async fn server(pool: &SqlitePool) -> anyhow::Result<Option<sqlx::sqlite::SqliteRow>> {
    Ok(
        sqlx::query("SELECT * FROM sk_email_server ORDER BY created_at DESC LIMIT 1")
            .fetch_optional(pool)
            .await?,
    )
}
fn component(installed: bool, running: bool) -> Value {
    json!({"installed":installed,"running":running,"status":if running{"running"}else if installed{"stopped"}else{"not_installed"}})
}
pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    if let Some(r) = server(pool).await? {
        let compose = r.get::<String, _>("compose_path");
        let ps = if Path::new(&compose).exists() {
            run(
                "docker",
                &["compose", "-f", &compose, "ps", "--format", "json"],
            )
        } else {
            json!({"success":false,"error":"compose missing"})
        };
        let running = ps["stdout"]
            .as_str()
            .unwrap_or("")
            .to_lowercase()
            .contains("running");
        let webmail_inst = r.get::<i64, _>("webmail_installed") != 0;
        return Ok(
            json!({"installed":true,"running":running,"hostname":r.get::<String,_>("hostname"),"postfix":component(true,running),"dovecot":component(true,running),"dkim":component(true,running),"spamassassin":component(true,running),"roundcube":component(webmail_inst,webmail_inst&&running),"docker":ps,"root_path":r.get::<String,_>("root_path"),"compose_path":compose}),
        );
    }
    Ok(
        json!({"installed":false,"running":false,"postfix":service_status("postfix"),"dovecot":service_status("dovecot"),"dkim":service_status("opendkim"),"spamassassin":service_status("spamassassin"),"roundcube":service_status("apache2")}),
    )
}
pub async fn install(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    if server(pool).await?.is_some() {
        return Ok(json!({"success":true,"email":status(pool).await?}));
    }
    if !exists("docker") {
        return Ok(json!({"success":false,"code":"DOCKER_NOT_AVAILABLE"}));
    }
    let hostname = s(body, "hostname", "mail.serverkit.local");
    let root = base_dir();
    std::fs::create_dir_all(root.join("config"))?;
    std::fs::create_dir_all(root.join("mail-data"))?;
    std::fs::create_dir_all(root.join("mail-state"))?;
    std::fs::create_dir_all(root.join("mail-logs"))?;
    let compose = root.join("compose.yaml");
    std::fs::write(&compose, compose_file(hostname, false))?;
    let ts = now();
    sqlx::query("INSERT INTO sk_email_server(id,hostname,status,root_path,compose_path,config_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(id()).bind(hostname).bind("installed").bind(root.to_string_lossy().to_string()).bind(compose.to_string_lossy().to_string()).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"email":status(pool).await?}))
}
pub async fn control(pool: &SqlitePool, component: &str, action: &str) -> anyhow::Result<Value> {
    let Some(r) = server(pool).await? else {
        return Ok(json!({"success":false,"code":"EMAIL_NOT_INSTALLED"}));
    };
    let compose = r.get::<String, _>("compose_path");
    let svc = if component == "roundcube" || component == "webmail" {
        "roundcube"
    } else {
        "mailserver"
    };
    let args = match action {
        "start" => vec!["compose", "-f", &compose, "up", "-d", svc],
        "stop" => vec!["compose", "-f", &compose, "stop", svc],
        "restart" => vec!["compose", "-f", &compose, "restart", svc],
        _ => vec!["compose", "-f", &compose, "ps", svc],
    };
    let res = run("docker", &args);
    Ok(
        json!({"success":res["success"],"component":component,"action":action,"result":res,"email":status(pool).await?}),
    )
}
fn dom_row(r: sqlx::sqlite::SqliteRow, accounts: i64, aliases: i64) -> Value {
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"dns_provider_id":r.get::<Option<String>,_>("dns_provider_id"),"dns_zone_id":r.get::<Option<String>,_>("dns_zone_id"),"dkim_public_key":r.get::<Option<String>,_>("dkim_public_key"),"spf_record":r.get::<Option<String>,_>("spf_record"),"dmarc_record":r.get::<Option<String>,_>("dmarc_record"),"is_active":r.get::<i64,_>("is_active")!=0,"dns_verified_at":r.get::<Option<String>,_>("dns_verified_at"),"accounts_count":accounts,"aliases_count":aliases,"created_at":r.get::<String,_>("created_at")})
}
pub async fn domains(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_email_domains ORDER BY name")
        .fetch_all(pool)
        .await?;
    let mut vals = Vec::new();
    for r in rows {
        let did = r.get::<String, _>("id");
        let ac: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM sk_email_accounts WHERE domain_id=?")
                .bind(&did)
                .fetch_one(pool)
                .await?;
        let al: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_email_aliases WHERE domain_id=?")
            .bind(&did)
            .fetch_one(pool)
            .await?;
        vals.push(dom_row(r, ac, al));
    }
    Ok(json!({"success":true,"domains":vals,"count":vals.len()}))
}
pub async fn add_domain(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let name = s(body, "name", "").trim();
    if name.is_empty() {
        return Ok(json!({"success":false,"error":"domain name required"}));
    }
    let did = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_email_domains(id,name,dns_provider_id,dns_zone_id,dkim_public_key,spf_record,dmarc_record,is_active,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)").bind(&did).bind(name).bind(s(body,"dns_provider_id","")).bind(s(body,"dns_zone_id","")).bind(format!("v=DKIM1; k=rsa; p=managed-by-serverkit-{did}")).bind("v=spf1 mx -all".to_string()).bind(format!("v=DMARC1; p=quarantine; rua=mailto:postmaster@{name}")).bind(1).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"domain":domain(pool,&did).await?["domain"].clone()}))
}
pub async fn domain(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_email_domains WHERE id=?")
        .bind(did)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            let ac: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM sk_email_accounts WHERE domain_id=?")
                    .bind(did)
                    .fetch_one(pool)
                    .await?;
            let al: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM sk_email_aliases WHERE domain_id=?")
                    .bind(did)
                    .fetch_one(pool)
                    .await?;
            json!({"success":true,"domain":dom_row(r,ac,al)})
        }
        None => json!({"success":false,"code":"DOMAIN_NOT_FOUND"}),
    })
}
pub async fn delete_domain(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let acct = sqlx::query("SELECT id FROM sk_email_accounts WHERE domain_id=?")
        .bind(did)
        .fetch_all(pool)
        .await?;
    for a in acct {
        sqlx::query("DELETE FROM sk_email_forwarding WHERE account_id=?")
            .bind(a.get::<String, _>("id"))
            .execute(pool)
            .await?;
    }
    sqlx::query("DELETE FROM sk_email_accounts WHERE domain_id=?")
        .bind(did)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_email_aliases WHERE domain_id=?")
        .bind(did)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_email_domains WHERE id=?")
        .bind(did)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
async fn domain_name(pool: &SqlitePool, did: &str) -> anyhow::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT name FROM sk_email_domains WHERE id=?")
            .bind(did)
            .fetch_optional(pool)
            .await?,
    )
}
fn acct_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"domain_id":r.get::<String,_>("domain_id"),"username":r.get::<String,_>("username"),"email":r.get::<String,_>("email"),"quota_mb":r.get::<i64,_>("quota_mb"),"is_active":r.get::<i64,_>("is_active")!=0,"created_at":r.get::<String,_>("created_at")})
}
pub async fn accounts(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_email_accounts WHERE domain_id=? ORDER BY email")
        .bind(did)
        .fetch_all(pool)
        .await?;
    let vals: Vec<_> = rows.into_iter().map(acct_row).collect();
    Ok(json!({"success":true,"accounts":vals,"count":vals.len()}))
}
pub async fn create_account(pool: &SqlitePool, did: &str, body: &Value) -> anyhow::Result<Value> {
    let Some(dom) = domain_name(pool, did).await? else {
        return Ok(json!({"success":false,"code":"DOMAIN_NOT_FOUND"}));
    };
    let user = s(body, "username", "");
    let pass = s(body, "password", "");
    if user.is_empty() || pass.is_empty() {
        return Ok(json!({"success":false,"error":"username and password required"}));
    }
    let email = format!("{user}@{dom}");
    let aid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_email_accounts(id,domain_id,username,email,password_enc,quota_mb,is_active,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(&aid).bind(did).bind(user).bind(&email).bind(sk_core::crypto::encrypt(pass)).bind(i(body,"quota_mb",1024)).bind(1).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"account":account(pool,&aid).await?["account"].clone()}))
}
pub async fn account(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_email_accounts WHERE id=?")
        .bind(aid)
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => json!({"success":true,"account":acct_row(r)}),
        None => json!({"success":false,"code":"ACCOUNT_NOT_FOUND"}),
    })
}
pub async fn update_account(pool: &SqlitePool, aid: &str, body: &Value) -> anyhow::Result<Value> {
    let n=sqlx::query("UPDATE sk_email_accounts SET quota_mb=COALESCE(?,quota_mb), is_active=COALESCE(?,is_active), updated_at=? WHERE id=?").bind(body.get("quota_mb").and_then(Value::as_i64)).bind(body.get("is_active").and_then(Value::as_bool).map(|x|if x{1}else{0})).bind(now()).bind(aid).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"account":account(pool,aid).await?["account"].clone()}))
}
pub async fn delete_account(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_email_forwarding WHERE account_id=?")
        .bind(aid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_email_accounts WHERE id=?")
        .bind(aid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn change_password(pool: &SqlitePool, aid: &str, body: &Value) -> anyhow::Result<Value> {
    let pass = s(body, "password", "");
    if pass.is_empty() {
        return Ok(json!({"success":false,"error":"password required"}));
    }
    let n = sqlx::query("UPDATE sk_email_accounts SET password_enc=?,updated_at=? WHERE id=?")
        .bind(sk_core::crypto::encrypt(pass))
        .bind(now())
        .bind(aid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0}))
}
fn alias_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"domain_id":r.get::<String,_>("domain_id"),"source":r.get::<String,_>("source"),"destination":r.get::<String,_>("destination"),"created_at":r.get::<String,_>("created_at")})
}
pub async fn aliases(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_email_aliases WHERE domain_id=? ORDER BY source")
        .bind(did)
        .fetch_all(pool)
        .await?;
    let vals: Vec<_> = rows.into_iter().map(alias_row).collect();
    Ok(json!({"success":true,"aliases":vals,"count":vals.len()}))
}
pub async fn create_alias(pool: &SqlitePool, did: &str, body: &Value) -> anyhow::Result<Value> {
    let alid = id();
    sqlx::query("INSERT INTO sk_email_aliases(id,domain_id,source,destination,created_at) VALUES(?,?,?,?,?)").bind(&alid).bind(did).bind(s(body,"source","")).bind(s(body,"destination","")).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"alias":{"id":alid}}))
}
pub async fn delete_alias(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_email_aliases WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
fn fwd_row(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"account_id":r.get::<String,_>("account_id"),"account_email":r.get::<String,_>("account_email"),"destination":r.get::<String,_>("destination"),"keep_copy":r.get::<i64,_>("keep_copy")!=0,"is_active":r.get::<i64,_>("is_active")!=0})
}
pub async fn forwarding(pool: &SqlitePool, aid: &str) -> anyhow::Result<Value> {
    let rows=sqlx::query("SELECT f.*, a.email account_email FROM sk_email_forwarding f JOIN sk_email_accounts a ON a.id=f.account_id WHERE f.account_id=? ORDER BY f.created_at DESC").bind(aid).fetch_all(pool).await?;
    let vals: Vec<_> = rows.into_iter().map(fwd_row).collect();
    Ok(json!({"success":true,"rules":vals,"count":vals.len()}))
}
pub async fn create_forwarding(
    pool: &SqlitePool,
    aid: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let fid = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_email_forwarding(id,account_id,destination,keep_copy,is_active,created_at,updated_at) VALUES(?,?,?,?,?,?,?)").bind(&fid).bind(aid).bind(s(body,"destination","")).bind(if b(body,"keep_copy",true){1}else{0}).bind(1).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"rule":{"id":fid}}))
}
pub async fn update_forwarding(
    pool: &SqlitePool,
    fid: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let n=sqlx::query("UPDATE sk_email_forwarding SET destination=COALESCE(?,destination), keep_copy=COALESCE(?,keep_copy), is_active=COALESCE(?,is_active), updated_at=? WHERE id=?").bind(body.get("destination").and_then(Value::as_str)).bind(body.get("keep_copy").and_then(Value::as_bool).map(|x|if x{1}else{0})).bind(body.get("is_active").and_then(Value::as_bool).map(|x|if x{1}else{0})).bind(now()).bind(fid).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0}))
}
pub async fn delete_forwarding(pool: &SqlitePool, fid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_email_forwarding WHERE id=?")
        .bind(fid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
fn provider_row(r: sqlx::sqlite::SqliteRow) -> Value {
    let key = r.get::<String, _>("api_key_enc");
    json!({"id":r.get::<String,_>("id"),"name":r.get::<String,_>("name"),"provider":r.get::<String,_>("provider"),"api_key":if key.is_empty(){""}else{"••••••"},"api_email":r.get::<Option<String>,_>("api_email"),"is_default":r.get::<i64,_>("is_default")!=0})
}
pub async fn providers(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_email_dns_providers ORDER BY is_default DESC,name")
        .fetch_all(pool)
        .await?;
    let vals: Vec<_> = rows.into_iter().map(provider_row).collect();
    Ok(json!({"success":true,"providers":vals,"count":vals.len()}))
}
pub async fn add_provider(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let pid = id();
    let ts = now();
    if b(body, "is_default", false) {
        sqlx::query("UPDATE sk_email_dns_providers SET is_default=0")
            .execute(pool)
            .await?;
    }
    sqlx::query("INSERT INTO sk_email_dns_providers(id,name,provider,api_key_enc,api_secret_enc,api_email,is_default,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(&pid).bind(s(body,"name","DNS Provider")).bind(s(body,"provider","cloudflare")).bind(sk_core::crypto::encrypt(s(body,"api_key",""))).bind(if s(body,"api_secret","").is_empty(){None}else{Some(sk_core::crypto::encrypt(s(body,"api_secret","")))}).bind(s(body,"api_email","")).bind(if b(body,"is_default",false){1}else{0}).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(json!({"success":true,"provider_id":pid}))
}
pub async fn delete_provider(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_email_dns_providers WHERE id=?")
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn test_provider(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let Some(r) = sqlx::query("SELECT * FROM sk_email_dns_providers WHERE id=?")
        .bind(pid)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(json!({"success":false,"code":"PROVIDER_NOT_FOUND"}));
    };
    if r.get::<String, _>("provider") != "cloudflare" {
        return Ok(json!({"success":false,"code":"DNS_PROVIDER_ADAPTER_UNAVAILABLE"}));
    }
    let token = sk_core::crypto::decrypt_or_plain(&r.get::<String, _>("api_key_enc"));
    let resp = reqwest::Client::new()
        .get("https://api.cloudflare.com/client/v4/user/tokens/verify")
        .bearer_auth(token)
        .send()
        .await;
    Ok(match resp {
        Ok(r) => json!({"success":r.status().is_success(),"status":r.status().as_u16()}),
        Err(e) => json!({"success":false,"error":e.to_string()}),
    })
}
pub async fn zones(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let Some(r) = sqlx::query("SELECT * FROM sk_email_dns_providers WHERE id=?")
        .bind(pid)
        .fetch_optional(pool)
        .await?
    else {
        return Ok(json!({"success":false,"code":"PROVIDER_NOT_FOUND","zones":[]}));
    };
    if r.get::<String, _>("provider") != "cloudflare" {
        return Ok(json!({"success":false,"code":"DNS_PROVIDER_ADAPTER_UNAVAILABLE","zones":[]}));
    }
    let token = sk_core::crypto::decrypt_or_plain(&r.get::<String, _>("api_key_enc"));
    let resp = reqwest::Client::new()
        .get("https://api.cloudflare.com/client/v4/zones")
        .bearer_auth(token)
        .send()
        .await;
    match resp {
        Ok(r) => {
            let status = r.status().as_u16();
            let v: Value = r.json().await.unwrap_or_else(|_| json!({}));
            Ok(
                json!({"success":status<400,"zones":v["result"].as_array().cloned().unwrap_or_default(),"status":status}),
            )
        }
        Err(e) => Ok(json!({"success":false,"error":e.to_string(),"zones":[]})),
    }
}
pub async fn verify_dns(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let d = domain(pool, did).await?;
    let name = d["domain"]["name"].as_str().unwrap_or("");
    let dig = exists("dig");
    let spf = if dig {
        run("dig", &["+short", "TXT", name])
    } else {
        json!({"success":false,"error":"dig not installed"})
    };
    let ok = spf["stdout"].as_str().unwrap_or("").contains("v=spf1");
    if ok {
        sqlx::query("UPDATE sk_email_domains SET dns_verified_at=?,updated_at=? WHERE id=?")
            .bind(now())
            .bind(now())
            .bind(did)
            .execute(pool)
            .await?;
    }
    Ok(
        json!({"success":true,"all_verified":ok,"records":[{"type":"TXT","name":name,"expected":"v=spf1 mx -all","verified":ok}],"dig":spf}),
    )
}
pub async fn deploy_dns(pool: &SqlitePool, did: &str) -> anyhow::Result<Value> {
    let d = domain(pool, did).await?;
    if d["success"].as_bool() != Some(true) {
        return Ok(d);
    };
    let provider = d["domain"]["dns_provider_id"].as_str().unwrap_or("");
    if provider.is_empty() {
        return Ok(json!({"success":false,"code":"DNS_PROVIDER_NOT_CONFIGURED"}));
    }
    sqlx::query("UPDATE sk_email_domains SET dns_deployed_at=?,updated_at=? WHERE id=?")
        .bind(now())
        .bind(now())
        .bind(did)
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"message":"DNS deployment request recorded","provider_id":provider}))
}
pub async fn relay(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_email_relay WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            json!({"success":true,"relay":{"enabled":r.get::<i64,_>("enabled")!=0,"host":r.get::<Option<String>,_>("host"),"port":r.get::<Option<i64>,_>("port"),"username":r.get::<Option<String>,_>("username"),"from_email":r.get::<Option<String>,_>("from_email"),"use_tls":r.get::<i64,_>("use_tls")!=0}})
        }
        None => json!({"success":true,"relay":{"enabled":false,"use_tls":true}}),
    })
}
pub async fn update_relay(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_email_relay(id,enabled,host,port,username,password_enc,from_email,use_tls,updated_at) VALUES(1,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled,host=excluded.host,port=excluded.port,username=excluded.username,password_enc=excluded.password_enc,from_email=excluded.from_email,use_tls=excluded.use_tls,updated_at=excluded.updated_at").bind(if b(body,"enabled",true){1}else{0}).bind(s(body,"host","")).bind(i(body,"port",587)).bind(s(body,"username","")).bind(sk_core::crypto::encrypt(s(body,"password",""))).bind(s(body,"from_email","")).bind(if b(body,"use_tls",true){1}else{0}).bind(now()).execute(pool).await?;
    relay(pool).await
}
pub async fn disable_relay(pool: &SqlitePool) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_email_relay WHERE id=1")
        .execute(pool)
        .await?;
    Ok(json!({"success":true}))
}
pub async fn test_relay(_pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let host = s(body, "host", "");
    let port = i(body, "port", 587) as u16;
    if host.is_empty() {
        return Ok(json!({"success":false,"error":"host required"}));
    }
    let addr = (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut x| x.next());
    let Some(addr) = addr else {
        return Ok(json!({"success":false,"error":"could not resolve host"}));
    };
    match TcpStream::connect_timeout(&addr, Duration::from_secs(5)) {
        Ok(_) => Ok(json!({"success":true,"message":"TCP connection succeeded"})),
        Err(e) => Ok(json!({"success":false,"error":e.to_string()})),
    }
}
pub async fn spam(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_email_spam_config WHERE id=1")
        .fetch_one(pool)
        .await?;
    Ok(
        json!({"success":true,"config":{"required_score":r.get::<f64,_>("required_score"),"rewrite_subject":r.get::<String,_>("rewrite_subject"),"use_bayes":r.get::<i64,_>("use_bayes"),"bayes_auto_learn":r.get::<i64,_>("bayes_auto_learn")}}),
    )
}
pub async fn update_spam(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    sqlx::query("UPDATE sk_email_spam_config SET required_score=?,rewrite_subject=?,use_bayes=?,bayes_auto_learn=?,updated_at=? WHERE id=1").bind(body.get("required_score").and_then(Value::as_f64).unwrap_or(5.0)).bind(s(body,"rewrite_subject","***SPAM***")).bind(i(body,"use_bayes",1)).bind(i(body,"bayes_auto_learn",1)).bind(now()).execute(pool).await?;
    spam(pool).await
}
pub async fn update_spam_rules() -> anyhow::Result<Value> {
    if exists("sa-update") {
        let r = run("sa-update", &[]);
        Ok(json!({"success":r["success"],"message":"sa-update executed","result":r}))
    } else {
        Ok(json!({"success":false,"code":"SA_UPDATE_NOT_INSTALLED"}))
    }
}
pub async fn webmail_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let Some(r) = server(pool).await? else {
        return Ok(json!({"installed":false,"running":false}));
    };
    let installed = r.get::<i64, _>("webmail_installed") != 0;
    Ok(
        json!({"installed":installed,"running":installed && status(pool).await?["running"].as_bool()==Some(true),"port":8089,"proxy_domain":r.get::<Option<String>,_>("webmail_proxy_domain")}),
    )
}
pub async fn webmail_install(pool: &SqlitePool) -> anyhow::Result<Value> {
    let Some(r) = server(pool).await? else {
        return Ok(json!({"success":false,"code":"EMAIL_NOT_INSTALLED"}));
    };
    let compose = r.get::<String, _>("compose_path");
    std::fs::write(
        &compose,
        compose_file(&r.get::<String, _>("hostname"), true),
    )?;
    sqlx::query("UPDATE sk_email_server SET webmail_installed=1,updated_at=? WHERE id=?")
        .bind(now())
        .bind(r.get::<String, _>("id"))
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"webmail":webmail_status(pool).await?}))
}
pub async fn webmail_control(pool: &SqlitePool, action: &str) -> anyhow::Result<Value> {
    control(pool, "roundcube", action).await
}
pub async fn webmail_proxy(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let domain = s(body, "domain", "");
    if domain.is_empty() {
        return Ok(json!({"success":false,"error":"domain required"}));
    }
    let n=sqlx::query("UPDATE sk_email_server SET webmail_proxy_domain=?,updated_at=? WHERE id=(SELECT id FROM sk_email_server ORDER BY created_at DESC LIMIT 1)").bind(domain).bind(now()).execute(pool).await?.rows_affected();
    Ok(json!({"success":n>0,"domain":domain,"message":"proxy desired state recorded"}))
}
pub async fn queue(pool: &SqlitePool) -> anyhow::Result<Value> {
    let cmd = if let Some(r) = server(pool).await? {
        let compose = r.get::<String, _>("compose_path");
        run(
            "docker",
            &[
                "compose",
                "-f",
                &compose,
                "exec",
                "-T",
                "mailserver",
                "postqueue",
                "-j",
            ],
        )
    } else if exists("postqueue") {
        run("postqueue", &["-j"])
    } else {
        json!({"success":false,"code":"POSTQUEUE_NOT_INSTALLED"})
    };
    Ok(json!({"success":cmd["success"].as_bool().unwrap_or(false),"queue":[],"source":cmd}))
}
pub async fn flush_queue(pool: &SqlitePool) -> anyhow::Result<Value> {
    let res = if let Some(r) = server(pool).await? {
        let compose = r.get::<String, _>("compose_path");
        run(
            "docker",
            &[
                "compose",
                "-f",
                &compose,
                "exec",
                "-T",
                "mailserver",
                "postqueue",
                "-f",
            ],
        )
    } else if exists("postqueue") {
        run("postqueue", &["-f"])
    } else {
        json!({"success":false,"code":"POSTQUEUE_NOT_INSTALLED"})
    };
    Ok(json!({"success":res["success"],"result":res}))
}
pub async fn delete_queue(pool: &SqlitePool, qid: &str) -> anyhow::Result<Value> {
    let res = if let Some(r) = server(pool).await? {
        let compose = r.get::<String, _>("compose_path");
        run(
            "docker",
            &[
                "compose",
                "-f",
                &compose,
                "exec",
                "-T",
                "mailserver",
                "postsuper",
                "-d",
                qid,
            ],
        )
    } else if exists("postsuper") {
        run("postsuper", &["-d", qid])
    } else {
        json!({"success":false,"code":"POSTSUPER_NOT_INSTALLED"})
    };
    Ok(json!({"success":res["success"],"queue_id":qid,"result":res}))
}
pub async fn logs(pool: &SqlitePool, lines: i64) -> anyhow::Result<Value> {
    let res = if let Some(r) = server(pool).await? {
        let compose = r.get::<String, _>("compose_path");
        run(
            "docker",
            &[
                "compose",
                "-f",
                &compose,
                "logs",
                "--tail",
                &lines.to_string(),
            ],
        )
    } else if exists("journalctl") {
        run(
            "journalctl",
            &["-u", "postfix", "-n", &lines.to_string(), "--no-pager"],
        )
    } else {
        json!({"success":false,"code":"JOURNALCTL_NOT_INSTALLED"})
    };
    let logs = res["stdout"]
        .as_str()
        .unwrap_or("")
        .lines()
        .map(|x| x.to_string())
        .collect::<Vec<_>>();
    Ok(json!({"success":res["success"].as_bool().unwrap_or(false),"logs":logs,"source":res}))
}
