use anyhow::Context;
use chrono::{DateTime, NaiveDateTime, Utc};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn cert_root() -> PathBuf {
    PathBuf::from(std::env::var("SK_CERT_DIR").unwrap_or_else(|_| "/etc/serverkit/certs".into()))
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn clean(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '.' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_ssl_certificates(domain TEXT PRIMARY KEY, domains_json TEXT NOT NULL, cert_path TEXT NOT NULL, key_path TEXT NOT NULL, chain_path TEXT, issuer TEXT, not_after TEXT, source TEXT NOT NULL, auto_renew INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL, metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_ssl_settings(id INTEGER PRIMARY KEY CHECK(id=1), auto_renew INTEGER NOT NULL DEFAULT 1, email TEXT, webroot TEXT NOT NULL DEFAULT '/var/www/letsencrypt', updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-web ssl schema")?;
    Ok(())
}

fn cert_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"domain":r.get::<String,_>("domain"),"domains":j(Some(r.get::<String,_>("domains_json"))),"cert_path":r.get::<String,_>("cert_path"),"key_path":r.get::<String,_>("key_path"),"chain_path":r.try_get::<Option<String>,_>("chain_path").ok().flatten(),"issuer":r.try_get::<Option<String>,_>("issuer").ok().flatten(),"not_after":r.try_get::<Option<String>,_>("not_after").ok().flatten(),"source":r.get::<String,_>("source"),"auto_renew":r.get::<i64,_>("auto_renew")!=0,"status":r.get::<String,_>("status"),"metadata":j(Some(r.get::<String,_>("metadata_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}

pub async fn status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let certs = list(pool).await?;
    let total = certs["certificates"].as_array().map(Vec::len).unwrap_or(0);
    let expiring = expiry_alerts(pool, 30).await?["alerts"]
        .as_array()
        .map(Vec::len)
        .unwrap_or(0);
    Ok(
        json!({"certificates":total,"total":total,"expiring_soon":expiring,"rust_acme":true,"certbot_installed":which("certbot"),"openssl_installed":which("openssl"),"auto_renewal":settings(pool).await?}),
    )
}
pub async fn list(pool: &SqlitePool) -> anyhow::Result<Value> {
    sync_letsencrypt(pool).await?;
    let rows = sqlx::query("SELECT * FROM sk_ssl_certificates ORDER BY domain")
        .fetch_all(pool)
        .await?;
    Ok(json!({"certificates":rows.iter().map(cert_value).collect::<Vec<_>>() }))
}
pub fn profiles() -> Value {
    json!({"profiles":[{"key":"self-signed","name":"Self-signed local certificate","requires_public_dns":false},{"key":"http-01","name":"Let's Encrypt HTTP-01","requires_public_dns":true},{"key":"dns-01-cloudflare","name":"Let's Encrypt DNS-01 via Cloudflare","requires_public_dns":false}]})
}
pub async fn settings(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_ssl_settings WHERE id=1")
        .fetch_optional(pool)
        .await?;
    Ok(match r {
        Some(r) => {
            json!({"auto_renew":r.get::<i64,_>("auto_renew")!=0,"email":r.try_get::<Option<String>,_>("email").ok().flatten(),"webroot":r.get::<String,_>("webroot"),"updated_at":r.get::<String,_>("updated_at")})
        }
        None => json!({"auto_renew":true,"email":Value::Null,"webroot":"/var/www/letsencrypt"}),
    })
}
pub async fn setup_auto_renewal(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_ssl_settings(id,auto_renew,email,webroot,updated_at) VALUES(1,?,?,?,?) ON CONFLICT(id) DO UPDATE SET auto_renew=excluded.auto_renew,email=excluded.email,webroot=excluded.webroot,updated_at=excluded.updated_at").bind(if b.get("auto_renew").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind(opt(b,"email")).bind(opt(b,"webroot").unwrap_or("/var/www/letsencrypt")).bind(now()).execute(pool).await?;
    Ok(json!({"success":true,"settings":settings(pool).await?}))
}
pub async fn install_certbot() -> anyhow::Result<Value> {
    Ok(if which("certbot") {
        json!({"success":true,"installed":true,"message":"certbot is installed"})
    } else {
        json!({"success":false,"installed":false,"error":"certbot is not installed; ServerKit-rs uses the built-in Rust ACME client for certificate issuance"})
    })
}

pub async fn obtain(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let domains = domains_from_body(b);
    if domains.is_empty() {
        return Ok(json!({"success":false,"error":"domain/domains are required"}));
    }
    let profile = s(
        b,
        "profile",
        s(
            b,
            "type",
            if b.get("self_signed")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                "self-signed"
            } else {
                "http-01"
            },
        ),
    );
    match profile {
        "self-signed" => self_signed(pool, &domains).await,
        "http-01" => {
            let email = s(b, "email", "");
            let webroot = s(b, "webroot", "/var/www/letsencrypt");
            if email.is_empty() {
                return Ok(json!({"success":false,"error":"email is required for HTTP-01 ACME"}));
            }
            issue_acme(
                pool,
                &domains,
                email,
                sk_acme::Challenge::Http01 {
                    webroot: webroot.into(),
                },
                b,
            )
            .await
        }
        "dns-01" | "dns-01-cloudflare" => {
            let email = s(b, "email", "");
            if email.is_empty() {
                return Ok(json!({"success":false,"error":"email is required for DNS-01 ACME"}));
            }
            if std::env::var("SK_CF_API_TOKEN").is_err() {
                return Ok(
                    json!({"success":false,"configured":false,"error":"SK_CF_API_TOKEN is required for Cloudflare DNS-01"}),
                );
            }
            issue_acme(pool, &domains, email, sk_acme::Challenge::Dns01, b).await
        }
        other => Ok(json!({"success":false,"error":format!("unknown SSL profile: {other}")})),
    }
}
pub async fn wildcard(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let domain = s(b, "domain", "").trim_start_matches("*.");
    if domain.is_empty() {
        return Ok(json!({"success":false,"error":"domain is required"}));
    }
    let mut x = b.clone();
    x["domains"] = json!([format!("*.{domain}"), domain]);
    x["profile"] = json!("dns-01-cloudflare");
    obtain(pool, &x).await
}
pub async fn san(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let mut x = b.clone();
    if x.get("profile").is_none() {
        x["profile"] = json!("http-01");
    }
    obtain(pool, &x).await
}
pub async fn upload(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let domain = s(b, "domain", "");
    let cert = s(b, "certificate", "");
    let key = s(b, "private_key", s(b, "key", ""));
    if domain.is_empty() || cert.is_empty() || key.is_empty() {
        return Ok(
            json!({"success":false,"error":"domain, certificate and private_key are required"}),
        );
    }
    write_and_record(
        pool,
        &[domain.to_string()],
        cert,
        key,
        opt(b, "chain"),
        "uploaded",
        json!({"uploaded":true}),
    )
    .await
}
pub async fn renew(pool: &SqlitePool, domain: &str) -> anyhow::Result<Value> {
    let rec = sqlx::query("SELECT * FROM sk_ssl_certificates WHERE domain=?")
        .bind(domain)
        .fetch_optional(pool)
        .await?;
    let Some(r) = rec else {
        return Ok(json!({"success":false,"error":"certificate not found"}));
    };
    let source: String = r.get("source");
    let domains: Vec<String> = j(Some(r.get::<String, _>("domains_json")))
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    if source == "self-signed" {
        self_signed(pool, &domains).await
    } else if source == "acme-http-01" {
        let meta = j(Some(r.get::<String, _>("metadata_json")));
        let email = s(&meta, "email", "");
        let webroot = s(&meta, "webroot", "/var/www/letsencrypt");
        if email.is_empty() {
            Ok(json!({"success":false,"error":"stored ACME email missing"}))
        } else {
            issue_acme(
                pool,
                &domains,
                email,
                sk_acme::Challenge::Http01 {
                    webroot: webroot.into(),
                },
                &meta,
            )
            .await
        }
    } else if source == "acme-dns-01" {
        if std::env::var("SK_CF_API_TOKEN").is_err() {
            Ok(
                json!({"success":false,"configured":false,"error":"SK_CF_API_TOKEN is required for Cloudflare DNS-01 renewal"}),
            )
        } else {
            let meta = j(Some(r.get::<String, _>("metadata_json")));
            let email = s(&meta, "email", "");
            issue_acme(pool, &domains, email, sk_acme::Challenge::Dns01, &meta).await
        }
    } else {
        Ok(json!({"success":false,"error":"uploaded certificates cannot be renewed automatically"}))
    }
}
pub async fn renew_all(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT domain FROM sk_ssl_certificates")
        .fetch_all(pool)
        .await?;
    let mut results = Vec::new();
    for r in rows {
        let d: String = r.get("domain");
        results.push(renew(pool, &d).await?);
    }
    Ok(json!({"success":true,"results":results}))
}
pub async fn delete(pool: &SqlitePool, domain: &str) -> anyhow::Result<Value> {
    let rec = sqlx::query("SELECT * FROM sk_ssl_certificates WHERE domain=?")
        .bind(domain)
        .fetch_optional(pool)
        .await?;
    if let Some(r) = rec {
        for col in ["cert_path", "key_path", "chain_path"] {
            if let Ok(Some(p)) = r.try_get::<Option<String>, _>(col) {
                let _ = std::fs::remove_file(p);
            }
        }
    }
    let n = sqlx::query("DELETE FROM sk_ssl_certificates WHERE domain=?")
        .bind(domain)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0}))
}
pub async fn health(pool: &SqlitePool, domain: &str) -> anyhow::Result<Value> {
    let rec = sqlx::query("SELECT * FROM sk_ssl_certificates WHERE domain=?")
        .bind(domain)
        .fetch_optional(pool)
        .await?;
    let Some(r) = rec else {
        return Ok(json!({"success":false,"error":"certificate not found"}));
    };
    let cert = cert_value(&r);
    let exists = Path::new(cert["cert_path"].as_str().unwrap_or("")).exists()
        && Path::new(cert["key_path"].as_str().unwrap_or("")).exists();
    let days = days_until(cert["not_after"].as_str());
    Ok(
        json!({"success":exists,"domain":domain,"exists":exists,"days_remaining":days,"certificate":cert}),
    )
}
pub async fn expiry_alerts(pool: &SqlitePool, days: i64) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_ssl_certificates")
        .fetch_all(pool)
        .await?;
    let alerts: Vec<Value> = rows
        .iter()
        .map(cert_value)
        .filter(|c| {
            days_until(c["not_after"].as_str())
                .map(|d| d <= days)
                .unwrap_or(true)
        })
        .collect();
    Ok(json!({"days":days,"alerts":alerts}))
}

async fn self_signed(pool: &SqlitePool, domains: &[String]) -> anyhow::Result<Value> {
    let domain = &domains[0];
    let dir = cert_root().join(clean(domain));
    std::fs::create_dir_all(&dir)?;
    let cert = dir.join("cert.pem");
    let key = dir.join("key.pem");
    let subj = format!("/CN={domain}");
    let out = tokio::process::Command::new("openssl")
        .args([
            "req",
            "-x509",
            "-newkey",
            "rsa:2048",
            "-nodes",
            "-days",
            "365",
            "-subj",
            &subj,
            "-keyout",
            key.to_str().unwrap(),
            "-out",
            cert.to_str().unwrap(),
        ])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            let meta = inspect_cert(&cert);
            write_record(
                pool,
                domains,
                cert.to_string_lossy().as_ref(),
                key.to_string_lossy().as_ref(),
                None,
                "self-signed",
                meta,
            )
            .await
        }
        Ok(o) => Ok(json!({"success":false,"error":String::from_utf8_lossy(&o.stderr).trim()})),
        Err(e) => Ok(json!({"success":false,"error":e.to_string()})),
    }
}
async fn issue_acme(
    pool: &SqlitePool,
    domains: &[String],
    email: &str,
    challenge: sk_acme::Challenge,
    body: &Value,
) -> anyhow::Result<Value> {
    match sk_acme::issue(
        domains,
        email,
        challenge,
        body.get("staging")
            .and_then(Value::as_bool)
            .unwrap_or(false),
    )
    .await
    {
        Ok(issued) => write_and_record(
            pool,
            domains,
            &issued.cert_pem,
            &issued.key_pem,
            None,
            if body
                .get("profile")
                .and_then(Value::as_str)
                .unwrap_or("")
                .contains("dns")
            {
                "acme-dns-01"
            } else {
                "acme-http-01"
            },
            json!({"email":email,"webroot":opt(body,"webroot").unwrap_or("/var/www/letsencrypt")}),
        )
        .await,
        Err(e) => Ok(json!({"success":false,"error":e.to_string()})),
    }
}
async fn write_and_record(
    pool: &SqlitePool,
    domains: &[String],
    cert: &str,
    key: &str,
    chain: Option<&str>,
    source: &str,
    metadata: Value,
) -> anyhow::Result<Value> {
    let domain = &domains[0];
    let dir = cert_root().join(clean(domain));
    std::fs::create_dir_all(&dir)?;
    let cert_path = dir.join("cert.pem");
    let key_path = dir.join("key.pem");
    std::fs::write(&cert_path, cert)?;
    std::fs::write(&key_path, key)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&key_path, std::fs::Permissions::from_mode(0o600));
    }
    let chain_path = if let Some(c) = chain {
        let p = dir.join("chain.pem");
        std::fs::write(&p, c)?;
        Some(p.to_string_lossy().to_string())
    } else {
        None
    };
    let meta = inspect_cert(&cert_path);
    write_record(
        pool,
        domains,
        cert_path.to_string_lossy().as_ref(),
        key_path.to_string_lossy().as_ref(),
        chain_path.as_deref(),
        source,
        merge(metadata, meta),
    )
    .await
}
async fn write_record(
    pool: &SqlitePool,
    domains: &[String],
    cert_path: &str,
    key_path: &str,
    chain_path: Option<&str>,
    source: &str,
    metadata: Value,
) -> anyhow::Result<Value> {
    let ts = now();
    let not_after = metadata.get("not_after").and_then(Value::as_str);
    let issuer = metadata.get("issuer").and_then(Value::as_str);
    sqlx::query("INSERT INTO sk_ssl_certificates(domain,domains_json,cert_path,key_path,chain_path,issuer,not_after,source,auto_renew,status,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(domain) DO UPDATE SET domains_json=excluded.domains_json,cert_path=excluded.cert_path,key_path=excluded.key_path,chain_path=excluded.chain_path,issuer=excluded.issuer,not_after=excluded.not_after,source=excluded.source,auto_renew=excluded.auto_renew,status=excluded.status,metadata_json=excluded.metadata_json,updated_at=excluded.updated_at").bind(&domains[0]).bind(json!(domains).to_string()).bind(cert_path).bind(key_path).bind(chain_path).bind(issuer).bind(not_after).bind(source).bind(if source.starts_with("acme"){1}else{0}).bind("active").bind(metadata.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_ssl_certificates WHERE domain=?")
        .bind(&domains[0])
        .fetch_one(pool)
        .await?;
    Ok(json!({"success":true,"certificate":cert_value(&row)}))
}
fn inspect_cert(path: &Path) -> Value {
    let out = std::process::Command::new("openssl")
        .args([
            "x509",
            "-in",
            path.to_str().unwrap_or(""),
            "-noout",
            "-issuer",
            "-enddate",
        ])
        .output();
    let Ok(o) = out else {
        return json!({});
    };
    let text = String::from_utf8_lossy(&o.stdout);
    let issuer = text
        .lines()
        .find_map(|l| l.strip_prefix("issuer="))
        .map(str::trim)
        .map(str::to_string);
    let not_after_raw = text
        .lines()
        .find_map(|l| l.strip_prefix("notAfter="))
        .map(str::trim)
        .map(str::to_string);
    json!({"issuer":issuer,"not_after_raw":not_after_raw,"not_after":not_after_raw.as_deref().and_then(parse_openssl_date)})
}
fn parse_openssl_date(s: &str) -> Option<String> {
    NaiveDateTime::parse_from_str(s, "%b %e %H:%M:%S %Y GMT")
        .or_else(|_| NaiveDateTime::parse_from_str(s, "%b %d %H:%M:%S %Y GMT"))
        .ok()
        .map(|d| d.and_utc().to_rfc3339())
}
fn days_until(iso: Option<&str>) -> Option<i64> {
    let dt = DateTime::parse_from_rfc3339(iso?).ok()?.with_timezone(&Utc);
    Some((dt - Utc::now()).num_days())
}
fn domains_from_body(b: &Value) -> Vec<String> {
    if let Some(a) = b.get("domains").and_then(Value::as_array) {
        a.iter()
            .filter_map(|v| v.as_str().map(|s| s.to_string()))
            .collect()
    } else {
        opt(b, "domain")
            .map(|s| vec![s.to_string()])
            .unwrap_or_default()
    }
}
fn which(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}
fn merge(mut a: Value, b: Value) -> Value {
    if let (Some(ao), Some(bo)) = (a.as_object_mut(), b.as_object()) {
        for (k, v) in bo {
            ao.insert(k.clone(), v.clone());
        }
    }
    a
}
async fn sync_letsencrypt(_pool: &SqlitePool) -> anyhow::Result<()> {
    Ok(())
}
