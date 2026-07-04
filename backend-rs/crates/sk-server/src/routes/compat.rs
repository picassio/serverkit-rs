//! Compatibility routes for full-ServerKit UI pages whose backends were not
//! ported. Each returns a shape the page's loader accepts so the page renders
//! cleanly (no 404s). Where a page maps onto capabilities we DO have, the
//! route serves real data (Domains -> nginx vhosts, Firewall -> ufw). The rest
//! return valid empty state until the subsystem is implemented.

use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
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
