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
fn opt<'a>(v: &'a Value, k: &str) -> Option<&'a str> {
    v.get(k).and_then(Value::as_str)
}
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
fn clean_name(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.' {
                c
            } else {
                '-'
            }
        })
        .collect()
}
fn valid_domain(domain: &str) -> bool {
    domain == "localhost"
        || regex::Regex::new(r"^([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$")
            .unwrap()
            .is_match(domain)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_domains(id TEXT PRIMARY KEY, domain TEXT NOT NULL UNIQUE, app_id TEXT, nginx_site TEXT, root_path TEXT, target_port INTEGER, ssl_enabled INTEGER NOT NULL DEFAULT 0, status TEXT NOT NULL DEFAULT 'active', verification_json TEXT NOT NULL DEFAULT '{}', metadata_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_base_domains(domain TEXT PRIMARY KEY, dns_mode TEXT NOT NULL DEFAULT 'wildcard', is_default INTEGER NOT NULL DEFAULT 0, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-web domains schema")?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sk_base_domains")
        .fetch_one(pool)
        .await
        .unwrap_or(0);
    if count == 0 {
        let ts = now();
        sqlx::query("INSERT OR IGNORE INTO sk_base_domains(domain,dns_mode,is_default,created_at,updated_at) VALUES(?,?,?,?,?)")
            .bind("serverkit.local").bind("wildcard").bind(1).bind(&ts).bind(&ts).execute(pool).await?;
    }
    Ok(())
}

fn domain_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"domain":r.get::<String,_>("domain"),"name":r.get::<String,_>("domain"),"app_id":r.try_get::<Option<String>,_>("app_id").ok().flatten(),"application_id":r.try_get::<Option<String>,_>("app_id").ok().flatten(),"nginx_site":r.try_get::<Option<String>,_>("nginx_site").ok().flatten(),"root_path":r.try_get::<Option<String>,_>("root_path").ok().flatten(),"target_port":r.try_get::<Option<i64>,_>("target_port").ok().flatten(),"ssl_enabled":r.get::<i64,_>("ssl_enabled")!=0,"ssl":r.get::<i64,_>("ssl_enabled")!=0,"status":r.get::<String,_>("status"),"verification":j(Some(r.get::<String,_>("verification_json"))),"metadata":j(Some(r.get::<String,_>("metadata_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn base_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"domain":r.get::<String,_>("domain"),"dns_mode":r.get::<String,_>("dns_mode"),"is_default":r.get::<i64,_>("is_default")!=0,"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}

pub async fn list(pool: &SqlitePool) -> anyhow::Result<Value> {
    sync_nginx_sites(pool).await?;
    let rows = sqlx::query("SELECT * FROM sk_domains ORDER BY domain")
        .fetch_all(pool)
        .await?;
    Ok(json!({"domains":rows.iter().map(domain_value).collect::<Vec<_>>() }))
}
pub async fn get(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Option<Value>> {
    sync_nginx_sites(pool).await?;
    let r = sqlx::query("SELECT * FROM sk_domains WHERE id=? OR domain=?")
        .bind(id_or_domain)
        .bind(id_or_domain)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(domain_value))
}

pub async fn create(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let domain = s(b, "domain", s(b, "name", ""))
        .trim()
        .trim_end_matches('.')
        .to_lowercase();
    if domain.is_empty() {
        return Ok(json!({"success":false,"error":"domain is required"}));
    }
    if !valid_domain(&domain) {
        return Ok(json!({"success":false,"error":"invalid domain"}));
    }
    let id = id();
    let ts = now();
    let app_id = opt(b, "application_id").or_else(|| opt(b, "app_id"));
    let root = opt(b, "root_path").or_else(|| opt(b, "root"));
    let nginx_site = opt(b, "nginx_site")
        .map(str::to_string)
        .unwrap_or_else(|| clean_name(&domain));
    let port = b
        .get("port")
        .or_else(|| b.get("target_port"))
        .and_then(Value::as_i64);
    sqlx::query("INSERT INTO sk_domains(id,domain,app_id,nginx_site,root_path,target_port,ssl_enabled,status,verification_json,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(domain) DO UPDATE SET app_id=excluded.app_id, nginx_site=excluded.nginx_site, root_path=excluded.root_path, target_port=excluded.target_port, metadata_json=excluded.metadata_json, updated_at=excluded.updated_at")
        .bind(&id).bind(&domain).bind(app_id).bind(&nginx_site).bind(root).bind(port).bind(0).bind("active").bind(json!({}).to_string()).bind(b.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    if root.is_some() || port.is_some() {
        let spec = crate::nginx::SiteSpec {
            name: nginx_site.clone(),
            app_type: s(
                b,
                "app_type",
                if port.is_some() { "docker" } else { "static" },
            )
            .to_string(),
            domains: vec![domain.clone()],
            root_path: root.unwrap_or("/var/www/html").to_string(),
            port: port.map(|p| p as u16),
            php_version: s(b, "php_version", "8.2").to_string(),
            ssl_cert: None,
            ssl_key: None,
        };
        let created = crate::nginx::create_site(&spec).await;
        if !created["success"].as_bool().unwrap_or(false) {
            return Ok(
                json!({"success":false,"error":"domain persisted but nginx site creation failed","nginx":created}),
            );
        }
    }
    Ok(get(pool, &domain).await?.unwrap())
}

pub async fn update(pool: &SqlitePool, id_or_domain: &str, b: &Value) -> anyhow::Result<Value> {
    let Some(old) = get(pool, id_or_domain).await? else {
        return Ok(json!({"success":false,"error":"domain not found"}));
    };
    let domain = opt(b, "domain").unwrap_or_else(|| old["domain"].as_str().unwrap_or(""));
    if !valid_domain(domain) {
        return Ok(json!({"success":false,"error":"invalid domain"}));
    }
    sqlx::query("UPDATE sk_domains SET domain=?, app_id=?, nginx_site=?, root_path=?, target_port=?, metadata_json=?, updated_at=? WHERE id=? OR domain=?")
        .bind(domain).bind(opt(b,"application_id").or_else(||opt(b,"app_id")).or_else(||old["app_id"].as_str()))
        .bind(opt(b,"nginx_site").or_else(||old["nginx_site"].as_str()))
        .bind(opt(b,"root_path").or_else(||old["root_path"].as_str()))
        .bind(b.get("target_port").or_else(||b.get("port")).and_then(Value::as_i64).or_else(||old["target_port"].as_i64()))
        .bind(b.to_string()).bind(now()).bind(id_or_domain).bind(id_or_domain).execute(pool).await?;
    get(pool, domain).await?.context("updated domain missing")
}

pub async fn delete(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Value> {
    let old = get(pool, id_or_domain).await?;
    if let Some(site) = old.as_ref().and_then(|d| d["nginx_site"].as_str()) {
        let _ = crate::nginx::delete_site(site).await;
    }
    let n = sqlx::query("DELETE FROM sk_domains WHERE id=? OR domain=?")
        .bind(id_or_domain)
        .bind(id_or_domain)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0}))
}

pub async fn verify(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Value> {
    let Some(d) = get(pool, id_or_domain).await? else {
        return Ok(json!({"success":false,"error":"domain not found"}));
    };
    let domain = d["domain"].as_str().unwrap_or("");
    let dns = crate::nginx::resolve_domain(domain).await;
    let nginx_site = d["nginx_site"]
        .as_str()
        .and_then(crate::nginx::site_by_name);
    let ok = nginx_site.is_some();
    let verification = json!({"dns":dns,"nginx_configured":ok,"checked_at":now()});
    sqlx::query("UPDATE sk_domains SET verification_json=?, status=?, updated_at=? WHERE id=?")
        .bind(verification.to_string())
        .bind(if ok { "verified" } else { "pending" })
        .bind(now())
        .bind(d["id"].as_str())
        .execute(pool)
        .await?;
    Ok(json!({"success":ok,"domain":domain,"verification":verification}))
}

pub async fn enable_ssl(pool: &SqlitePool, id_or_domain: &str, b: &Value) -> anyhow::Result<Value> {
    let Some(d) = get(pool, id_or_domain).await? else {
        return Ok(json!({"success":false,"error":"domain not found"}));
    };
    let site = d["nginx_site"]
        .as_str()
        .unwrap_or(d["domain"].as_str().unwrap_or(""));
    let cert =
        opt(b, "cert_path").unwrap_or_else(|| d["metadata"]["cert_path"].as_str().unwrap_or(""));
    let key =
        opt(b, "key_path").unwrap_or_else(|| d["metadata"]["key_path"].as_str().unwrap_or(""));
    if cert.is_empty() || key.is_empty() {
        return Ok(
            json!({"success":false,"error":"cert_path and key_path are required unless certificate automation is configured"}),
        );
    }
    let r = crate::nginx::add_ssl_to_site(site, cert, key).await;
    if r["success"].as_bool().unwrap_or(false) {
        sqlx::query("UPDATE sk_domains SET ssl_enabled=1, updated_at=? WHERE id=?")
            .bind(now())
            .bind(d["id"].as_str())
            .execute(pool)
            .await?;
    }
    Ok(r)
}
pub async fn disable_ssl(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Value> {
    let Some(d) = get(pool, id_or_domain).await? else {
        return Ok(json!({"success":false,"error":"domain not found"}));
    };
    sqlx::query("UPDATE sk_domains SET ssl_enabled=0, updated_at=? WHERE id=?")
        .bind(now())
        .bind(d["id"].as_str())
        .execute(pool)
        .await?;
    Ok(
        json!({"success":true,"message":"SSL disabled in domain metadata; remove certificate directives from nginx site config manually or recreate the site"}),
    )
}
pub async fn renew_ssl(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Value> {
    let Some(d) = get(pool, id_or_domain).await? else {
        return Ok(json!({"success":false,"error":"domain not found"}));
    };
    Ok(
        json!({"success":false,"domain":d["domain"],"error":"Automated certificate renewal is not configured for this domain; use /ssl certificate routes with ACME configuration"}),
    )
}

pub async fn base_domains(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_base_domains ORDER BY is_default DESC, domain")
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"base_domains":rows.iter().map(base_value).collect::<Vec<_>>(),"domains":rows.iter().map(base_value).collect::<Vec<_>>() }),
    )
}
pub async fn suggest_subdomain(
    pool: &SqlitePool,
    app_id: &str,
    base: Option<&str>,
) -> anyhow::Result<Value> {
    let bases = base_domains(pool).await?;
    let base = base
        .map(str::to_string)
        .or_else(|| {
            bases["base_domains"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|b| b["domain"].as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "serverkit.local".into());
    let label = app_label(pool, app_id)
        .await
        .unwrap_or_else(|| format!("app-{}", &app_id.chars().take(8).collect::<String>()));
    let host = format!("{}.{}", clean_name(&label).trim_matches('-'), base);
    Ok(
        json!({"application_id":app_id,"label":clean_name(&label),"base":base,"domain":host,"available":get(pool,&host).await?.is_none()}),
    )
}
pub async fn give_subdomain(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let app_id = s(b, "application_id", s(b, "app_id", "")).to_string();
    if app_id.is_empty() {
        return Ok(json!({"success":false,"error":"application_id is required"}));
    }
    let suggested = suggest_subdomain(pool, &app_id, opt(b, "base")).await?;
    let label = opt(b, "label").unwrap_or_else(|| suggested["label"].as_str().unwrap_or("app"));
    let base =
        opt(b, "base").unwrap_or_else(|| suggested["base"].as_str().unwrap_or("serverkit.local"));
    let domain = format!("{}.{}", clean_name(label).trim_matches('-'), base);
    let app = app_metadata(pool, &app_id).await.unwrap_or(Value::Null);
    let root = app["root_path"].as_str().unwrap_or("/var/www/html");
    let app_type = app["app_type"].as_str().unwrap_or("static");
    create(
        pool,
        &json!({"domain":domain,"application_id":app_id,"root_path":root,"app_type":app_type}),
    )
    .await
}

pub async fn nginx_sites() -> anyhow::Result<Value> {
    Ok(json!({"sites":crate::nginx::list_sites()}))
}
pub async fn ssl_status(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_domains ORDER BY domain")
        .fetch_all(pool)
        .await?;
    let domains: Vec<Value> = rows.iter().map(domain_value).collect();
    let enabled = domains
        .iter()
        .filter(|d| d["ssl_enabled"].as_bool() == Some(true))
        .count();
    Ok(json!({"total":domains.len(),"enabled":enabled,"domains":domains}))
}

async fn app_metadata(pool: &SqlitePool, app_id: &str) -> Option<Value> {
    let row = sqlx::query("SELECT id,name,app_type,root_path FROM sk_apps WHERE id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await
        .ok()??;
    Some(json!({
        "id": row.get::<String, _>("id"),
        "name": row.get::<String, _>("name"),
        "app_type": row.get::<String, _>("app_type"),
        "root_path": row.try_get::<Option<String>, _>("root_path").ok().flatten(),
    }))
}

async fn app_label(pool: &SqlitePool, app_id: &str) -> Option<String> {
    app_metadata(pool, app_id)
        .await
        .and_then(|a| a["name"].as_str().map(str::to_string))
}
async fn sync_nginx_sites(pool: &SqlitePool) -> anyhow::Result<()> {
    let ts = now();
    for site in crate::nginx::list_sites() {
        let domains = site["domains"].as_array().cloned().unwrap_or_default();
        for d in domains.iter().filter_map(Value::as_str) {
            if d == "_" {
                continue;
            }
            sqlx::query("INSERT INTO sk_domains(id,domain,nginx_site,root_path,ssl_enabled,status,verification_json,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(domain) DO UPDATE SET nginx_site=excluded.nginx_site, root_path=excluded.root_path, ssl_enabled=excluded.ssl_enabled, updated_at=excluded.updated_at").bind(id()).bind(d).bind(site["name"].as_str()).bind(site["root"].as_str()).bind(if site["ssl"].as_bool().unwrap_or(false){1}else{0}).bind("active").bind(json!({}).to_string()).bind(json!({"source":"nginx"}).to_string()).bind(&ts).bind(&ts).execute(pool).await?;
        }
    }
    Ok(())
}
