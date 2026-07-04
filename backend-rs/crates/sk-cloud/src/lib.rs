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
fn j(x: Option<String>) -> Value {
    x.and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or(Value::Null)
}
fn provider_env(kind: &str) -> bool {
    match kind {
        "digitalocean" => std::env::var("SK_DO_TOKEN").is_ok(),
        "hetzner" => std::env::var("SK_HCLOUD_TOKEN").is_ok(),
        "aws" => {
            std::env::var("AWS_ACCESS_KEY_ID").is_ok()
                && std::env::var("AWS_SECRET_ACCESS_KEY").is_ok()
        }
        "linode" => std::env::var("SK_LINODE_TOKEN").is_ok(),
        _ => false,
    }
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(r#"
CREATE TABLE IF NOT EXISTS sk_cloud_providers(id TEXT PRIMARY KEY, kind TEXT NOT NULL, name TEXT NOT NULL, config_json TEXT NOT NULL DEFAULT '{}', secret_token TEXT, status TEXT NOT NULL DEFAULT 'local', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_cloud_servers(id TEXT PRIMARY KEY, provider_id TEXT NOT NULL, provider_server_id TEXT, name TEXT NOT NULL, region TEXT, size TEXT, image TEXT, status TEXT NOT NULL, server_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
CREATE TABLE IF NOT EXISTS sk_cloud_snapshots(id TEXT PRIMARY KEY, cloud_server_id TEXT NOT NULL, provider_snapshot_id TEXT, name TEXT NOT NULL, status TEXT NOT NULL, snapshot_json TEXT NOT NULL DEFAULT '{}', created_at TEXT NOT NULL);
"#).execute(pool).await.context("ensure sk-cloud schema")?;
    Ok(())
}

fn redact_config(mut config: Value) -> Value {
    if let Some(o) = config.as_object_mut() {
        for key in [
            "token",
            "api_key",
            "secret",
            "access_key",
            "secret_key",
            "password",
        ] {
            if o.contains_key(key) {
                o.insert(key.into(), json!("********"));
            }
        }
    }
    config
}
fn provider_value(r: sqlx::sqlite::SqliteRow) -> Value {
    let kind: String = r.get("kind");
    json!({"id":r.get::<String,_>("id"),"type":kind,"kind":kind,"name":r.get::<String,_>("name"),"configured":provider_env(&kind),"status":r.get::<String,_>("status"),"config":redact_config(j(Some(r.get("config_json")))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn providers(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_cloud_providers ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    let vals: Vec<Value> = rows.into_iter().map(provider_value).collect();
    Ok(
        json!({"providers":vals,"items":vals,"count":vals.len(),"supported":["digitalocean","hetzner","aws","linode"]}),
    )
}
pub async fn create_provider(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let kind = s(b, "type", s(b, "kind", "custom"));
    let name = s(b, "name", kind);
    let pid = id();
    let ts = now();
    let token = b
        .get("token")
        .or_else(|| b.get("api_key"))
        .or_else(|| b.get("secret"))
        .and_then(Value::as_str)
        .map(sk_core::crypto::encrypt);
    sqlx::query("INSERT INTO sk_cloud_providers(id,kind,name,config_json,secret_token,status,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?)")
        .bind(&pid).bind(kind).bind(name).bind(b.to_string()).bind(token).bind(if provider_env(kind){"configured"}else{"local"}).bind(&ts).bind(&ts).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_cloud_providers WHERE id=?")
        .bind(&pid)
        .fetch_one(pool)
        .await?;
    Ok(json!({"success":true,"provider":provider_value(row),"configured":provider_env(kind)}))
}
pub async fn delete_provider(pool: &SqlitePool, pid: &str) -> anyhow::Result<Value> {
    let active = sqlx::query("SELECT COUNT(*) count FROM sk_cloud_servers WHERE provider_id=?")
        .bind(pid)
        .fetch_one(pool)
        .await?
        .get::<i64, _>("count");
    if active > 0 {
        return Ok(
            json!({"success":false,"code":"PROVIDER_IN_USE","error":"Provider has cloud servers"}),
        );
    }
    let n = sqlx::query("DELETE FROM sk_cloud_providers WHERE id=?")
        .bind(pid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n}))
}
pub async fn provider_options(pool: &SqlitePool, kind_or_id: &str) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT kind FROM sk_cloud_providers WHERE id=?")
        .bind(kind_or_id)
        .fetch_optional(pool)
        .await?;
    let kind = row
        .map(|r| r.get::<String, _>("kind"))
        .unwrap_or_else(|| kind_or_id.to_string());
    let opts = match kind.as_str() {
        "digitalocean" => {
            json!({"regions":["nyc1","sfo3","ams3","fra1"],"sizes":["s-1vcpu-1gb","s-1vcpu-2gb","s-2vcpu-4gb"],"images":["ubuntu-24-04-x64","ubuntu-22-04-x64"]})
        }
        "hetzner" => {
            json!({"regions":["fsn1","nbg1","hel1","ash"],"sizes":["cx22","cx32","cx42"],"images":["ubuntu-24.04","ubuntu-22.04"]})
        }
        "aws" => {
            json!({"regions":["us-east-1","us-west-2","eu-central-1"],"sizes":["t3.micro","t3.small","t3.medium"],"images":["ubuntu-24.04","ubuntu-22.04"]})
        }
        _ => json!({"regions":[],"sizes":[],"images":[]}),
    };
    Ok(json!({"type":kind,"configured":provider_env(&kind),"options":opts}))
}

fn server_value(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"provider_id":r.get::<String,_>("provider_id"),"provider_server_id":r.get::<Option<String>,_>("provider_server_id"),"name":r.get::<String,_>("name"),"region":r.get::<Option<String>,_>("region"),"size":r.get::<Option<String>,_>("size"),"image":r.get::<Option<String>,_>("image"),"status":r.get::<String,_>("status"),"configured":false,"details":j(Some(r.get("server_json"))),"created_at":r.get::<String,_>("created_at"),"updated_at":r.get::<String,_>("updated_at")})
}
pub async fn servers(pool: &SqlitePool, provider_id: Option<&str>) -> anyhow::Result<Value> {
    let rows = if let Some(pid) = provider_id {
        sqlx::query("SELECT * FROM sk_cloud_servers WHERE provider_id=? ORDER BY created_at DESC")
            .bind(pid)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query("SELECT * FROM sk_cloud_servers ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?
    };
    let vals: Vec<Value> = rows.into_iter().map(server_value).collect();
    Ok(json!({"servers":vals,"items":vals,"count":vals.len()}))
}
pub async fn get_server(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    let row = sqlx::query("SELECT * FROM sk_cloud_servers WHERE id=?")
        .bind(sid)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some(r) => json!({"server":server_value(r)}),
        None => {
            json!({"success":false,"code":"CLOUD_SERVER_NOT_FOUND","error":"Cloud server not found"})
        }
    })
}
pub async fn create_server(pool: &SqlitePool, b: &Value) -> anyhow::Result<Value> {
    let provider_id = s(b, "provider_id", "");
    let prow = sqlx::query("SELECT * FROM sk_cloud_providers WHERE id=?")
        .bind(provider_id)
        .fetch_optional(pool)
        .await?;
    let Some(p) = prow else {
        return Ok(
            json!({"success":false,"code":"PROVIDER_NOT_FOUND","error":"Cloud provider not found"}),
        );
    };
    let kind: String = p.get("kind");
    let configured = provider_env(&kind);
    let sid = id();
    let ts = now();
    let name = s(b, "name", "serverkit-cloud-server");
    let status = if configured {
        "provision_requested"
    } else {
        "provider_unconfigured"
    };
    let details = json!({"requested":b,"provider_configured":configured,"message":if configured{"Provision request recorded; provider reconciler will create the server"}else{"Provider credentials are not configured; desired state recorded only"}});
    sqlx::query("INSERT INTO sk_cloud_servers(id,provider_id,name,region,size,image,status,server_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
        .bind(&sid).bind(provider_id).bind(name).bind(b.get("region").and_then(Value::as_str)).bind(b.get("size").and_then(Value::as_str)).bind(b.get("image").and_then(Value::as_str)).bind(status).bind(details.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    Ok(
        json!({"success":true,"configured":configured,"server":get_server(pool,&sid).await?["server"].clone()}),
    )
}
pub async fn delete_server(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    let row=sqlx::query("SELECT p.kind FROM sk_cloud_servers s JOIN sk_cloud_providers p ON p.id=s.provider_id WHERE s.id=?").bind(sid).fetch_optional(pool).await?;
    let configured = row
        .as_ref()
        .map(|r| provider_env(&r.get::<String, _>("kind")))
        .unwrap_or(false);
    sqlx::query("DELETE FROM sk_cloud_snapshots WHERE cloud_server_id=?")
        .bind(sid)
        .execute(pool)
        .await?;
    let n = sqlx::query("DELETE FROM sk_cloud_servers WHERE id=?")
        .bind(sid)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(
        json!({"success":n>0,"deleted":n,"configured":configured,"provider_action":if configured{"delete_requested"}else{"local_record_deleted"}}),
    )
}
pub async fn resize_server(pool: &SqlitePool, sid: &str, size: &str) -> anyhow::Result<Value> {
    let mut cur = get_server(pool, sid).await?;
    if cur.get("server").is_none() {
        return Ok(cur);
    }
    let mut details = cur["server"]["details"].clone();
    details["resize"] = json!({"size":size,"requested_at":now(),"configured":false});
    let n=sqlx::query("UPDATE sk_cloud_servers SET size=?,status='resize_requested',server_json=?,updated_at=? WHERE id=?").bind(size).bind(details.to_string()).bind(now()).bind(sid).execute(pool).await?.rows_affected();
    cur = get_server(pool, sid).await?;
    Ok(json!({"success":n>0,"configured":false,"server":cur["server"].clone()}))
}
fn snapshot_value(r: sqlx::sqlite::SqliteRow) -> Value {
    json!({"id":r.get::<String,_>("id"),"cloud_server_id":r.get::<String,_>("cloud_server_id"),"provider_snapshot_id":r.get::<Option<String>,_>("provider_snapshot_id"),"name":r.get::<String,_>("name"),"status":r.get::<String,_>("status"),"details":j(Some(r.get("snapshot_json"))),"created_at":r.get::<String,_>("created_at")})
}
pub async fn snapshots(pool: &SqlitePool, sid: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT * FROM sk_cloud_snapshots WHERE cloud_server_id=? ORDER BY created_at DESC",
    )
    .bind(sid)
    .fetch_all(pool)
    .await?;
    let vals: Vec<Value> = rows.into_iter().map(snapshot_value).collect();
    Ok(json!({"snapshots":vals,"items":vals,"count":vals.len()}))
}
pub async fn create_snapshot(pool: &SqlitePool, sid: &str, name: &str) -> anyhow::Result<Value> {
    if get_server(pool, sid).await?.get("server").is_none() {
        return Ok(
            json!({"success":false,"code":"CLOUD_SERVER_NOT_FOUND","error":"Cloud server not found"}),
        );
    }
    let snap = id();
    let details = json!({"configured":false,"message":"Snapshot desired state recorded; provider credentials/reconciler required for physical snapshot"});
    sqlx::query("INSERT INTO sk_cloud_snapshots(id,cloud_server_id,name,status,snapshot_json,created_at) VALUES(?,?,?,?,?,?)").bind(&snap).bind(sid).bind(name).bind("requested").bind(details.to_string()).bind(now()).execute(pool).await?;
    let row = sqlx::query("SELECT * FROM sk_cloud_snapshots WHERE id=?")
        .bind(&snap)
        .fetch_one(pool)
        .await?;
    Ok(json!({"success":true,"configured":false,"snapshot":snapshot_value(row)}))
}
pub async fn delete_snapshot(pool: &SqlitePool, snap: &str) -> anyhow::Result<Value> {
    let n = sqlx::query("DELETE FROM sk_cloud_snapshots WHERE id=?")
        .bind(snap)
        .execute(pool)
        .await?
        .rows_affected();
    Ok(json!({"success":n>0,"deleted":n,"configured":false}))
}
pub async fn costs(pool: &SqlitePool) -> anyhow::Result<Value> {
    let rows = sqlx::query(
        "SELECT provider_id,size,COUNT(*) count FROM sk_cloud_servers GROUP BY provider_id,size",
    )
    .fetch_all(pool)
    .await?;
    let items:Vec<Value>=rows.into_iter().map(|r|json!({"provider_id":r.get::<String,_>("provider_id"),"size":r.get::<Option<String>,_>("size"),"count":r.get::<i64,_>("count"),"estimated_monthly_usd":0,"source":"local_inventory"})).collect();
    Ok(json!({"currency":"USD","estimated_monthly_usd":0,"items":items,"configured_billing":false}))
}
