use anyhow::Context;
use chrono::Utc;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
fn which(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn run(cmd: &str, args: &[&str]) -> Value {
    match std::process::Command::new(cmd).args(args).output() {
        Ok(o) => {
            json!({"success":o.status.success(),"stdout":String::from_utf8_lossy(&o.stdout).trim(),"stderr":String::from_utf8_lossy(&o.stderr).trim()})
        }
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}
fn safe_path(p: &str) -> bool {
    let p = Path::new(p);
    p.is_absolute()
        && !p
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
        && ["/home", "/var/www", "/opt", "/srv", "/tmp"]
            .iter()
            .any(|root| p.starts_with(root))
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_security_config(id INTEGER PRIMARY KEY CHECK(id=1), config_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_security_events(id TEXT PRIMARY KEY, kind TEXT NOT NULL, severity TEXT NOT NULL, message TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_security_scans(id TEXT PRIMARY KEY, scan_type TEXT NOT NULL, target TEXT, status TEXT NOT NULL, result_json TEXT NOT NULL DEFAULT '{}', started_at TEXT NOT NULL, finished_at TEXT);
CREATE TABLE IF NOT EXISTS sk_security_quarantine(id TEXT PRIMARY KEY, original_path TEXT NOT NULL, quarantine_path TEXT NOT NULL, status TEXT NOT NULL, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_security_integrity(path TEXT PRIMARY KEY, sha256 TEXT NOT NULL, size INTEGER NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_security_ip_lists(list_type TEXT NOT NULL, ip TEXT NOT NULL, comment TEXT, created_at TEXT NOT NULL, PRIMARY KEY(list_type,ip));
CREATE TABLE IF NOT EXISTS sk_waf_policies(app_id TEXT PRIMARY KEY, policy_json TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_waf_events(id TEXT PRIMARY KEY, app_id TEXT NOT NULL, event_json TEXT NOT NULL, created_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-security schema")?;
    Ok(())
}
async fn event(
    pool: &SqlitePool,
    kind: &str,
    severity: &str,
    message: &str,
    meta: Value,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sk_security_events(id,kind,severity,message,metadata_json,created_at) VALUES(?,?,?,?,?,?)").bind(id()).bind(kind).bind(severity).bind(message).bind(meta.to_string()).bind(now()).execute(pool).await?;
    Ok(())
}

pub async fn config(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT config_json FROM sk_security_config WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(r.map(|r|j(Some(r.get("config_json")))).unwrap_or_else(||json!({"scan_paths":["/var/www"],"auto_updates":true,"quarantine_dir":"/var/lib/serverkit/quarantine"})))
}
pub async fn set_config(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_security_config(id,config_json,updated_at) VALUES(1,?,?) ON CONFLICT(id) DO UPDATE SET config_json=excluded.config_json,updated_at=excluded.updated_at").bind(b.to_string()).bind(now()).execute(pool).await?;
    event(
        pool,
        "config",
        "info",
        "Security configuration updated",
        b.clone(),
    )
    .await?;
    config(pool).await
}
pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let fw = firewall_status().await;
    let clam = clamav_status().await;
    let fail = fail2ban_status().await;
    let lyn = lynis_status().await;
    let checks = vec![
        json!({"key":"firewall","ok":fw["enabled"].as_bool().unwrap_or(false)}),
        json!({"key":"clamav","ok":clam["installed"].as_bool().unwrap_or(false)}),
        json!({"key":"fail2ban","ok":fail["installed"].as_bool().unwrap_or(false)}),
        json!({"key":"auto_updates","ok":auto_updates_status().await["enabled"].as_bool().unwrap_or(false)}),
    ];
    let score = checks
        .iter()
        .filter(|c| c["ok"].as_bool() == Some(true))
        .count();
    Ok(
        json!({"score":score,"max_score":checks.len(),"checks":checks,"firewall":fw,"clamav":clam,"fail2ban":fail,"lynis":lyn,"config":config(pool).await?}),
    )
}
pub async fn audit(pool: &SqlitePool) -> anyhow::Result<Value> {
    Ok(
        json!({"generated_at":now(),"status":status(pool).await?,"failed_logins":failed_logins(24).await?,"events":events(pool,50).await?}),
    )
}
pub async fn events(pool: &SqlitePool, limit: i64) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_security_events ORDER BY created_at DESC LIMIT ?")
        .bind(limit.clamp(1, 1000))
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"events":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"kind":r.get::<String,_>("kind"),"severity":r.get::<String,_>("severity"),"message":r.get::<String,_>("message"),"metadata":j(Some(r.get::<String,_>("metadata_json"))),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}

pub async fn firewall_status() -> Value {
    let installed = which("ufw");
    let out = if installed {
        run("ufw", &["status", "verbose"])
    } else {
        json!({"success":false})
    };
    let text = out["stdout"].as_str().unwrap_or("");
    json!({"installed":installed,"enabled":text.contains("Status: active"),"default":text.lines().find(|l|l.starts_with("Default:")).unwrap_or(""),"logging":text.lines().find(|l|l.starts_with("Logging:")).unwrap_or(""),"backend":"ufw"})
}
pub async fn firewall_rules() -> Value {
    if !which("ufw") {
        return json!({"installed":false,"rules":[]});
    }
    let out = run("ufw", &["status", "numbered"]);
    let mut rules = Vec::new();
    for line in out["stdout"].as_str().unwrap_or("").lines() {
        let l = line.trim();
        if let Some(rest) = l.strip_prefix('[') {
            if let Some((num, body)) = rest.split_once(']') {
                rules.push(json!({"index":num.trim().parse::<u32>().unwrap_or(0),"raw":body.trim(),"action":if body.contains("ALLOW"){"allow"}else if body.contains("DENY"){"deny"}else{""}}));
            }
        }
    }
    json!({"installed":true,"rules":rules})
}
pub async fn firewall_run(pool: &SqlitePool, args: &[String]) -> anyhow::Result<Value> {
    if !which("ufw") {
        return Ok(json!({"success":false,"installed":false,"error":"ufw is not installed"}));
    }
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let r = run("ufw", &refs);
    event(
        pool,
        "firewall",
        if r["success"].as_bool() == Some(true) {
            "info"
        } else {
            "warning"
        },
        "ufw command executed",
        json!({"args":args,"result":r}),
    )
    .await?;
    Ok(r)
}
pub async fn firewall_enable(pool: &SqlitePool) -> anyhow::Result<Value> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "5000".into());
    let _ = firewall_run(pool, &["allow".into(), "22/tcp".into()]).await;
    let _ = firewall_run(pool, &["allow".into(), format!("{port}/tcp")]).await;
    firewall_run(pool, &["--force".into(), "enable".into()]).await
}
pub async fn firewall_disable(pool: &SqlitePool) -> anyhow::Result<Value> {
    firewall_run(pool, &["disable".into()]).await
}
pub async fn firewall_install(_pool: &SqlitePool) -> anyhow::Result<Value> {
    Ok(if which("ufw") {
        json!({"success":true,"installed":true,"firewall":"ufw"})
    } else {
        json!({"success":false,"installed":false,"error":"ufw is not installed; install the ufw package on the host"})
    })
}
pub async fn firewall_add_rule(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let action = s(b, "action", "allow");
    if !matches!(action, "allow" | "deny" | "reject" | "limit") {
        return Ok(json!({"success":false,"error":"invalid action"}));
    }
    let mut args = vec![action.to_string()];
    if let Some(from) = opt(b, "from").or_else(|| opt(b, "ip")) {
        args.extend(["from".into(), from.into()]);
    }
    if let Some(port) = opt(b, "port")
        .map(str::to_string)
        .or_else(|| b.get("port").and_then(Value::as_i64).map(|n| n.to_string()))
    {
        if opt(b, "from").or_else(|| opt(b, "ip")).is_some() {
            args.extend(["to".into(), "any".into(), "port".into(), port]);
            if let Some(proto) = opt(b, "protocol") {
                args.extend(["proto".into(), proto.into()]);
            }
        } else {
            args.push(format!("{}/{}", port, s(b, "protocol", "tcp")));
        }
    }
    firewall_run(pool, &args).await
}
pub async fn firewall_del_rule(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let idx = b
        .get("index")
        .or_else(|| b.get("number"))
        .and_then(Value::as_u64)
        .or_else(|| opt(b, "index").and_then(|s| s.parse().ok()));
    if let Some(i) = idx {
        firewall_run(pool, &["--force".into(), "delete".into(), i.to_string()]).await
    } else {
        Ok(json!({"success":false,"error":"rule index required"}))
    }
}
pub async fn firewall_block_ip(pool: &SqlitePool, ip: &str) -> anyhow::Result<Value> {
    firewall_add_rule(pool, &json!({"action":"deny","from":ip})).await
}
pub async fn firewall_unblock_ip(pool: &SqlitePool, ip: &str) -> anyhow::Result<Value> {
    if !which("ufw") {
        return Ok(json!({"success":false,"installed":false,"error":"ufw is not installed"}));
    }
    let _ = run("ufw", &["delete", "deny", "from", ip]);
    event(
        pool,
        "firewall",
        "info",
        "IP unblock requested",
        json!({"ip":ip}),
    )
    .await?;
    Ok(json!({"success":true,"ip":ip}))
}
pub async fn blocked_ips() -> Value {
    let rules = firewall_rules().await;
    let blocked: Vec<Value> = rules["rules"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|r| r["action"].as_str() == Some("deny"))
        .cloned()
        .collect();
    json!({"blocked":blocked,"ips":blocked})
}
pub fn zones() -> Value {
    json!({"zones":[{"name":"public","default":true,"backend":"ufw"}],"default":"public"})
}
pub async fn set_default_zone(_pool: &SqlitePool, zone: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":zone=="public","zone":zone,"message":"ufw uses a single public profile in this adapter"}),
    )
}

pub async fn clamav_status() -> Value {
    json!({"installed":which("clamscan"),"daemon_installed":which("clamdscan"),"running":service_active("clamav-daemon"),"freshclam_installed":which("freshclam")})
}
pub async fn clamav_install() -> Value {
    if which("clamscan") {
        json!({"success":true,"installed":true})
    } else {
        json!({"success":false,"installed":false,"error":"ClamAV is not installed; install clamav/clamav-daemon packages on the host"})
    }
}
pub async fn clamav_update(pool: &SqlitePool) -> anyhow::Result<Value> {
    if !which("freshclam") {
        return Ok(json!({"success":false,"error":"freshclam is not installed"}));
    }
    let r = run("freshclam", &[]);
    event(
        pool,
        "clamav",
        "info",
        "Virus definitions update requested",
        r.clone(),
    )
    .await?;
    Ok(r)
}
pub async fn clamav_start(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = run("systemctl", &["start", "clamav-daemon"]);
    event(pool, "clamav", "info", "ClamAV start requested", r.clone()).await?;
    Ok(r)
}
async fn record_scan(
    pool: &SqlitePool,
    typ: &str,
    target: Option<&str>,
    status: &str,
    result: Value,
) -> anyhow::Result<Value> {
    let scan_id = id();
    let ts = now();
    sqlx::query("INSERT INTO sk_security_scans(id,scan_type,target,status,result_json,started_at,finished_at) VALUES(?,?,?,?,?,?,?)").bind(&scan_id).bind(typ).bind(target).bind(status).bind(result.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"id":scan_id,"scan_type":typ,"target":target,"status":status,"result":result,"started_at":ts,"finished_at":ts}),
    )
}
pub async fn scan_path(
    pool: &SqlitePool,
    typ: &str,
    path: &str,
    recursive: bool,
) -> anyhow::Result<Value> {
    if !safe_path(path) {
        return Ok(json!({"success":false,"error":"path is not allowed"}));
    }
    if !which("clamscan") {
        let r = json!({"success":false,"configured":false,"error":"clamscan is not installed"});
        return record_scan(pool, typ, Some(path), "unavailable", r).await;
    }
    let mut args = vec![path];
    if recursive {
        args.insert(0, "-r");
    }
    let r = run("clamscan", &args);
    record_scan(
        pool,
        typ,
        Some(path),
        if r["success"].as_bool() == Some(true) {
            "clean"
        } else {
            "found-or-error"
        },
        r,
    )
    .await
}
pub async fn scan_quick(pool: &SqlitePool) -> anyhow::Result<Value> {
    scan_path(pool, "quick", "/tmp", false).await
}
pub async fn scan_full(pool: &SqlitePool) -> anyhow::Result<Value> {
    scan_path(pool, "full", "/var/www", true).await
}
pub async fn scan_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_security_scans ORDER BY started_at DESC LIMIT 1")
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref()
        .map(scan_value)
        .unwrap_or_else(|| json!({"status":"idle"})))
}
pub async fn scan_cancel() -> Value {
    json!({"success":true,"status":"idle","message":"No background scanner is running"})
}
pub async fn scan_history(pool: &SqlitePool, limit: i64) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_security_scans ORDER BY started_at DESC LIMIT ?")
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?;
    Ok(json!({"scans":rows.iter().map(scan_value).collect::<Vec<_>>() }))
}
fn scan_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"scan_type":r.get::<String,_>("scan_type"),"target":r.try_get::<Option<String>,_>("target").ok().flatten(),"status":r.get::<String,_>("status"),"result":j(Some(r.get::<String,_>("result_json"))),"started_at":r.get::<String,_>("started_at"),"finished_at":r.try_get::<Option<String>,_>("finished_at").ok().flatten()})
}

pub async fn quarantine_list(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_security_quarantine ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"files":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"original_path":r.get::<String,_>("original_path"),"quarantine_path":r.get::<String,_>("quarantine_path"),"status":r.get::<String,_>("status"),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn quarantine_add(pool: &SqlitePool, path: &str) -> anyhow::Result<Value> {
    if !safe_path(path) {
        return Ok(json!({"success":false,"error":"path is not allowed"}));
    }
    let src = Path::new(path);
    if !src.exists() {
        return Ok(json!({"success":false,"error":"file not found"}));
    }
    let dir = PathBuf::from("/var/lib/serverkit/quarantine");
    std::fs::create_dir_all(&dir)?;
    let q = dir.join(format!(
        "{}-{}",
        Utc::now().timestamp(),
        src.file_name().and_then(|n| n.to_str()).unwrap_or("file")
    ));
    std::fs::rename(src, &q).or_else(|_| {
        std::fs::copy(src, &q)?;
        std::fs::remove_file(src)
    })?;
    let qid = id();
    sqlx::query("INSERT INTO sk_security_quarantine(id,original_path,quarantine_path,status,created_at) VALUES(?,?,?,?,?)").bind(&qid).bind(path).bind(q.to_string_lossy().to_string()).bind("quarantined").bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"id":qid,"quarantine_path":q}))
}
pub async fn quarantine_delete(pool: &SqlitePool, id_or_file: &str) -> anyhow::Result<Value> {
    let r =
        sqlx::query("SELECT * FROM sk_security_quarantine WHERE id=? OR quarantine_path LIKE ?")
            .bind(id_or_file)
            .bind(format!("%{id_or_file}"))
            .fetch_optional(pool)
            .await?;
    if let Some(r) = r {
        let p: String = r.get("quarantine_path");
        let _ = std::fs::remove_file(&p);
        sqlx::query("DELETE FROM sk_security_quarantine WHERE id=?")
            .bind(r.get::<String, _>("id"))
            .execute(pool)
            .await?;
        Ok(json!({"success":true}))
    } else {
        Ok(json!({"success":false,"error":"quarantine entry not found"}))
    }
}

pub async fn integrity_initialize(pool: &SqlitePool, paths: &[String]) -> anyhow::Result<Value> {
    let mut count = 0;
    for p in paths {
        if safe_path(p) {
            for f in walk_files(Path::new(p), 50) {
                if let Ok((hash, size)) = hash_file(&f) {
                    sqlx::query("INSERT INTO sk_security_integrity(path,sha256,size,updated_at) VALUES(?,?,?,?) ON CONFLICT(path) DO UPDATE SET sha256=excluded.sha256,size=excluded.size,updated_at=excluded.updated_at").bind(f.to_string_lossy().to_string()).bind(hash).bind(size as i64).bind(now()).execute(pool).await?;
                    count += 1;
                }
            }
        }
    }
    Ok(json!({"success":true,"files":count}))
}
pub async fn integrity_check(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_security_integrity")
        .fetch_all(pool)
        .await?;
    let mut changed = Vec::new();
    for r in rows {
        let p: String = r.get("path");
        match hash_file(Path::new(&p)){Ok((h,size)) if h==r.get::<String,_>("sha256") && size as i64==r.get::<i64,_>("size")=>{},Ok((h,size))=>changed.push(json!({"path":p,"old_sha256":r.get::<String,_>("sha256"),"new_sha256":h,"old_size":r.get::<i64,_>("size"),"new_size":size})),Err(_)=>changed.push(json!({"path":p,"missing":true}))}
    }
    Ok(json!({"success":true,"changed":changed,"changed_count":changed.len()}))
}
fn walk_files(root: &Path, limit: usize) -> Vec<PathBuf> {
    let mut out = Vec::new();
    if root.is_file() {
        out.push(root.to_path_buf());
        return out;
    }
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            if out.len() >= limit {
                break;
            }
            let p = e.path();
            if p.is_file() {
                out.push(p)
            } else if p.is_dir() {
                out.extend(walk_files(&p, limit - out.len()));
            }
        }
    }
    out
}
fn hash_file(p: &Path) -> std::io::Result<(String, u64)> {
    let bytes = std::fs::read(p)?;
    let mut h = Sha256::new();
    h.update(&bytes);
    Ok((hex::encode(h.finalize()), bytes.len() as u64))
}

pub async fn failed_logins(hours: i64) -> anyhow::Result<Value> {
    let since = format!("-{hours} hours");
    let r = run("journalctl", &["--since", &since, "--no-pager"]);
    let lines: Vec<_> = r["stdout"]
        .as_str()
        .unwrap_or("")
        .lines()
        .filter(|l| {
            l.to_lowercase().contains("failed password")
                || l.to_lowercase().contains("authentication failure")
        })
        .take(200)
        .map(|l| json!(l))
        .collect();
    Ok(json!({"hours":hours,"failed_logins":lines,"count":lines.len()}))
}

pub async fn fail2ban_status() -> Value {
    json!({"installed":which("fail2ban-client"),"running":service_active("fail2ban"),"backend":"fail2ban"})
}
pub async fn fail2ban_install() -> Value {
    if which("fail2ban-client") {
        json!({"success":true,"installed":true})
    } else {
        json!({"success":false,"installed":false,"error":"fail2ban is not installed"})
    }
}
pub async fn fail2ban_jail(jail: &str) -> Value {
    if !which("fail2ban-client") {
        return json!({"success":false,"error":"fail2ban-client is not installed"});
    }
    run("fail2ban-client", &["status", jail])
}
pub async fn fail2ban_bans() -> Value {
    if !which("fail2ban-client") {
        return json!({"success":false,"bans":[],"error":"fail2ban-client is not installed"});
    }
    let r = run("fail2ban-client", &["banned"]);
    json!({"success":r["success"],"output":r,"bans":[]})
}
pub async fn fail2ban_ban(ip: &str, jail: &str) -> Value {
    if !which("fail2ban-client") {
        return json!({"success":false,"error":"fail2ban-client is not installed"});
    }
    run("fail2ban-client", &["set", jail, "banip", ip])
}
pub async fn fail2ban_unban(ip: &str, jail: Option<&str>) -> Value {
    if !which("fail2ban-client") {
        return json!({"success":false,"error":"fail2ban-client is not installed"});
    }
    if let Some(j) = jail {
        run("fail2ban-client", &["set", j, "unbanip", ip])
    } else {
        run("fail2ban-client", &["unban", ip])
    }
}

pub async fn ssh_keys(user: &str) -> anyhow::Result<Value> {
    let path = authorized_keys(user);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let keys: Vec<Value> = text
        .lines()
        .enumerate()
        .filter(|(_, l)| !l.trim().is_empty())
        .map(|(i, l)| json!({"id":i.to_string(),"key":l,"user":user}))
        .collect();
    Ok(json!({"user":user,"keys":keys,"path":path}))
}
pub async fn ssh_add_key(user: &str, key: &str) -> anyhow::Result<Value> {
    let path = authorized_keys(user);
    if let Some(p) = Path::new(&path).parent() {
        std::fs::create_dir_all(p)?;
    }
    let mut text = std::fs::read_to_string(&path).unwrap_or_default();
    if !text.contains(key) {
        text.push_str(key);
        text.push('\n');
        std::fs::write(&path, text)?;
    }
    Ok(json!({"success":true,"user":user}))
}
pub async fn ssh_delete_key(user: &str, key_id: &str) -> anyhow::Result<Value> {
    let path = authorized_keys(user);
    let text = std::fs::read_to_string(&path).unwrap_or_default();
    let mut lines: Vec<_> = text.lines().map(str::to_string).collect();
    let idx = key_id.parse::<usize>().ok();
    if let Some(i) = idx.filter(|i| *i < lines.len()) {
        lines.remove(i);
        std::fs::write(&path, format!("{}\n", lines.join("\n")))?;
        Ok(json!({"success":true}))
    } else {
        Ok(json!({"success":false,"error":"key not found"}))
    }
}
fn authorized_keys(user: &str) -> String {
    if user == "root" {
        "/root/.ssh/authorized_keys".into()
    } else {
        format!("/home/{user}/.ssh/authorized_keys")
    }
}

pub async fn ip_lists(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_security_ip_lists ORDER BY list_type,ip")
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"items":rows.iter().map(|r|json!({"list_type":r.get::<String,_>("list_type"),"ip":r.get::<String,_>("ip"),"comment":r.try_get::<Option<String>,_>("comment").ok().flatten(),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
pub async fn ip_list_add(
    pool: &SqlitePool,
    list: &str,
    ip: &str,
    comment: Option<&str>,
) -> anyhow::Result<Value> {
    sqlx::query("INSERT OR REPLACE INTO sk_security_ip_lists(list_type,ip,comment,created_at) VALUES(?,?,?,?)").bind(list).bind(ip).bind(comment).bind(now()).execute(pool).await?;
    Ok(json!({"success":true}))
}
pub async fn ip_list_delete(pool: &SqlitePool, list: &str, ip: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_security_ip_lists WHERE list_type=? AND ip=?").bind(list).bind(ip).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn lynis_status() -> Value {
    json!({"installed":which("lynis"),"last_scan":Value::Null})
}
pub async fn lynis_install() -> Value {
    if which("lynis") {
        json!({"success":true,"installed":true})
    } else {
        json!({"success":false,"installed":false,"error":"lynis is not installed"})
    }
}
pub async fn lynis_scan(pool: &SqlitePool) -> anyhow::Result<Value> {
    if !which("lynis") {
        return Ok(json!({"success":false,"error":"lynis is not installed"}));
    }
    let r = run("lynis", &["audit", "system", "--quick"]);
    event(pool, "lynis", "info", "Lynis scan requested", r.clone()).await?;
    Ok(r)
}
pub async fn auto_updates_status() -> Value {
    json!({"installed":which("unattended-upgrade")||which("unattended-upgrades"),"enabled":service_active("unattended-upgrades")||Path::new("/etc/apt/apt.conf.d/20auto-upgrades").exists()})
}
pub async fn auto_updates_install() -> Value {
    if which("unattended-upgrade") || which("unattended-upgrades") {
        json!({"success":true,"installed":true})
    } else {
        json!({"success":false,"installed":false,"error":"unattended-upgrades is not installed"})
    }
}
pub async fn auto_updates_enable() -> Value {
    if which("systemctl") {
        run("systemctl", &["enable", "--now", "unattended-upgrades"])
    } else {
        json!({"success":false,"error":"systemctl unavailable"})
    }
}
pub async fn auto_updates_disable() -> Value {
    if which("systemctl") {
        run("systemctl", &["disable", "--now", "unattended-upgrades"])
    } else {
        json!({"success":false,"error":"systemctl unavailable"})
    }
}
fn service_active(svc: &str) -> bool {
    std::process::Command::new("systemctl")
        .args(["is-active", "--quiet", svc])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

pub async fn waf_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_waf_policies")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    Ok(
        json!({"installed":which("modsecurity-configure")||Path::new("/etc/modsecurity").exists(),"policies":count,"backend":"modsecurity/nginx"}),
    )
}
pub async fn waf_install() -> Value {
    if Path::new("/etc/modsecurity").exists() {
        json!({"success":true,"installed":true})
    } else {
        json!({"success":false,"installed":false,"error":"ModSecurity is not installed/configured for nginx"})
    }
}
pub async fn waf_policy(pool: &SqlitePool, app: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT policy_json FROM sk_waf_policies WHERE app_id=?")
        .bind(app)
        .fetch_optional(pool)
        .await?;
    Ok(
        json!({"app_id":app,"policy":r.map(|r|j(Some(r.get("policy_json")))).unwrap_or_else(||json!({"enabled":false,"mode":"detection","rules":[]}))}),
    )
}
pub async fn waf_set_policy(pool: &SqlitePool, app: &str, b: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_waf_policies(app_id,policy_json,updated_at) VALUES(?,?,?) ON CONFLICT(app_id) DO UPDATE SET policy_json=excluded.policy_json,updated_at=excluded.updated_at").bind(app).bind(b.to_string()).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"app_id":app,"policy":b}))
}
pub async fn waf_apply(pool: &SqlitePool, app: &str) -> anyhow::Result<Value> {
    let pol = waf_policy(pool, app).await?;
    let eid = id();
    let ev = json!({"action":"apply","policy":pol["policy"],"installed":Path::new("/etc/modsecurity").exists()});
    sqlx::query("INSERT INTO sk_waf_events(id,app_id,event_json,created_at) VALUES(?,?,?,?)")
        .bind(&eid)
        .bind(app)
        .bind(ev.to_string())
        .bind(now())
        .execute(pool)
        .await?;
    Ok(
        json!({"success":true,"app_id":app,"event_id":eid,"provider_applied":ev["installed"].as_bool().unwrap_or(false),"message":"WAF policy desired state recorded"}),
    )
}
pub async fn waf_events(pool: &SqlitePool, app: &str, limit: i64) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT * FROM sk_waf_events WHERE app_id=? ORDER BY created_at DESC LIMIT ?")
            .bind(app)
            .bind(limit.clamp(1, 500))
            .fetch_all(pool)
            .await?;
    Ok(
        json!({"events":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"app_id":r.get::<String,_>("app_id"),"event":j(Some(r.get::<String,_>("event_json"))),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}
