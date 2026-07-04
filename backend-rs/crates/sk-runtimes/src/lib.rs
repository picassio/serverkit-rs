use anyhow::{anyhow, Context};
use chrono::Utc;
use serde_json::{json, Value};
use sqlx::{Row, SqlitePool};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use uuid::Uuid;

const BASE_DIR: &str = "/var/lib/serverkit/python-apps";

fn now() -> String {
    Utc::now().to_rfc3339()
}
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn s<'a>(v: &'a Value, key: &str, default: &'a str) -> &'a str {
    v.get(key).and_then(Value::as_str).unwrap_or(default)
}
fn opt_s<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}
fn j(s: Option<String>) -> Value {
    s.and_then(|x| serde_json::from_str(&x).ok())
        .unwrap_or(Value::Null)
}
fn valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}
fn app_dir(app_id: &str) -> PathBuf {
    Path::new(BASE_DIR).join(app_id)
}
fn unit_name(app_id: &str) -> String {
    format!("serverkit-python-{app_id}.service")
}

pub async fn ensure_schema(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
CREATE TABLE IF NOT EXISTS sk_python_apps(
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    framework TEXT NOT NULL,
    path TEXT NOT NULL,
    python_version TEXT NOT NULL,
    status TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 0,
    venv_path TEXT,
    gunicorn_config TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sk_python_env(
    app_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value_encrypted TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(app_id, key)
);
CREATE TABLE IF NOT EXISTS sk_python_packages(
    app_id TEXT NOT NULL,
    package TEXT NOT NULL,
    version TEXT,
    installed_at TEXT NOT NULL,
    PRIMARY KEY(app_id, package)
);
CREATE TABLE IF NOT EXISTS sk_node_apps(
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    node_version TEXT NOT NULL,
    package_manager TEXT NOT NULL,
    status TEXT NOT NULL,
    port INTEGER NOT NULL DEFAULT 0,
    start_command TEXT NOT NULL,
    metadata_json TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE IF NOT EXISTS sk_node_env(
    app_id TEXT NOT NULL,
    key TEXT NOT NULL,
    value_encrypted TEXT NOT NULL,
    updated_at TEXT NOT NULL,
    PRIMARY KEY(app_id, key)
);
CREATE TABLE IF NOT EXISTS sk_node_packages(
    app_id TEXT NOT NULL,
    package TEXT NOT NULL,
    version TEXT,
    installed_at TEXT NOT NULL,
    PRIMARY KEY(app_id, package)
);
"#,
    )
    .execute(pool)
    .await
    .context("ensure sk-runtimes schema")?;
    Ok(())
}

fn app_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"),
        "name": row.get::<String,_>("name"),
        "framework": row.get::<String,_>("framework"),
        "path": row.get::<String,_>("path"),
        "python_version": row.get::<String,_>("python_version"),
        "status": row.get::<String,_>("status"),
        "port": row.get::<i64,_>("port"),
        "venv_path": row.try_get::<Option<String>,_>("venv_path").ok().flatten(),
        "gunicorn_config": row.get::<String,_>("gunicorn_config"),
        "metadata": j(row.try_get::<Option<String>,_>("metadata_json").ok().flatten()),
        "created_at": row.get::<String,_>("created_at"),
        "updated_at": row.get::<String,_>("updated_at"),
    })
}

pub async fn python_versions() -> Value {
    let candidates = [
        "python3.13",
        "python3.12",
        "python3.11",
        "python3.10",
        "python3",
    ];
    let mut versions = Vec::new();
    for bin in candidates {
        let out = Command::new(bin).arg("--version").output().await;
        if let Ok(out) = out {
            if out.status.success() {
                let stdout = String::from_utf8_lossy(&out.stdout);
                let stderr = String::from_utf8_lossy(&out.stderr);
                let full = if stdout.trim().is_empty() {
                    stderr.trim()
                } else {
                    stdout.trim()
                };
                let version = full.strip_prefix("Python ").unwrap_or(full).to_string();
                if !versions.iter().any(|v: &Value| v["path"] == json!(bin)) {
                    versions.push(json!({"binary":bin,"path":bin,"version":version,"full_version":full,"installed":true}));
                }
            }
        }
    }
    json!({"versions":versions,"default":versions.first().cloned(),"supported":["3.10","3.11","3.12","3.13"]})
}

pub async fn create_app(pool: &SqlitePool, framework: &str, body: &Value) -> anyhow::Result<Value> {
    if !matches!(framework, "flask" | "django") {
        return Err(anyhow!("unsupported framework"));
    }
    let id = id();
    let name = s(
        body,
        "name",
        if framework == "flask" {
            "Flask App"
        } else {
            "Django App"
        },
    );
    let python_version = s(body, "python_version", s(body, "version", "python3"));
    let port = body.get("port").and_then(Value::as_i64).unwrap_or(0);
    let path = opt_s(body, "path")
        .map(PathBuf::from)
        .unwrap_or_else(|| app_dir(&id));
    tokio::fs::create_dir_all(&path).await?;
    if framework == "flask" {
        tokio::fs::write(path.join("app.py"), "def app(environ, start_response):\n    body = b'Hello from ServerKit Flask runtime'\n    start_response('200 OK', [('Content-Type', 'text/plain'), ('Content-Length', str(len(body)))])\n    return [body]\n").await?;
        tokio::fs::write(path.join("requirements.txt"), "gunicorn\n").await?;
    } else {
        tokio::fs::write(
            path.join("manage.py"),
            "#!/usr/bin/env python3\nprint('Django project placeholder managed by ServerKit')\n",
        )
        .await?;
        tokio::fs::write(path.join("requirements.txt"), "django\ngunicorn\n").await?;
    }
    let gunicorn = format!(
        "bind = '127.0.0.1:{}'\nworkers = 2\ntimeout = 120\n",
        if port > 0 { port } else { 8000 }
    );
    let ts = now();
    sqlx::query("INSERT INTO sk_python_apps(id,name,framework,path,python_version,status,port,gunicorn_config,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&id).bind(name).bind(framework).bind(path.to_string_lossy().to_string()).bind(python_version).bind("created").bind(port).bind(&gunicorn).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    get_app(pool, &id).await?.context("created app missing")
}

pub async fn get_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_python_apps WHERE id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(app_value))
}

pub async fn delete_app(
    pool: &SqlitePool,
    app_id: &str,
    remove_files: bool,
) -> anyhow::Result<Value> {
    let app = get_app(pool, app_id).await?;
    let Some(app) = app else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let _ = stop_app(pool, app_id).await;
    sqlx::query("DELETE FROM sk_python_env WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_python_packages WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_python_apps WHERE id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    if remove_files {
        if let Some(path) = app.get("path").and_then(Value::as_str) {
            let _ = tokio::fs::remove_dir_all(path).await;
        }
    }
    Ok(json!({"success":true,"id":app_id,"removed_files":remove_files}))
}

async fn run_cmd(cmd: &str, args: &[&str], dir: Option<&str>, timeout: u64) -> Value {
    let mut c = Command::new(cmd);
    c.args(args);
    if let Some(d) = dir {
        c.current_dir(d);
    }
    let fut = c.output();
    match tokio::time::timeout(std::time::Duration::from_secs(timeout), fut).await {
        Ok(Ok(out)) => {
            json!({"success":out.status.success(),"status":out.status.code(),"stdout":String::from_utf8_lossy(&out.stdout),"stderr":String::from_utf8_lossy(&out.stderr)})
        }
        Ok(Err(e)) => json!({"success":false,"error":e.to_string()}),
        Err(_) => json!({"success":false,"error":"command timed out"}),
    }
}

pub async fn create_venv(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let path = app["path"].as_str().unwrap_or("");
    let python = app["python_version"].as_str().unwrap_or("python3");
    let venv = Path::new(path).join("venv");
    let out = run_cmd(
        python,
        &["-m", "venv", venv.to_string_lossy().as_ref()],
        None,
        180,
    )
    .await;
    if out["success"].as_bool().unwrap_or(false) {
        sqlx::query(
            "UPDATE sk_python_apps SET venv_path=?, status='venv_ready', updated_at=? WHERE id=?",
        )
        .bind(venv.to_string_lossy().to_string())
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    }
    Ok(
        json!({"app_id":app_id,"venv_path":venv,"result":out,"success":out["success"].as_bool().unwrap_or(false)}),
    )
}

pub async fn packages(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_python_packages WHERE app_id=? ORDER BY package")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"packages":rows.iter().map(|r|json!({"name":r.get::<String,_>("package"),"version":r.try_get::<Option<String>,_>("version").ok().flatten(),"installed_at":r.get::<String,_>("installed_at")})).collect::<Vec<_>>() }),
    )
}

pub async fn install_packages(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let Some(venv) = app.get("venv_path").and_then(Value::as_str) else {
        return Ok(
            json!({"success":false,"error":"create a virtualenv before installing packages"}),
        );
    };
    let packages: Vec<String> = body
        .get("packages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if packages.is_empty() {
        return Ok(json!({"success":false,"error":"packages are required"}));
    }
    let pip = Path::new(venv).join("bin/pip");
    let mut args = vec!["install".to_string()];
    args.extend(packages.iter().cloned());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_cmd(
        pip.to_string_lossy().as_ref(),
        &refs,
        app.get("path").and_then(Value::as_str),
        600,
    )
    .await;
    if out["success"].as_bool().unwrap_or(false) {
        let ts = now();
        for p in &packages {
            sqlx::query("INSERT INTO sk_python_packages(app_id,package,installed_at) VALUES(?,?,?) ON CONFLICT(app_id,package) DO UPDATE SET installed_at=excluded.installed_at")
                .bind(app_id).bind(p).bind(&ts).execute(pool).await?;
        }
    }
    Ok(
        json!({"success":out["success"].as_bool().unwrap_or(false),"packages":packages,"result":out}),
    )
}

pub async fn freeze_requirements(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let Some(venv) = app.get("venv_path").and_then(Value::as_str) else {
        return Ok(
            json!({"success":false,"error":"create a virtualenv before freezing requirements"}),
        );
    };
    let pip = Path::new(venv).join("bin/pip");
    let out = run_cmd(
        pip.to_string_lossy().as_ref(),
        &["freeze"],
        app.get("path").and_then(Value::as_str),
        120,
    )
    .await;
    if out["success"].as_bool().unwrap_or(false) {
        if let (Some(path), Some(stdout)) = (
            app.get("path").and_then(Value::as_str),
            out.get("stdout").and_then(Value::as_str),
        ) {
            tokio::fs::write(Path::new(path).join("requirements.txt"), stdout).await?;
        }
    }
    Ok(
        json!({"success":out["success"].as_bool().unwrap_or(false),"requirements":out.get("stdout").cloned().unwrap_or(Value::Null),"result":out}),
    )
}

pub async fn env_vars(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT key,value_encrypted FROM sk_python_env WHERE app_id=? ORDER BY key")
            .bind(app_id)
            .fetch_all(pool)
            .await?;
    let mut obj = serde_json::Map::new();
    for r in rows {
        let key: String = r.get("key");
        let val: String = r.get("value_encrypted");
        obj.insert(
            key,
            json!(sk_core::crypto::decrypt(&val).unwrap_or_default()),
        );
    }
    Ok(json!({"env_vars":Value::Object(obj)}))
}

pub async fn set_env_vars(pool: &SqlitePool, app_id: &str, body: &Value) -> anyhow::Result<Value> {
    let vars = body
        .get("env_vars")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let ts = now();
    for (key, value) in vars {
        if !valid_key(&key) {
            continue;
        }
        let raw = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        sqlx::query("INSERT INTO sk_python_env(app_id,key,value_encrypted,updated_at) VALUES(?,?,?,?) ON CONFLICT(app_id,key) DO UPDATE SET value_encrypted=excluded.value_encrypted,updated_at=excluded.updated_at")
            .bind(app_id).bind(key).bind(sk_core::crypto::encrypt(&raw)).bind(&ts).execute(pool).await?;
    }
    env_vars(pool, app_id).await
}

pub async fn delete_env_var(pool: &SqlitePool, app_id: &str, key: &str) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_python_env WHERE app_id=? AND key=?").bind(app_id).bind(key).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn gunicorn_config(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    Ok(json!({"content":app["gunicorn_config"],"config":app["gunicorn_config"]}))
}

pub async fn set_gunicorn_config(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let content = s(body, "content", "");
    sqlx::query("UPDATE sk_python_apps SET gunicorn_config=?, updated_at=? WHERE id=?")
        .bind(content)
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    gunicorn_config(pool, app_id).await
}

async fn write_unit(app: &Value) -> anyhow::Result<String> {
    let app_id = app["id"].as_str().unwrap_or("");
    let path = app["path"].as_str().unwrap_or("");
    let unit = unit_name(app_id);
    let venv = app.get("venv_path").and_then(Value::as_str).unwrap_or("");
    let exec = if !venv.is_empty() && Path::new(venv).join("bin/gunicorn").exists() {
        format!("{venv}/bin/gunicorn -c {path}/gunicorn.conf.py app:app")
    } else {
        format!(
            "{} -m http.server {} --bind 127.0.0.1",
            app["python_version"].as_str().unwrap_or("python3"),
            app["port"].as_i64().unwrap_or(8000).max(8000)
        )
    };
    tokio::fs::write(
        Path::new(path).join("gunicorn.conf.py"),
        app["gunicorn_config"].as_str().unwrap_or(""),
    )
    .await?;
    let content = format!("[Unit]\nDescription=ServerKit Python app {app_id}\nAfter=network.target\n\n[Service]\nWorkingDirectory={path}\nExecStart={exec}\nRestart=on-failure\nEnvironment=PYTHONUNBUFFERED=1\n\n[Install]\nWantedBy=multi-user.target\n");
    let unit_path = format!("/etc/systemd/system/{unit}");
    tokio::fs::write(&unit_path, content).await?;
    let _ = run_cmd("systemctl", &["daemon-reload"], None, 30).await;
    Ok(unit)
}

pub async fn start_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let unit = write_unit(&app).await?;
    let out = run_cmd("systemctl", &["enable", "--now", &unit], None, 60).await;
    let ok = out["success"].as_bool().unwrap_or(false);
    sqlx::query("UPDATE sk_python_apps SET status=?, updated_at=? WHERE id=?")
        .bind(if ok { "running" } else { "start_failed" })
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    let _ = sk_jobs::insert_job(
        pool,
        "python.app.start",
        json!({"app_id":app_id,"unit":unit,"result":out}),
    )
    .await;
    Ok(json!({"success":ok,"unit":unit,"result":out}))
}

pub async fn stop_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let unit = unit_name(app_id);
    let out = run_cmd("systemctl", &["disable", "--now", &unit], None, 60).await;
    sqlx::query("UPDATE sk_python_apps SET status='stopped', updated_at=? WHERE id=?")
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":out["success"].as_bool().unwrap_or(false),"unit":unit,"result":out}))
}

pub async fn restart_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let _ = write_unit(
        &get_app(pool, app_id)
            .await?
            .context("python app not found")?,
    )
    .await?;
    let unit = unit_name(app_id);
    let out = run_cmd("systemctl", &["restart", &unit], None, 60).await;
    let ok = out["success"].as_bool().unwrap_or(false);
    sqlx::query("UPDATE sk_python_apps SET status=?, updated_at=? WHERE id=?")
        .bind(if ok { "running" } else { "restart_failed" })
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":ok,"unit":unit,"result":out}))
}

pub async fn status(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let app = get_app(pool, app_id).await?;
    let unit = unit_name(app_id);
    let out = run_cmd("systemctl", &["is-active", &unit], None, 10).await;
    Ok(
        json!({"app":app,"unit":unit,"running":out.get("stdout").and_then(Value::as_str).map(|s|s.trim()=="active").unwrap_or(false),"systemd":out}),
    )
}

pub async fn django_action(pool: &SqlitePool, app_id: &str, action: &str) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    if app["framework"] != json!("django") {
        return Ok(json!({"success":false,"error":"action requires a Django app"}));
    }
    let path = app["path"].as_str().unwrap_or("");
    let python = app
        .get("venv_path")
        .and_then(Value::as_str)
        .map(|v| format!("{v}/bin/python"))
        .unwrap_or_else(|| {
            app["python_version"]
                .as_str()
                .unwrap_or("python3")
                .to_string()
        });
    let args = if action == "migrate" {
        vec!["manage.py", "migrate", "--noinput"]
    } else {
        vec!["manage.py", "collectstatic", "--noinput"]
    };
    let out = run_cmd(&python, &args, Some(path), 600).await;
    let _ = sk_jobs::insert_job(
        pool,
        &format!("python.django.{action}"),
        json!({"app_id":app_id,"result":out}),
    )
    .await;
    Ok(out)
}

pub async fn run_app_command(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let Some(app) = get_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"python app not found"}));
    };
    let command = s(body, "command", "");
    if command.trim().is_empty() {
        return Ok(json!({"success":false,"error":"command is required"}));
    }
    let out = run_cmd(
        "sh",
        &["-lc", command],
        app.get("path").and_then(Value::as_str),
        600,
    )
    .await;
    let _ = sk_jobs::insert_job(
        pool,
        "python.app.run",
        json!({"app_id":app_id,"command":command,"result":out}),
    )
    .await;
    Ok(out)
}

fn node_app_value(row: &sqlx::sqlite::SqliteRow) -> Value {
    json!({
        "id": row.get::<String,_>("id"),
        "name": row.get::<String,_>("name"),
        "path": row.get::<String,_>("path"),
        "node_version": row.get::<String,_>("node_version"),
        "package_manager": row.get::<String,_>("package_manager"),
        "status": row.get::<String,_>("status"),
        "port": row.get::<i64,_>("port"),
        "start_command": row.get::<String,_>("start_command"),
        "metadata": j(row.try_get::<Option<String>,_>("metadata_json").ok().flatten()),
        "created_at": row.get::<String,_>("created_at"),
        "updated_at": row.get::<String,_>("updated_at"),
    })
}

pub async fn node_versions() -> Value {
    let mut runtimes = Vec::new();
    for bin in ["node", "nodejs"] {
        let out = Command::new(bin).arg("--version").output().await;
        if let Ok(out) = out {
            if out.status.success() {
                let version = String::from_utf8_lossy(&out.stdout).trim().to_string();
                if !version.is_empty() && !runtimes.iter().any(|v: &Value| v["path"] == json!(bin))
                {
                    runtimes
                        .push(json!({"binary":bin,"path":bin,"version":version,"installed":true}));
                }
            }
        }
    }
    let mut package_managers = Vec::new();
    for bin in ["npm", "pnpm", "yarn"] {
        let out = Command::new(bin).arg("--version").output().await;
        if let Ok(out) = out {
            if out.status.success() {
                package_managers.push(json!({"binary":bin,"version":String::from_utf8_lossy(&out.stdout).trim(),"installed":true}));
            }
        }
    }
    json!({"versions":runtimes,"default":runtimes.first().cloned(),"package_managers":package_managers,"supported":["18","20","22"]})
}

pub async fn create_node_app(pool: &SqlitePool, body: &Value) -> anyhow::Result<Value> {
    let id = id();
    let name = s(body, "name", "Node.js App");
    let node_version = s(body, "node_version", s(body, "version", "node"));
    let package_manager = s(body, "package_manager", "npm");
    let port = body.get("port").and_then(Value::as_i64).unwrap_or(0);
    let path = opt_s(body, "path")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new("/var/lib/serverkit/node-apps").join(&id));
    let start_command = s(body, "start_command", "npm start");
    tokio::fs::create_dir_all(&path).await?;
    let server_js = format!(
        "const http = require('http');\nconst port = process.env.PORT || {};\nhttp.createServer((req, res) => {{ res.end('Hello from ServerKit Node.js runtime\\n'); }}).listen(port, '127.0.0.1', () => console.log('listening', port));\n",
        if port > 0 { port } else { 3000 }
    );
    if !path.join("server.js").exists() {
        tokio::fs::write(path.join("server.js"), server_js).await?;
    }
    if !path.join("package.json").exists() {
        tokio::fs::write(
            path.join("package.json"),
            format!(
                "{{\n  \"name\": \"{}\",\n  \"version\": \"1.0.0\",\n  \"private\": true,\n  \"scripts\": {{ \"start\": \"node server.js\" }}\n}}\n",
                name.to_ascii_lowercase().replace(|c: char| !c.is_ascii_alphanumeric() && c != '-', "-")
            ),
        )
        .await?;
    }
    let ts = now();
    sqlx::query("INSERT INTO sk_node_apps(id,name,path,node_version,package_manager,status,port,start_command,metadata_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
        .bind(&id).bind(name).bind(path.to_string_lossy().to_string()).bind(node_version).bind(package_manager).bind("created").bind(port).bind(start_command).bind(body.to_string()).bind(&ts).bind(&ts).execute(pool).await?;
    get_node_app(pool, &id)
        .await?
        .context("created node app missing")
}

pub async fn get_node_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Option<Value>> {
    let row = sqlx::query("SELECT * FROM sk_node_apps WHERE id=?")
        .bind(app_id)
        .fetch_optional(pool)
        .await?;
    Ok(row.as_ref().map(node_app_value))
}

pub async fn delete_node_app(
    pool: &SqlitePool,
    app_id: &str,
    remove_files: bool,
) -> anyhow::Result<Value> {
    let Some(app) = get_node_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"node app not found"}));
    };
    let _ = stop_node_app(pool, app_id).await;
    sqlx::query("DELETE FROM sk_node_env WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_node_packages WHERE app_id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    sqlx::query("DELETE FROM sk_node_apps WHERE id=?")
        .bind(app_id)
        .execute(pool)
        .await?;
    if remove_files {
        if let Some(path) = app.get("path").and_then(Value::as_str) {
            let _ = tokio::fs::remove_dir_all(path).await;
        }
    }
    Ok(json!({"success":true,"id":app_id,"removed_files":remove_files}))
}

pub async fn node_packages(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows = sqlx::query("SELECT * FROM sk_node_packages WHERE app_id=? ORDER BY package")
        .bind(app_id)
        .fetch_all(pool)
        .await?;
    Ok(
        json!({"packages":rows.iter().map(|r|json!({"name":r.get::<String,_>("package"),"version":r.try_get::<Option<String>,_>("version").ok().flatten(),"installed_at":r.get::<String,_>("installed_at")})).collect::<Vec<_>>() }),
    )
}

pub async fn install_node_packages(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let Some(app) = get_node_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"node app not found"}));
    };
    let packages: Vec<String> = body
        .get("packages")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    if packages.is_empty() {
        return Ok(json!({"success":false,"error":"packages are required"}));
    }
    let pm = app["package_manager"].as_str().unwrap_or("npm");
    let mut args = if pm == "yarn" {
        vec!["add".to_string()]
    } else {
        vec!["install".to_string()]
    };
    args.extend(packages.iter().cloned());
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let out = run_cmd(pm, &refs, app.get("path").and_then(Value::as_str), 600).await;
    if out["success"].as_bool().unwrap_or(false) {
        let ts = now();
        for package in &packages {
            sqlx::query("INSERT INTO sk_node_packages(app_id,package,installed_at) VALUES(?,?,?) ON CONFLICT(app_id,package) DO UPDATE SET installed_at=excluded.installed_at")
                .bind(app_id).bind(package).bind(&ts).execute(pool).await?;
        }
    }
    Ok(
        json!({"success":out["success"].as_bool().unwrap_or(false),"packages":packages,"result":out}),
    )
}

pub async fn node_env_vars(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let rows =
        sqlx::query("SELECT key,value_encrypted FROM sk_node_env WHERE app_id=? ORDER BY key")
            .bind(app_id)
            .fetch_all(pool)
            .await?;
    let mut obj = serde_json::Map::new();
    for r in rows {
        let key: String = r.get("key");
        let val: String = r.get("value_encrypted");
        obj.insert(
            key,
            json!(sk_core::crypto::decrypt(&val).unwrap_or_default()),
        );
    }
    Ok(json!({"env_vars":Value::Object(obj)}))
}

pub async fn set_node_env_vars(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let vars = body
        .get("env_vars")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let ts = now();
    for (key, value) in vars {
        if !valid_key(&key) {
            continue;
        }
        let raw = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        sqlx::query("INSERT INTO sk_node_env(app_id,key,value_encrypted,updated_at) VALUES(?,?,?,?) ON CONFLICT(app_id,key) DO UPDATE SET value_encrypted=excluded.value_encrypted,updated_at=excluded.updated_at")
            .bind(app_id).bind(key).bind(sk_core::crypto::encrypt(&raw)).bind(&ts).execute(pool).await?;
    }
    node_env_vars(pool, app_id).await
}

pub async fn delete_node_env_var(
    pool: &SqlitePool,
    app_id: &str,
    key: &str,
) -> anyhow::Result<Value> {
    Ok(
        json!({"success":sqlx::query("DELETE FROM sk_node_env WHERE app_id=? AND key=?").bind(app_id).bind(key).execute(pool).await?.rows_affected()>0}),
    )
}

pub async fn set_node_start_command(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let command = s(body, "start_command", s(body, "command", "npm start"));
    sqlx::query("UPDATE sk_node_apps SET start_command=?, updated_at=? WHERE id=?")
        .bind(command)
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":true,"start_command":command}))
}

fn node_unit_name(app_id: &str) -> String {
    format!("serverkit-node-{app_id}.service")
}

async fn write_node_unit(app: &Value) -> anyhow::Result<String> {
    let app_id = app["id"].as_str().unwrap_or("");
    let path = app["path"].as_str().unwrap_or("");
    let unit = node_unit_name(app_id);
    let command = app["start_command"].as_str().unwrap_or("npm start");
    let port = app["port"].as_i64().unwrap_or(0);
    let mut env_line = "Environment=NODE_ENV=production".to_string();
    if port > 0 {
        env_line.push_str(&format!(" PORT={port}"));
    }
    let content = format!("[Unit]\nDescription=ServerKit Node.js app {app_id}\nAfter=network.target\n\n[Service]\nWorkingDirectory={path}\nExecStart=/bin/sh -lc '{command}'\nRestart=on-failure\n{env_line}\n\n[Install]\nWantedBy=multi-user.target\n");
    let unit_path = format!("/etc/systemd/system/{unit}");
    tokio::fs::write(&unit_path, content).await?;
    let _ = run_cmd("systemctl", &["daemon-reload"], None, 30).await;
    Ok(unit)
}

pub async fn start_node_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let Some(app) = get_node_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"node app not found"}));
    };
    let unit = write_node_unit(&app).await?;
    let out = run_cmd("systemctl", &["enable", "--now", &unit], None, 60).await;
    let ok = out["success"].as_bool().unwrap_or(false);
    sqlx::query("UPDATE sk_node_apps SET status=?, updated_at=? WHERE id=?")
        .bind(if ok { "running" } else { "start_failed" })
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    let _ = sk_jobs::insert_job(
        pool,
        "node.app.start",
        json!({"app_id":app_id,"unit":unit,"result":out}),
    )
    .await;
    Ok(json!({"success":ok,"unit":unit,"result":out}))
}

pub async fn stop_node_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let unit = node_unit_name(app_id);
    let out = run_cmd("systemctl", &["disable", "--now", &unit], None, 60).await;
    sqlx::query("UPDATE sk_node_apps SET status='stopped', updated_at=? WHERE id=?")
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":out["success"].as_bool().unwrap_or(false),"unit":unit,"result":out}))
}

pub async fn restart_node_app(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let app = get_node_app(pool, app_id)
        .await?
        .context("node app not found")?;
    let _ = write_node_unit(&app).await?;
    let unit = node_unit_name(app_id);
    let out = run_cmd("systemctl", &["restart", &unit], None, 60).await;
    let ok = out["success"].as_bool().unwrap_or(false);
    sqlx::query("UPDATE sk_node_apps SET status=?, updated_at=? WHERE id=?")
        .bind(if ok { "running" } else { "restart_failed" })
        .bind(now())
        .bind(app_id)
        .execute(pool)
        .await?;
    Ok(json!({"success":ok,"unit":unit,"result":out}))
}

pub async fn node_status(pool: &SqlitePool, app_id: &str) -> anyhow::Result<Value> {
    let app = get_node_app(pool, app_id).await?;
    let unit = node_unit_name(app_id);
    let out = run_cmd("systemctl", &["is-active", &unit], None, 10).await;
    Ok(
        json!({"app":app,"unit":unit,"running":out.get("stdout").and_then(Value::as_str).map(|s|s.trim()=="active").unwrap_or(false),"systemd":out}),
    )
}

pub async fn run_node_command(
    pool: &SqlitePool,
    app_id: &str,
    body: &Value,
) -> anyhow::Result<Value> {
    let Some(app) = get_node_app(pool, app_id).await? else {
        return Ok(json!({"success":false,"error":"node app not found"}));
    };
    let command = s(body, "command", "");
    if command.trim().is_empty() {
        return Ok(json!({"success":false,"error":"command is required"}));
    }
    let out = run_cmd(
        "sh",
        &["-lc", command],
        app.get("path").and_then(Value::as_str),
        600,
    )
    .await;
    let _ = sk_jobs::insert_job(
        pool,
        "node.app.run",
        json!({"app_id":app_id,"command":command,"result":out}),
    )
    .await;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn python_app_roundtrip() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let tmp = std::env::temp_dir().join(format!("sk-python-test-{}", id()));
        let app = create_app(&pool, "flask", &json!({"name":"Demo","path":tmp}))
            .await
            .unwrap();
        let app_id = app["id"].as_str().unwrap();
        set_env_vars(&pool, app_id, &json!({"env_vars":{"A":"B"}}))
            .await
            .unwrap();
        assert_eq!(
            env_vars(&pool, app_id).await.unwrap()["env_vars"]["A"],
            json!("B")
        );
        delete_app(&pool, app_id, true).await.unwrap();
    }

    #[tokio::test]
    async fn node_app_roundtrip() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        ensure_schema(&pool).await.unwrap();
        let tmp = std::env::temp_dir().join(format!("sk-node-test-{}", id()));
        let app = create_node_app(&pool, &json!({"name":"Node Demo","path":tmp,"port":3999}))
            .await
            .unwrap();
        let app_id = app["id"].as_str().unwrap();
        set_node_env_vars(&pool, app_id, &json!({"env_vars":{"NODE_ENV":"test"}}))
            .await
            .unwrap();
        assert_eq!(
            node_env_vars(&pool, app_id).await.unwrap()["env_vars"]["NODE_ENV"],
            json!("test")
        );
        delete_node_app(&pool, app_id, true).await.unwrap();
    }
}
