use anyhow::Context;
use chrono::Utc;
use rand::{distributions::Alphanumeric, Rng};
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
fn now() -> String {
    Utc::now().to_rfc3339()
}
fn s<'a>(v: &'a Value, k: &str, d: &'a str) -> &'a str {
    v.get(k).and_then(Value::as_str).unwrap_or(d)
}
fn b(v: &Value, k: &str, d: bool) -> bool {
    v.get(k).and_then(Value::as_bool).unwrap_or(d)
}
fn state() -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(32)
        .map(char::from)
        .collect()
}
fn enc(v: &str) -> String {
    if v.is_empty() {
        String::new()
    } else {
        sk_core::crypto::encrypt(v)
    }
}
fn dec(v: &str) -> String {
    if v.is_empty() {
        String::new()
    } else {
        sk_core::crypto::decrypt_or_plain(v)
    }
}
pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_sso_general(id INTEGER PRIMARY KEY CHECK(id=1), auto_provision INTEGER NOT NULL DEFAULT 1, default_role TEXT NOT NULL DEFAULT 'developer', force_sso INTEGER NOT NULL DEFAULT 0, allowed_domains_json TEXT NOT NULL DEFAULT '[]', updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_sso_providers(id TEXT PRIMARY KEY, enabled INTEGER NOT NULL DEFAULT 0, provider_name TEXT, client_id TEXT, client_secret_enc TEXT, discovery_url TEXT, entity_id TEXT, idp_metadata_url TEXT, idp_sso_url TEXT, idp_cert_enc TEXT, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_sso_identities(id TEXT PRIMARY KEY, user_id TEXT NOT NULL, provider TEXT NOT NULL, subject TEXT NOT NULL, email TEXT, display_name TEXT, created_at TEXT NOT NULL, UNIQUE(user_id, provider));
CREATE TABLE IF NOT EXISTS sk_sso_states(state TEXT PRIMARY KEY, provider TEXT NOT NULL, redirect_uri TEXT, user_id TEXT, purpose TEXT NOT NULL, created_at TEXT NOT NULL);
INSERT OR IGNORE INTO sk_sso_general(id, updated_at) VALUES(1, datetime('now'));
"#).execute(pool).await.context("ensure sk-sso schema")?;
    Ok(())
}
const PROVIDERS: &[(&str, &str)] = &[
    ("google", "Google"),
    ("github", "GitHub"),
    ("oidc", "OIDC"),
    ("saml", "SAML 2.0"),
];
async fn general(pool: &SqlitePool) -> anyhow::Result<Value> {
    let r = sqlx::query("SELECT * FROM sk_sso_general WHERE id=1")
        .fetch_one(pool)
        .await?;
    Ok(
        json!({"sso_auto_provision":r.get::<i64,_>("auto_provision")!=0,"sso_default_role":r.get::<String,_>("default_role"),"sso_force_sso":r.get::<i64,_>("force_sso")!=0,"sso_allowed_domains":serde_json::from_str::<Value>(&r.get::<String,_>("allowed_domains_json")).unwrap_or_else(|_|json!([]))}),
    )
}
async fn provider_row(
    pool: &SqlitePool,
    pid: &str,
) -> anyhow::Result<Option<sqlx::sqlite::SqliteRow>> {
    Ok(sqlx::query("SELECT * FROM sk_sso_providers WHERE id=?")
        .bind(pid)
        .fetch_optional(pool)
        .await?)
}
fn cfg_value(r: &sqlx::sqlite::SqliteRow, k: &str) -> String {
    r.try_get::<Option<String>, _>(k)
        .ok()
        .flatten()
        .unwrap_or_default()
}
fn redact_secret(v: String) -> String {
    if v.is_empty() {
        v
    } else {
        "***".into()
    }
}
pub async fn admin_config(pool: &SqlitePool) -> anyhow::Result<Value> {
    let mut obj = general(pool)
        .await?
        .as_object()
        .cloned()
        .unwrap_or_default();
    for (pid, _) in PROVIDERS {
        if let Some(r) = provider_row(pool, pid).await? {
            obj.insert(
                format!("sso_{pid}_enabled"),
                json!(r.get::<i64, _>("enabled") != 0),
            );
            obj.insert(
                format!("sso_{pid}_client_id"),
                json!(cfg_value(&r, "client_id")),
            );
            obj.insert(
                format!("sso_{pid}_client_secret"),
                json!(redact_secret(cfg_value(&r, "client_secret_enc"))),
            );
            obj.insert(
                format!("sso_{pid}_provider_name"),
                json!(cfg_value(&r, "provider_name")),
            );
            obj.insert(
                format!("sso_{pid}_discovery_url"),
                json!(cfg_value(&r, "discovery_url")),
            );
            obj.insert(
                format!("sso_{pid}_entity_id"),
                json!(cfg_value(&r, "entity_id")),
            );
            obj.insert(
                format!("sso_{pid}_idp_metadata_url"),
                json!(cfg_value(&r, "idp_metadata_url")),
            );
            obj.insert(
                format!("sso_{pid}_idp_sso_url"),
                json!(cfg_value(&r, "idp_sso_url")),
            );
            obj.insert(
                format!("sso_{pid}_idp_cert"),
                json!(redact_secret(cfg_value(&r, "idp_cert_enc"))),
            );
        } else {
            obj.insert(format!("sso_{pid}_enabled"), json!(false));
        }
    }
    Ok(json!({"success":true,"config":Value::Object(obj)}))
}
pub async fn update_general(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    sqlx::query("INSERT INTO sk_sso_general(id,auto_provision,default_role,force_sso,allowed_domains_json,updated_at) VALUES(1,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET auto_provision=excluded.auto_provision,default_role=excluded.default_role,force_sso=excluded.force_sso,allowed_domains_json=excluded.allowed_domains_json,updated_at=excluded.updated_at").bind(if b(body,"sso_auto_provision",true){1}else{0}).bind(s(body,"sso_default_role","developer")).bind(if b(body,"sso_force_sso",false){1}else{0}).bind(body.get("sso_allowed_domains").cloned().unwrap_or_else(||json!([])).to_string()).bind(now()).execute(pool).await?;
    admin_config(pool).await
}
pub async fn update_provider(pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    if !PROVIDERS.iter().any(|p| p.0 == pid) {
        return Ok(json!({"success":false,"code":"UNKNOWN_SSO_PROVIDER"}));
    }
    let old = provider_row(pool, pid).await?;
    let old_secret = old
        .as_ref()
        .map(|r| cfg_value(r, "client_secret_enc"))
        .unwrap_or_default();
    let old_cert = old
        .as_ref()
        .map(|r| cfg_value(r, "idp_cert_enc"))
        .unwrap_or_default();
    let sec = s(body, "client_secret", "");
    let cert = s(body, "idp_cert", "");
    let sec_enc = if sec.is_empty() || sec == "***" {
        old_secret
    } else {
        enc(sec)
    };
    let cert_enc = if cert.is_empty() || cert == "***" {
        old_cert
    } else {
        enc(cert)
    };
    sqlx::query("INSERT INTO sk_sso_providers(id,enabled,provider_name,client_id,client_secret_enc,discovery_url,entity_id,idp_metadata_url,idp_sso_url,idp_cert_enc,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(id) DO UPDATE SET enabled=excluded.enabled,provider_name=excluded.provider_name,client_id=excluded.client_id,client_secret_enc=excluded.client_secret_enc,discovery_url=excluded.discovery_url,entity_id=excluded.entity_id,idp_metadata_url=excluded.idp_metadata_url,idp_sso_url=excluded.idp_sso_url,idp_cert_enc=excluded.idp_cert_enc,updated_at=excluded.updated_at").bind(pid).bind(if b(body,"enabled",false){1}else{0}).bind(s(body,"provider_name","")).bind(s(body,"client_id","")).bind(sec_enc).bind(s(body,"discovery_url","")).bind(s(body,"entity_id","")).bind(s(body,"idp_metadata_url","")).bind(s(body,"idp_sso_url","")).bind(cert_enc).bind(now()).execute(pool).await?;
    admin_config(pool).await
}
fn provider_public(pid: &str, name: &str, r: &sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":pid,"name":name,"enabled":r.get::<i64,_>("enabled")!=0})
}
pub async fn providers(pool: &SqlitePool) -> anyhow::Result<Value> {
    let mut vals = Vec::new();
    for (pid, name) in PROVIDERS {
        if let Some(r) = provider_row(pool, pid).await? {
            if r.get::<i64, _>("enabled") != 0 {
                vals.push(provider_public(pid, name, &r));
            }
        }
    }
    Ok(
        json!({"success":true,"providers":vals,"password_login_enabled":!general(pool).await?["sso_force_sso"].as_bool().unwrap_or(false)}),
    )
}
pub async fn identities(pool: &SqlitePool, user_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_sso_identities WHERE user_id=? ORDER BY provider")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    let vals:Vec<_>=rows.into_iter().map(|r|json!({"id":r.get::<String,_>("id"),"provider":r.get::<String,_>("provider"),"subject":r.get::<String,_>("subject"),"email":r.get::<Option<String>,_>("email"),"display_name":r.get::<Option<String>,_>("display_name"),"created_at":r.get::<String,_>("created_at")})).collect();
    Ok(json!({"success":true,"identities":vals}))
}
fn auth_base(pid: &str, r: &sqlx::sqlite::SqliteRow) -> Option<String> {
    match pid {
        "google" => Some("https://accounts.google.com/o/oauth2/v2/auth".into()),
        "github" => Some("https://github.com/login/oauth/authorize".into()),
        "oidc" => None,
        "saml" => Some(cfg_value(r, "idp_sso_url")),
        _ => None,
    }
}
pub async fn authorize(
    pool: &SqlitePool,
    pid: &str,
    redirect_uri: Option<&str>,
    user_id: Option<&str>,
    purpose: &str,
) -> anyhow::Result<Value> {
    let Some(r) = provider_row(pool, pid).await? else {
        return Ok(json!({"success":false,"code":"SSO_PROVIDER_NOT_CONFIGURED"}));
    };
    if r.get::<i64, _>("enabled") == 0 {
        return Ok(json!({"success":false,"code":"SSO_PROVIDER_DISABLED"}));
    };
    let client_id = cfg_value(&r, "client_id");
    let Some(base) = auth_base(pid, &r).filter(|x| !x.is_empty()) else {
        return Ok(json!({"success":false,"code":"SSO_AUTHORIZE_URL_UNAVAILABLE"}));
    };
    if client_id.is_empty() && pid != "saml" {
        return Ok(json!({"success":false,"code":"SSO_CLIENT_ID_MISSING"}));
    };
    let st = state();
    sqlx::query("INSERT INTO sk_sso_states(state,provider,redirect_uri,user_id,purpose,created_at) VALUES(?,?,?,?,?,?)").bind(&st).bind(pid).bind(redirect_uri.unwrap_or("")).bind(user_id.unwrap_or("")).bind(purpose).bind(now()).execute(pool).await?;
    let red = redirect_uri.unwrap_or("");
    let url = if pid == "saml" {
        format!("{base}?RelayState={}", urlencoding::encode(&st))
    } else {
        format!(
            "{base}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
            urlencoding::encode(&client_id),
            urlencoding::encode(red),
            urlencoding::encode(if pid == "github" {
                "read:user user:email"
            } else {
                "openid email profile"
            }),
            urlencoding::encode(&st)
        )
    };
    Ok(json!({"success":true,"auth_url":url,"state":st}))
}
pub async fn callback(_pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    if s(body, "code", "").is_empty() {
        return Ok(json!({"success":false,"code":"SSO_AUTH_CODE_REQUIRED"}));
    }
    Ok(
        json!({"success":false,"code":"SSO_TOKEN_EXCHANGE_UNAVAILABLE","provider":pid,"error":"Provider token exchange is not configured in this Rust build without explicit adapter credentials"}),
    )
}
pub async fn link(pool: &SqlitePool, pid: &str, body: &Value) -> anyhow::Result<Value> {
    callback(pool, pid, body).await
}
pub async fn unlink(pool: &SqlitePool, user_id: &str, pid: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_sso_identities WHERE user_id=? AND provider=?")
        .bind(user_id)
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":true,"deleted":n}))
}
pub async fn test_provider(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let Some(r) = provider_row(pool, pid).await? else {
        return Ok(json!({"ok":false,"error":"Provider not configured"}));
    };
    if r.get::<i64, _>("enabled") == 0 {
        return Ok(json!({"ok":false,"error":"Provider disabled"}));
    };
    match pid {
        "oidc" => {
            let url = cfg_value(&r, "discovery_url");
            if url.is_empty() {
                return Ok(json!({"ok":false,"error":"discovery_url is required"}));
            }
            let resp = reqwest::Client::new().get(url).send().await;
            Ok(match resp {
                Ok(r) => {
                    json!({"ok":r.status().is_success(),"status":r.status().as_u16(),"message":"OIDC discovery URL reachable"})
                }
                Err(e) => json!({"ok":false,"error":e.to_string()}),
            })
        }
        "saml" => {
            let ok = !cfg_value(&r, "idp_sso_url").is_empty()
                || !cfg_value(&r, "idp_metadata_url").is_empty()
                || !dec(&cfg_value(&r, "idp_cert_enc")).is_empty();
            Ok(
                json!({"ok":ok,"message":if ok{"SAML configuration present"}else{"SAML IdP metadata, SSO URL, or certificate required"}}),
            )
        }
        _ => {
            let ok = !cfg_value(&r, "client_id").is_empty()
                && !dec(&cfg_value(&r, "client_secret_enc")).is_empty();
            Ok(
                json!({"ok":ok,"message":if ok{"OAuth client credentials present"}else{"client_id and client_secret required"}}),
            )
        }
    }
}
