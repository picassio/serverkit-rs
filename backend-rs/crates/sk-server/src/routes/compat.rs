//! Compatibility routes for full-ServerKit UI pages whose backends were not
//! ported. Each returns a shape the page's loader accepts so the page renders
//! cleanly (no 404s). Where a page maps onto capabilities we DO have, the
//! route serves real data (Domains -> nginx vhosts, Firewall -> ufw). The rest
//! return valid empty state until the subsystem is implemented.

use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        // ── Domains: real, from nginx vhosts ────────────────────────────
        .route("/domains", get(domains))
        // ── Firewall: real, from ufw ────────────────────────────────────
        .route("/firewall/status", get(firewall_status))
        .route("/firewall/rules", get(firewall_rules))
        .route("/firewall/blocked-ips", get(async |AuthUser(_u): AuthUser| Json(json!({ "blocked": [] }))))
        // ── Marketplace registry (remote packages) ──────────────────────
        .route("/marketplace/registry", get(async |AuthUser(_u): AuthUser| Json(json!({ "items": [] }))))
        // ── Backups (no backup subsystem yet) ───────────────────────────
        .route("/backups", get(async |AuthUser(_u): AuthUser| Json(json!({ "backups": [] }))))
        .route("/backups/config", get(async |AuthUser(_u): AuthUser| Json(json!({ "enabled": false, "schedule": null, "retention_days": 7, "targets": [] }))))
        .route("/backups/cost-rates", get(async |AuthUser(_u): AuthUser| Json(json!({ "rates": [] }))))
        .route("/backups/cost-summary", get(async |AuthUser(_u): AuthUser| Json(json!({ "total_usd": 0, "items": [] }))))
        .route("/backups/schedules", get(async |AuthUser(_u): AuthUser| Json(json!({ "schedules": [] }))))
        .route("/backups/stats", get(async |AuthUser(_u): AuthUser| Json(json!({ "total_backups": 0, "total_size_bytes": 0, "last_backup": null }))))
        // ── Deployments / jobs / queue ──────────────────────────────────
        .route("/deployment-jobs", get(async |AuthUser(_u): AuthUser| Json(json!({ "jobs": [] }))))
        .route("/jobs", get(async |AuthUser(_u): AuthUser| Json(json!({ "jobs": [], "total": 0 }))))
        .route("/jobs/kinds", get(async |AuthUser(_u): AuthUser| Json(json!({ "kinds": [] }))))
        .route("/jobs/scheduled", get(async |AuthUser(_u): AuthUser| Json(json!({ "scheduled": [] }))))
        .route("/jobs/stats", get(async |AuthUser(_u): AuthUser| Json(json!({ "total": 0, "running": 0, "queued": 0, "failed": 0, "completed": 0 }))))
        .route("/queue/groups", get(async |AuthUser(_u): AuthUser| Json(json!({ "groups": [] }))))
        .route("/queue/stats", get(async |AuthUser(_u): AuthUser| Json(json!({ "total": 0, "pending": 0, "active": 0, "completed": 0, "failed": 0 }))))
        // ── DNS / registrars ────────────────────────────────────────────
        .route("/dns/portfolio", get(async |AuthUser(_u): AuthUser| Json(json!({ "domains": [] }))))
        .route("/registrars/domains", get(async |AuthUser(_u): AuthUser| Json(json!({ "domains": [] }))))
        // ── Plugins / extensions ────────────────────────────────────────
        .route("/plugins", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/builtin", get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))))
        .route("/plugins/updates", get(async |AuthUser(_u): AuthUser| Json(json!({ "updates": [] }))))
        // ── Projects / workspaces ───────────────────────────────────────
        .route("/projects", get(async |AuthUser(_u): AuthUser| Json(json!({ "projects": [] }))))
        // ── Security ────────────────────────────────────────────────────
        .route("/security/status", get(async |AuthUser(_u): AuthUser| Json(json!({ "score": 0, "max_score": 0, "checks": [] }))))
        .route("/security/clamav/status", get(async |AuthUser(_u): AuthUser| Json(json!({ "installed": false, "running": false, "last_scan": null, "definitions": null }))))
        // (servers/* live in the nested servers router to avoid a nest conflict)
        // ── Webhooks ────────────────────────────────────────────────────
        .route("/webhooks/endpoints", get(async |AuthUser(_u): AuthUser| Json(json!({ "endpoints": [] }))))
}

/// GET /domains — map nginx vhosts to the Domains page's domain objects.
async fn domains(AuthUser(_u): AuthUser) -> Json<Value> {
    let sites = sk_web::nginx::list_sites();
    let domains: Vec<Value> = sites
        .into_iter()
        .map(|s| {
            let list = s.get("domains").cloned().unwrap_or_else(|| json!([]));
            let primary = list.as_array().and_then(|a| a.first()).and_then(|d| d.as_str()).unwrap_or("").to_string();
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
    let default_line = out.lines().find(|l| l.starts_with("Default:")).unwrap_or("");
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
                let action = if body.contains("ALLOW") { "allow" } else if body.contains("DENY") { "deny" } else if body.contains("REJECT") { "reject" } else { "" };
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
    std::process::Command::new("sh").args(["-c", "command -v ufw"]).output().map(|o| o.status.success()).unwrap_or(false)
}
