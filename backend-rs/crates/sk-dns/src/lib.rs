use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use uuid::Uuid;

fn id() -> String {
    Uuid::new_v4().to_string()
}
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn token() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(40)
        .map(char::from)
        .collect()
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
fn encrypt_json(v: &Value) -> String {
    sk_core::crypto::encrypt(&v.to_string())
}
fn decrypt_json(s: &str) -> Value {
    sk_core::crypto::decrypt(s)
        .and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_dns_zones(id TEXT PRIMARY KEY, domain TEXT NOT NULL UNIQUE, provider_config_id TEXT, provider_zone_id TEXT, managed INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL DEFAULT 'active', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_dns_records(id TEXT PRIMARY KEY, zone_id TEXT NOT NULL, type TEXT NOT NULL, name TEXT NOT NULL, value TEXT NOT NULL, ttl INTEGER NOT NULL DEFAULT 300, priority INTEGER, proxied INTEGER NOT NULL DEFAULT 0, source TEXT NOT NULL DEFAULT 'manual', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_dns_changes(id TEXT PRIMARY KEY, config_id TEXT, zone_id TEXT, action TEXT NOT NULL, result TEXT NOT NULL, message TEXT, created_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_ddns_hosts(id TEXT PRIMARY KEY, hostname TEXT NOT NULL UNIQUE, zone_id TEXT, record_id TEXT, token_encrypted TEXT NOT NULL, current_ip TEXT, last_update_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_registrar_connections(id TEXT PRIMARY KEY, provider TEXT NOT NULL, name TEXT NOT NULL, config_encrypted TEXT NOT NULL, status TEXT NOT NULL DEFAULT 'configured', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_registrar_domains(id TEXT PRIMARY KEY, connection_id TEXT, domain TEXT NOT NULL, registrar TEXT, expires_at TEXT, status TEXT, raw_json TEXT, updated_at TEXT NOT NULL, UNIQUE(connection_id,domain));
"#).execute(pool).await.context("ensure sk-dns schema")?;
    Ok(())
}

fn zone_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"domain":r.get::<String,_>("domain"),"name":r.get::<String,_>("domain"),"provider_config_id":r.try_get::<Option<String>,_>("provider_config_id").ok().flatten(),"provider_zone_id":r.try_get::<Option<String>,_>("provider_zone_id").ok().flatten(),"managed":r.get::<i64,_>("managed")!=0,"status":r.get::<String,_>("status"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn record_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"zone_id":r.get::<String,_>("zone_id"),"type":r.get::<String,_>("type"),"name":r.get::<String,_>("name"),"value":r.get::<String,_>("value"),"content":r.get::<String,_>("value"),"ttl":r.get::<i64,_>("ttl"),"priority":r.try_get::<Option<i64>,_>("priority").ok().flatten(),"proxied":r.get::<i64,_>("proxied")!=0,"source":r.get::<String,_>("source"),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn ddns_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"hostname":r.get::<String,_>("hostname"),"zone_id":r.try_get::<Option<String>,_>("zone_id").ok().flatten(),"record_id":r.try_get::<Option<String>,_>("record_id").ok().flatten(),"current_ip":r.try_get::<Option<String>,_>("current_ip").ok().flatten(),"last_update_at":r.try_get::<Option<String>,_>("last_update_at").ok().flatten(),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn registrar_connection_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    let cfg = decrypt_json(&r.get::<String, _>("config_encrypted"));
    json!({"id":r.get::<String,_>("id"),"provider":r.get::<String,_>("provider"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"has_secret":cfg.get("api_key").is_some()||cfg.get("token").is_some()||cfg.get("secret").is_some(),"config":redact(cfg),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
fn registrar_domain_value(r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"connection_id":r.try_get::<Option<String>,_>("connection_id").ok().flatten(),"domain":r.get::<String,_>("domain"),"registrar":r.try_get::<Option<String>,_>("registrar").ok().flatten(),"expires_at":r.try_get::<Option<String>,_>("expires_at").ok().flatten(),"status":r.try_get::<Option<String>,_>("status").ok().flatten(),"raw":j(r.try_get::<Option<String>,_>("raw_json").ok().flatten()),"updated_at":r.get::<String,_>("updated_at")})
}
fn redact(mut v: Value) -> Value {
    if let Some(o) = v.as_object_mut() {
        for k in ["api_key", "token", "secret", "password"] {
            if o.contains_key(k) {
                o.insert(k.into(), json!("***"));
            }
        }
    }
    v
}

pub async fn zones(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_dns_zones ORDER BY domain")
        .fetch_all(pool)
        .await?;
    Ok(json!({"zones":rows.iter().map(zone_value).collect::<Vec<_>>() }))
}
pub async fn get_zone(pool: &SqlitePool, id_or_domain: &str) -> anyhow::Result<Option<Value>> {
    let r = sqlx::query("SELECT * FROM sk_dns_zones WHERE id=? OR domain=?")
        .bind(id_or_domain)
        .bind(id_or_domain)
        .fetch_optional(pool)
        .await?;
    Ok(r.as_ref().map(zone_value))
}
pub async fn create_zone(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let domain = s(b, "domain", s(b, "name", ""))
        .trim()
        .trim_end_matches('.')
        .to_lowercase();
    if domain.is_empty() {
        return Ok(json!({"success":false,"error":"domain is required"}));
    }
    let ts = now();
    let existing = sqlx::query("SELECT * FROM sk_dns_zones WHERE domain=?")
        .bind(&domain)
        .fetch_optional(pool)
        .await?;
    if let Some(r) = existing {
        return Ok(zone_value(&r));
    }
    let id = id();
    sqlx::query("INSERT INTO sk_dns_zones(id,domain,provider_config_id,provider_zone_id,managed,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(&id).bind(&domain).bind(opt(b,"dns_provider_config_id").or_else(||opt(b,"provider_config_id"))).bind(opt(b,"provider_zone_id")).bind(if b.get("managed").and_then(Value::as_bool).unwrap_or(true){1}else{0}).bind("active").bind(&ts).bind(&ts).execute(pool).await?;
    log_change(
        pool,
        opt(b, "dns_provider_config_id"),
        Some(&id),
        "zone.create",
        "success",
        Some(&domain),
    )
    .await?;
    Ok(get_zone(pool, &id).await?.unwrap())
}
pub async fn adopt_zone(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    create_zone(pool,&json!({"domain":s(b,"domain",""),"dns_provider_config_id":opt(b,"dns_provider_config_id"),"managed":true})).await
}
pub async fn delete_zone(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_dns_records WHERE zone_id=?")
        .bind(id)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_dns_zones WHERE id=? OR domain=?")
        .bind(id)
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0}))
}

pub async fn records(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_dns_records WHERE zone_id=? ORDER BY type,name")
        .bind(zone_id)
        .fetch_all(pool)
        .await?;
    Ok(json!({"records":rows.iter().map(record_value).collect::<Vec<_>>() }))
}
pub async fn create_record(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    if get_zone(pool, zone_id).await?.is_none() {
        return Ok(json!({"success":false,"error":"zone not found"}));
    }
    let id = id();
    let ts = now();
    let rtype = s(b, "type", "A").to_uppercase();
    let name = s(b, "name", "@");
    let value = s(b, "value", s(b, "content", ""));
    if value.is_empty() {
        return Ok(json!({"success":false,"error":"record value/content is required"}));
    }
    sqlx::query("INSERT INTO sk_dns_records(id,zone_id,type,name,value,ttl,priority,proxied,source,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)").bind(&id).bind(zone_id).bind(&rtype).bind(name).bind(value).bind(b.get("ttl").and_then(Value::as_i64).unwrap_or(300)).bind(b.get("priority").and_then(Value::as_i64)).bind(if b.get("proxied").and_then(Value::as_bool).unwrap_or(false){1}else{0}).bind(s(b,"source","manual")).bind(&ts).bind(&ts).execute(pool).await?;
    log_change(
        pool,
        None,
        Some(zone_id),
        "record.create",
        "success",
        Some(&format!("{rtype} {name}")),
    )
    .await?;
    let row = sqlx::query("SELECT * FROM sk_dns_records WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    Ok(record_value(&row))
}
pub async fn update_record(pool: &SqlitePool, id: &str, b: &Value) -> anyhow::Result<Value> {
    let old = sqlx::query("SELECT * FROM sk_dns_records WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(old) = old else {
        return Ok(json!({"success":false,"error":"record not found"}));
    };
    let rtype = opt(b, "type")
        .map(str::to_uppercase)
        .unwrap_or_else(|| old.get("type"));
    sqlx::query("UPDATE sk_dns_records SET type=?, name=?, value=?, ttl=?, priority=?, proxied=?, updated_at=? WHERE id=?").bind(&rtype).bind(opt(b,"name").unwrap_or_else(||old.get::<String,_>("name").leak())).bind(opt(b,"value").or_else(||opt(b,"content")).unwrap_or_else(||old.get::<String,_>("value").leak())).bind(b.get("ttl").and_then(Value::as_i64).unwrap_or(old.get("ttl"))).bind(b.get("priority").and_then(Value::as_i64).or_else(||old.try_get::<Option<i64>,_>("priority").ok().flatten())).bind(if b.get("proxied").and_then(Value::as_bool).unwrap_or(old.get::<i64,_>("proxied")!=0){1}else{0}).bind(now()).bind(id).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_dns_records WHERE id=?")
        .bind(id)
        .fetch_one(pool)
        .await?;
    log_change(
        pool,
        None,
        Some(&row.get::<String, _>("zone_id")),
        "record.update",
        "success",
        Some(id),
    )
    .await?;
    Ok(record_value(&row))
}
pub async fn delete_record(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT zone_id FROM sk_dns_records WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_dns_records WHERE id=?")
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    if let Some(r) = r {
        log_change(
            pool,
            None,
            Some(&r.get::<String, _>("zone_id")),
            "record.delete",
            "success",
            Some(id),
        )
        .await?;
    }
    Ok(json!({"success":n>0}))
}

pub fn presets() -> Value {
    json!({"presets":[{"key":"basic","name":"Basic website","records":[{"type":"A","name":"@","value":"${ip}"},{"type":"CNAME","name":"www","value":"@"}]},{"key":"google-workspace","name":"Google Workspace MX","records":[{"type":"MX","name":"@","value":"smtp.google.com","priority":1}]}]})
}
pub async fn apply_preset(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    let preset = s(b, "preset", "basic");
    let vars = b.get("variables").cloned().unwrap_or(Value::Null);
    let ip = s(&vars, "ip", "");
    let mut created = Vec::new();
    if preset == "basic" {
        if ip.is_empty() {
            return Ok(
                json!({"success":false,"error":"variables.ip is required for basic preset"}),
            );
        }
        created.push(
            create_record(
                pool,
                zone_id,
                &json!({"type":"A","name":"@","value":ip,"source":"preset"}),
            )
            .await?,
        );
        created.push(
            create_record(
                pool,
                zone_id,
                &json!({"type":"CNAME","name":"www","value":"@","source":"preset"}),
            )
            .await?,
        );
    } else if preset == "google-workspace" {
        created.push(create_record(pool,zone_id,&json!({"type":"MX","name":"@","value":"smtp.google.com","priority":1,"source":"preset"})).await?);
    } else {
        return Ok(json!({"success":false,"error":"unknown DNS preset"}));
    }
    Ok(json!({"success":true,"records":created}))
}
pub async fn export_zone(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    let zone = get_zone(pool, zone_id).await?.context("zone not found")?;
    let recs = records(pool, zone_id).await?;
    let mut zone_file = String::new();
    for r in recs["records"].as_array().into_iter().flatten() {
        zone_file.push_str(&format!(
            "{} {} IN {} {}\n",
            r["name"].as_str().unwrap_or("@"),
            r["ttl"].as_i64().unwrap_or(300),
            r["type"].as_str().unwrap_or("A"),
            r["value"].as_str().unwrap_or("")
        ));
    }
    Ok(json!({"zone":zone,"zone_file":zone_file}))
}
pub async fn import_zone(pool: &SqlitePool, zone_id: &str, b: &Value) -> anyhow::Result<Value> {
    let text = s(b, "zone_file", "");
    let mut imported = 0;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with(';') || t.starts_with('#') {
            continue;
        }
        let parts: Vec<_> = t.split_whitespace().collect();
        if parts.len() >= 5 && parts[2].eq_ignore_ascii_case("IN") {
            let _=create_record(pool,zone_id,&json!({"name":parts[0],"ttl":parts[1].parse::<i64>().unwrap_or(300),"type":parts[3],"value":parts[4],"source":"import"})).await?;
            imported += 1;
        }
    }
    Ok(json!({"success":true,"imported":imported}))
}
pub async fn managed(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows=sqlx::query("SELECT r.*, z.domain FROM sk_dns_records r JOIN sk_dns_zones z ON z.id=r.zone_id ORDER BY z.domain,r.type,r.name").fetch_all(pool).await?;
    Ok(
        json!({"records":rows.iter().map(|r|{let mut v=record_value(r); v["domain"]=json!(r.get::<String,_>("domain")); v}).collect::<Vec<_>>() }),
    )
}
pub async fn mirror(pool: &SqlitePool, zone_id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"zone":get_zone(pool,zone_id).await?,"records":records(pool,zone_id).await?["records"].clone(),"provider":{"configured":false,"message":"No live DNS provider adapter is configured for this zone"}}),
    )
}
pub async fn provider_records(
    _pool: &SqlitePool,
    config_id: Option<&str>,
    zone: Option<&str>,
) -> anyhow::Result<Value> {
    Ok(
        json!({"success":false,"configured":false,"config_id":config_id,"zone":zone,"records":[],"error":"DNS provider live-record access is not configured"}),
    )
}
pub async fn portfolio(pool: &SqlitePool) -> anyhow::Result<Value> {
    let zones = zones(pool).await?;
    let regs = registrar_domains(pool).await?;
    Ok(json!({"domains":zones["zones"],"zones":zones["zones"],"registrar_domains":regs["domains"]}))
}

pub async fn propagation(domain: &str, rtype: &str) -> anyhow::Result<Value> {
    let url = "https://cloudflare-dns.com/dns-query";
    let resp = reqwest::Client::new()
        .get(url)
        .query(&[("name", domain), ("type", rtype)])
        .header("accept", "application/dns-json")
        .send()
        .await;
    match resp {
        Ok(r) => {
            let v: Value = r.json().await.unwrap_or(Value::Null);
            let answers = v["Answer"].as_array().cloned().unwrap_or_default();
            Ok(
                json!({"domain":domain,"type":rtype,"propagated":!answers.is_empty(),"answers":answers,"resolver":"cloudflare-doh"}),
            )
        }
        Err(e) => {
            Ok(json!({"domain":domain,"type":rtype,"propagated":false,"error":e.to_string()}))
        }
    }
}
pub async fn registration(domain: &str) -> anyhow::Result<Value> {
    let url = format!("https://rdap.org/domain/{domain}");
    match reqwest::Client::new().get(&url).send().await {
        Ok(r) => {
            if !r.status().is_success() {
                return Ok(json!({"domain":domain,"found":false,"status":r.status().as_u16()}));
            }
            let v: Value = r.json().await.unwrap_or(Value::Null);
            let registrar = v["entities"]
                .as_array()
                .and_then(|a| a.first())
                .and_then(|e| e["vcardArray"][1].as_array())
                .and_then(|items| items.iter().find(|i| i[0].as_str() == Some("fn")))
                .and_then(|i| i[3].as_str())
                .map(str::to_string);
            let expires = v["events"]
                .as_array()
                .and_then(|events| {
                    events
                        .iter()
                        .find(|e| e["eventAction"].as_str() == Some("expiration"))
                })
                .and_then(|e| e["eventDate"].as_str())
                .map(str::to_string);
            Ok(
                json!({"domain":domain,"found":true,"registrar":registrar,"expires_at":expires,"raw":v}),
            )
        }
        Err(e) => Ok(json!({"domain":domain,"found":false,"error":e.to_string()})),
    }
}

async fn log_change(
    pool: &SqlitePool,
    config_id: Option<&str>,
    zone_id: Option<&str>,
    action: &str,
    result: &str,
    message: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query("INSERT INTO sk_dns_changes(id,config_id,zone_id,action,result,message,created_at) VALUES(?,?,?,?,?,?,?)").bind(id()).bind(config_id).bind(zone_id).bind(action).bind(result).bind(message).bind(now()).execute(pool).await?;
    Ok(())
}
pub async fn changes(
    pool: &SqlitePool,
    config_id: Option<&str>,
    zone: Option<&str>,
    result: Option<&str>,
    limit: i64,
) -> anyhow::Result<Value> {
    let mut rows = sqlx::query("SELECT * FROM sk_dns_changes ORDER BY created_at DESC LIMIT ?")
        .bind(limit.clamp(1, 500))
        .fetch_all(pool)
        .await?;
    rows.retain(|r| {
        config_id
            .map(|c| {
                r.try_get::<Option<String>, _>("config_id")
                    .ok()
                    .flatten()
                    .as_deref()
                    == Some(c)
            })
            .unwrap_or(true)
            && zone
                .map(|z| {
                    r.try_get::<Option<String>, _>("zone_id")
                        .ok()
                        .flatten()
                        .as_deref()
                        == Some(z)
                })
                .unwrap_or(true)
            && result
                .map(|x| r.get::<String, _>("result") == x)
                .unwrap_or(true)
    });
    Ok(
        json!({"changes":rows.iter().map(|r|json!({"id":r.get::<String,_>("id"),"config_id":r.try_get::<Option<String>,_>("config_id").ok().flatten(),"zone_id":r.try_get::<Option<String>,_>("zone_id").ok().flatten(),"action":r.get::<String,_>("action"),"result":r.get::<String,_>("result"),"message":r.try_get::<Option<String>,_>("message").ok().flatten(),"created_at":r.get::<String,_>("created_at")})).collect::<Vec<_>>() }),
    )
}

pub async fn ddns_hosts(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_ddns_hosts ORDER BY hostname")
        .fetch_all(pool)
        .await?;
    Ok(json!({"hosts":rows.iter().map(ddns_value).collect::<Vec<_>>() }))
}
pub async fn create_ddns_host(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let hostname = s(b, "hostname", s(b, "domain", ""))
        .trim()
        .trim_end_matches('.')
        .to_lowercase();
    if hostname.is_empty() {
        return Ok(json!({"success":false,"error":"hostname is required"}));
    }
    let id = id();
    let ts = now();
    let t = token();
    sqlx::query("INSERT INTO sk_ddns_hosts(id,hostname,zone_id,record_id,token_encrypted,current_ip,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)").bind(&id).bind(&hostname).bind(opt(b,"zone_id")).bind(opt(b,"record_id")).bind(sk_core::crypto::encrypt(&t)).bind(opt(b,"current_ip")).bind(&ts).bind(&ts).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_ddns_hosts WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    let mut v = ddns_value(&row);
    v["token"] = json!(t);
    Ok(v)
}
pub async fn delete_ddns_host(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_ddns_hosts WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}
pub async fn regen_ddns_token(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let t = token();
    let n = sqlx::query("UPDATE sk_ddns_hosts SET token_encrypted=?, updated_at=? WHERE id=?")
        .bind(sk_core::crypto::encrypt(&t))
        .bind(now())
        .bind(id)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(if n > 0 {
        json!({"success":true,"token":t})
    } else {
        json!({"success":false,"error":"host not found"})
    })
}

pub async fn registrar_connections(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_registrar_connections ORDER BY name")
        .fetch_all(pool)
        .await?;
    Ok(json!({"connections":rows.iter().map(registrar_connection_value).collect::<Vec<_>>() }))
}
pub async fn add_registrar_connection(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let id = id();
    let ts = now();
    let provider = s(b, "provider", "manual");
    let name = s(b, "name", provider);
    sqlx::query("INSERT INTO sk_registrar_connections(id,provider,name,config_encrypted,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?)").bind(&id).bind(provider).bind(name).bind(encrypt_json(b)).bind("configured").bind(&ts).bind(&ts).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_registrar_connections WHERE id=?")
        .bind(&id)
        .fetch_one(pool)
        .await?;
    Ok(registrar_connection_value(&row))
}
pub async fn delete_registrar_connection(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    sqlx::query("DELETE FROM sk_registrar_domains WHERE connection_id=?")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_registrar_connections WHERE id=?").bind(id).execute(pool).await?.rows_affected()>0}),
    )
}
pub async fn test_registrar_connection(pool: &SqlitePool, id: &str) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT * FROM sk_registrar_connections WHERE id=?")
        .bind(id)
        .fetch_optional(pool)
        .await?;
    let Some(row) = row else {
        return Ok(json!({"success":false,"error":"connection not found"}));
    };
    let cfg = decrypt_json(&row.get::<String, _>("config_encrypted"));
    if cfg
        .get("domains")
        .and_then(Value::as_array)
        .map(|a| !a.is_empty())
        .unwrap_or(false)
    {
        Ok(
            json!({"success":true,"mode":"manual-domains","provider":row.get::<String,_>("provider")}),
        )
    } else {
        Ok(
            json!({"success":false,"configured":true,"provider":row.get::<String,_>("provider"),"error":"No live registrar API adapter is configured; add a domains array for manual sync"}),
        )
    }
}
pub async fn registrar_domains(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_registrar_domains ORDER BY domain")
        .fetch_all(pool)
        .await?;
    Ok(json!({"domains":rows.iter().map(registrar_domain_value).collect::<Vec<_>>() }))
}
pub async fn sync_registrar_domains(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_registrar_connections")
        .fetch_all(pool)
        .await?;
    let mut synced = 0;
    for r in rows {
        let conn_id: String = r.get("id");
        let cfg = decrypt_json(&r.get::<String, _>("config_encrypted"));
        for d in cfg
            .get("domains")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            let reg = registration(d)
                .await
                .unwrap_or_else(|_| json!({"domain":d,"found":false}));
            let rid = id();
            sqlx::query("INSERT INTO sk_registrar_domains(id,connection_id,domain,registrar,expires_at,status,raw_json,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(connection_id,domain) DO UPDATE SET registrar=excluded.registrar, expires_at=excluded.expires_at, status=excluded.status, raw_json=excluded.raw_json, updated_at=excluded.updated_at").bind(rid).bind(&conn_id).bind(d).bind(reg.get("registrar").and_then(Value::as_str)).bind(reg.get("expires_at").and_then(Value::as_str)).bind(if reg.get("found").and_then(Value::as_bool).unwrap_or(false){"active"}else{"unknown"}).bind(reg.to_string()).bind(now()).execute(pool).await?;
            synced += 1;
        }
    }
    Ok(json!({"success":true,"synced":synced,"domains":registrar_domains(pool).await?["domains"]}))
}
