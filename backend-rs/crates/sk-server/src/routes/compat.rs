//! Compatibility routes for full-ServerKit UI pages whose backends were not
//! ported. Each returns a shape the page's loader accepts so the page renders
//! cleanly (no 404s). Where a page maps onto capabilities we DO have, the
//! route serves real data (Domains -> nginx vhosts, Firewall -> ufw). The rest
//! return valid empty state until the subsystem is implemented.

use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        // ── Apps / Services: real — Magento stores + installed template apps ─
        .route("/apps", get(apps_list))
        .route("/apps/{id}", get(app_detail))
        .route("/apps/{id}/start", post(app_start))
        .route("/apps/{id}/stop", post(app_stop))
        .route("/apps/{id}/restart", post(app_restart))
        // ── Domains: real, from nginx vhosts ────────────────────────────
        .route("/domains", get(domains))
        // ── Firewall: real, from ufw ────────────────────────────────────
        .route("/firewall/status", get(firewall_status))
        .route("/firewall/rules", get(firewall_rules).post(firewall_add_rule).delete(firewall_del_rule))
        .route("/firewall/enable", post(firewall_enable))
        .route("/firewall/disable", post(firewall_disable))
        .route("/firewall/blocked-ips", get(async |AuthUser(_u): AuthUser| Json(json!({ "blocked": [] }))))
        // ── Marketplace registry (remote packages) ──────────────────────
        .route("/marketplace/registry", get(async |AuthUser(_u): AuthUser| Json(json!({ "items": [] }))))
        // ── Backups: real — Magento database backups ────────────────────
        .route("/backups", get(backups_list))
        .route("/backups/config", get(async |AuthUser(_u): AuthUser| Json(json!({ "enabled": false, "schedule": null, "retention_days": 7, "targets": [] }))))
        .route("/backups/cost-rates", get(async |AuthUser(_u): AuthUser| Json(json!({ "rates": [] }))))
        .route("/backups/cost-summary", get(async |AuthUser(_u): AuthUser| Json(json!({ "total_usd": 0, "items": [] }))))
        .route("/backups/schedules", get(async |AuthUser(_u): AuthUser| Json(json!({ "schedules": [] }))))
        .route("/backups/stats", get(backups_stats))
        // ── Deployments ────────────────────────────────────────────────
        .route("/deployment-jobs", get(async |AuthUser(_u): AuthUser| Json(json!({ "jobs": [] }))))
        // ── DNS / registrars ────────────────────────────────────────────
        .route("/dns/portfolio", get(async |AuthUser(_u): AuthUser| Json(json!({ "domains": [] }))))
        .route("/registrars/domains", get(async |AuthUser(_u): AuthUser| Json(json!({ "domains": [] }))))
        // ── Plugins / extensions ────────────────────────────────────────
        .route("/plugins", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/builtin", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/updates", get(async |AuthUser(_u): AuthUser| Json(json!({ "updates": [] }))))
        // ── Security ────────────────────────────────────────────────────
        .route("/security/status", get(async |AuthUser(_u): AuthUser| Json(json!({ "score": 0, "max_score": 0, "checks": [] }))))
        .route("/security/clamav/status", get(async |AuthUser(_u): AuthUser| Json(json!({ "installed": false, "running": false, "last_scan": null, "definitions": null }))))
    // (servers/* live in the nested servers router to avoid a nest conflict)
}

/// GET /domains — map nginx vhosts to the Domains page's domain objects.
async fn domains(AuthUser(_u): AuthUser) -> Json<Value> {
    let sites = sk_web::nginx::list_sites();
    let domains: Vec<Value> = sites
        .into_iter()
        .map(|s| {
            let list = s.get("domains").cloned().unwrap_or_else(|| json!([]));
            let primary = list
                .as_array()
                .and_then(|a| a.first())
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            json!({
                "name": s.get("name").and_then(|v| v.as_str()).unwrap_or(""),
                "domain": primary,
                "domains": list,
                "enabled": s.get("enabled").and_then(|v| v.as_bool()).unwrap_or(false),
                "ssl": s.get("ssl").and_then(|v| v.as_bool()).unwrap_or(false),
                "root": s.get("root").cloned().unwrap_or(Value::Null),
                "source": "nginx",
            })
        })
        .collect();
    Json(json!({ "domains": domains }))
}

fn ufw(args: &[&str]) -> Option<String> {
    std::process::Command::new("ufw")
        .args(args)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).to_string())
}

/// GET /firewall/status — ufw status.
async fn firewall_status(AuthUser(_u): AuthUser) -> Json<Value> {
    let installed = which_ufw();
    let out = ufw(&["status", "verbose"]).unwrap_or_default();
    let enabled = out.contains("Status: active");
    let default_line = out
        .lines()
        .find(|l| l.starts_with("Default:"))
        .unwrap_or("");
    Json(json!({
        "installed": installed,
        "enabled": enabled,
        "default": default_line.trim_start_matches("Default:").trim(),
        "logging": out.lines().find(|l| l.starts_with("Logging:")).map(|l| l.trim_start_matches("Logging:").trim()).unwrap_or("unknown"),
    }))
}

/// GET /firewall/rules — parse `ufw status numbered`.
async fn firewall_rules(AuthUser(_u): AuthUser) -> Json<Value> {
    let out = ufw(&["status", "numbered"]).unwrap_or_default();
    let mut rules = Vec::new();
    for line in out.lines() {
        let l = line.trim();
        // lines look like: [ 1] 22/tcp   ALLOW IN  Anywhere
        if let Some(rest) = l.strip_prefix('[') {
            if let Some((num, body)) = rest.split_once(']') {
                let body = body.trim();
                let action = if body.contains("ALLOW") {
                    "allow"
                } else if body.contains("DENY") {
                    "deny"
                } else if body.contains("REJECT") {
                    "reject"
                } else {
                    ""
                };
                rules.push(json!({
                    "index": num.trim().parse::<u32>().unwrap_or(0),
                    "action": action,
                    "raw": body,
                }));
            }
        }
    }
    Json(json!({ "rules": rules }))
}

fn which_ufw() -> bool {
    std::process::Command::new("sh")
        .args(["-c", "command -v ufw"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn ufw_run(args: &[String]) -> Json<Value> {
    let out = std::process::Command::new("ufw").args(args).output();
    match out {
        Ok(o) if o.status.success() => {
            Json(json!({ "success": true, "output": String::from_utf8_lossy(&o.stdout).trim() }))
        }
        Ok(o) => {
            Json(json!({ "success": false, "error": String::from_utf8_lossy(&o.stderr).trim() }))
        }
        Err(e) => Json(json!({ "success": false, "error": e.to_string() })),
    }
}

fn admin_or(u: &sk_models::user::User) -> Option<Json<Value>> {
    (!u.is_admin()).then(|| Json(json!({ "success": false, "error": "Admin access required" })))
}

async fn firewall_enable(AuthUser(u): AuthUser) -> Json<Value> {
    if let Some(e) = admin_or(&u) {
        return e;
    }
    // SAFETY: allow SSH + the panel port BEFORE enabling, or a default-deny
    // policy locks the operator (and this panel) out of the box.
    let port = std::env::var("PORT").unwrap_or_else(|_| "5000".into());
    for spec in ["22/tcp".to_string(), format!("{port}/tcp")] {
        let _ = std::process::Command::new("ufw")
            .args(["allow", &spec])
            .status();
    }
    ufw_run(&["--force".into(), "enable".into()])
}
async fn firewall_disable(AuthUser(u): AuthUser) -> Json<Value> {
    if let Some(e) = admin_or(&u) {
        return e;
    }
    ufw_run(&["disable".into()])
}

/// POST /firewall/rules — build a ufw spec from {action, port, protocol, from}.
async fn firewall_add_rule(AuthUser(u): AuthUser, Json(b): Json<Value>) -> Json<Value> {
    if let Some(e) = admin_or(&u) {
        return e;
    }
    let action = b["action"].as_str().unwrap_or("allow");
    if !matches!(action, "allow" | "deny" | "reject" | "limit") {
        return Json(json!({ "success": false, "error": "invalid action" }));
    }
    let mut args = vec![action.to_string()];
    if let Some(from) = b["from"].as_str().filter(|x| !x.is_empty()) {
        args.push("from".into());
        args.push(from.to_string());
    }
    let port = b["port"]
        .as_str()
        .map(|s| s.to_string())
        .or_else(|| b["port"].as_i64().map(|n| n.to_string()));
    if let Some(port) = port.filter(|p| !p.is_empty()) {
        let proto = b["protocol"]
            .as_str()
            .filter(|p| matches!(*p, "tcp" | "udp"));
        // `ufw allow from <ip> to any port <p>` vs `ufw allow <p>/tcp`
        if b["from"].as_str().map(|x| !x.is_empty()).unwrap_or(false) {
            args.push("to".into());
            args.push("any".into());
            args.push("port".into());
            args.push(port);
            if let Some(p) = proto {
                args.push("proto".into());
                args.push(p.into());
            }
        } else {
            args.push(match proto {
                Some(p) => format!("{port}/{p}"),
                None => port,
            });
        }
    }
    ufw_run(&args)
}

/// DELETE /firewall/rules — delete by {index}/{number}.
async fn firewall_del_rule(AuthUser(u): AuthUser, Json(b): Json<Value>) -> Json<Value> {
    if let Some(e) = admin_or(&u) {
        return e;
    }
    let idx = b["index"]
        .as_u64()
        .or_else(|| b["number"].as_u64())
        .or_else(|| b["index"].as_str().and_then(|s| s.parse().ok()));
    match idx {
        Some(n) => ufw_run(&["--force".into(), "delete".into(), n.to_string()]),
        None => Json(json!({ "success": false, "error": "rule index required" })),
    }
}

// ── Apps / Services: Magento stores + installed template apps ───────────────
fn apps_dir() -> String {
    std::env::var("SK_APPS_DIR").unwrap_or_else(|_| "/var/www/serverkit-apps".into())
}

/// Running if `docker compose ps -q` lists any container.
fn compose_status(compose: &str) -> &'static str {
    let out = std::process::Command::new("docker")
        .args(["compose", "-f", compose, "ps", "-q"])
        .output();
    match out {
        Ok(o) if o.status.success() && !o.stdout.trim_ascii().is_empty() => "running",
        _ => "stopped",
    }
}

async fn collect_apps(s: &SharedState) -> Vec<Value> {
    let mut apps = Vec::new();
    if let Ok(stores) = sk_magento::store::list(&s.db).await {
        for st in stores {
            apps.push(json!({
                "id": format!("magento-{}", st.id),
                "name": st.name,
                "app_type": "magento",
                "status": st.status,
                "domains": [st.domain],
                "root_path": st.root_path,
                "source": "magento",
                "version": st.magento_version,
                "project_name": Value::Null, "environment_name": Value::Null,
                "last_deploy_at": Value::Null, "deploy_repo_url": Value::Null,
                "upload_path": Value::Null,
            }));
        }
    }
    let dir = apps_dir();
    for app in sk_templates::list_installed() {
        let name = app
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let compose = format!("{dir}/{name}/docker-compose.yml");
        apps.push(json!({
            "id": format!("app-{}", name),
            "name": name,
            "app_type": "docker",
            "status": compose_status(&compose),
            "domains": app.get("domains").cloned().unwrap_or_else(|| json!([])),
            "root_path": format!("{dir}/{name}"),
            "source": "template",
            "version": app.get("template").cloned().unwrap_or(Value::Null),
            "project_name": Value::Null, "environment_name": Value::Null,
            "last_deploy_at": Value::Null, "deploy_repo_url": Value::Null,
            "upload_path": Value::Null,
        }));
    }
    apps
}

async fn apps_list(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "apps": collect_apps(&s).await }))
}

async fn app_detail(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> Json<Value> {
    let app = collect_apps(&s)
        .await
        .into_iter()
        .find(|a| a["id"] == json!(id));
    Json(app.unwrap_or_else(|| json!({ "error": "not found" })))
}

/// Resolve an app id to its docker-compose.yml path.
async fn app_compose(s: &SharedState, id: &str) -> Option<String> {
    if let Some(sid) = id.strip_prefix("magento-") {
        let sid: i64 = sid.parse().ok()?;
        let stores = sk_magento::store::list(&s.db).await.ok()?;
        let st = stores.into_iter().find(|x| x.id == sid)?;
        Some(format!("{}/docker-compose.yml", st.root_path))
    } else {
        id.strip_prefix("app-")
            .map(|name| format!("{}/{}/docker-compose.yml", apps_dir(), name))
    }
}

async fn app_compose_action(s: &SharedState, id: &str, action: &str) -> Json<Value> {
    let Some(compose) = app_compose(s, id).await else {
        return Json(json!({ "success": false, "error": "unknown app" }));
    };
    let out = std::process::Command::new("docker")
        .args(["compose", "-f", &compose, action])
        .output();
    match out {
        Ok(o) if o.status.success() => Json(
            json!({ "success": true, "status": if action == "stop" { "stopped" } else { "running" } }),
        ),
        Ok(o) => Json(json!({ "success": false, "error": String::from_utf8_lossy(&o.stderr) })),
        Err(e) => Json(json!({ "success": false, "error": e.to_string() })),
    }
}

async fn app_start(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> Json<Value> {
    app_compose_action(&s, &id, "start").await
}
async fn app_stop(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> Json<Value> {
    app_compose_action(&s, &id, "stop").await
}
async fn app_restart(
    State(s): State<SharedState>,
    AuthUser(_u): AuthUser,
    Path(id): Path<String>,
) -> Json<Value> {
    app_compose_action(&s, &id, "restart").await
}

// ── Backups: real Magento database backups ──────────────────────────────────
async fn collect_backups(s: &SharedState) -> Vec<Value> {
    let mut out = Vec::new();
    if let Ok(stores) = sk_magento::store::list(&s.db).await {
        for st in &stores {
            for b in sk_magento::backup::list_backups(st) {
                out.push(json!({
                    "name": b.get("filename").cloned().unwrap_or(Value::Null),
                    "type": "database",
                    "path": b.get("path").cloned().unwrap_or(Value::Null),
                    "size": b.get("size").cloned().unwrap_or(json!(0)),
                    "size_human": b.get("size_human").cloned().unwrap_or(Value::Null),
                    "timestamp": b.get("created_at").cloned().unwrap_or(Value::Null),
                    "app_name": st.name.clone(),
                    "remote_status": Value::Null,
                }));
            }
        }
    }
    out
}

async fn backups_list(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> Json<Value> {
    Json(json!({ "backups": collect_backups(&s).await }))
}

async fn backups_stats(State(s): State<SharedState>, AuthUser(_u): AuthUser) -> Json<Value> {
    let backups = collect_backups(&s).await;
    let total_size: u64 = backups.iter().filter_map(|b| b["size"].as_u64()).sum();
    let last = backups
        .iter()
        .filter_map(|b| b["timestamp"].as_str())
        .max()
        .map(|s| s.to_string());
    Json(json!({
        "total_backups": backups.len(),
        "total_size_bytes": total_size,
        "last_backup": last,
    }))
}
