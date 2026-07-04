//! Quick actions + health — the per-store dashboard operations.
//! Whitelisted `bin/magento` commands only; nothing user-supplied reaches argv.

use crate::store::Store;
use serde_json::{json, Value};
use std::process::Stdio;
use tokio::process::Command;

/// action id → (argv, timeout_secs). The Magento ops set from the plan.
pub const ACTIONS: &[(&str, &[&str], u64)] = &[
    ("cache-flush", &["cache:flush"], 120),
    ("cache-clean", &["cache:clean"], 120),
    ("cache-status", &["cache:status"], 60),
    ("reindex", &["indexer:reindex"], 1800),
    ("indexer-status", &["indexer:status"], 60),
    (
        "setup-upgrade",
        &["setup:upgrade", "--keep-generated"],
        1800,
    ),
    ("di-compile", &["setup:di:compile"], 1800),
    (
        "static-deploy",
        &["setup:static-content:deploy", "-f"],
        1800,
    ),
    ("maintenance-enable", &["maintenance:enable"], 60),
    ("maintenance-disable", &["maintenance:disable"], 60),
    ("maintenance-status", &["maintenance:status"], 60),
    ("mode-show", &["deploy:mode:show"], 60),
    ("mode-developer", &["deploy:mode:set", "developer"], 600),
    ("mode-production", &["deploy:mode:set", "production"], 3600),
    ("cron-run", &["cron:run"], 600),
];

pub fn list_actions() -> Value {
    json!(ACTIONS
        .iter()
        .map(|(id, argv, _)| json!({
            "id": id,
            "command": format!("bin/magento {}", argv.join(" ")),
        }))
        .collect::<Vec<_>>())
}

async fn bin_magento(s: &Store, args: &[&str], timeout_secs: u64) -> Value {
    let src = format!("{}/src", s.root_path);
    let php = format!("php{}", s.php_version);
    let magento = format!("{src}/bin/magento");

    let mut cmd = Command::new(&php);
    cmd.arg(&magento)
        .args(args)
        .arg("--no-ansi")
        .current_dir(&src)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let fut = cmd.output();
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut).await {
        Ok(Ok(out)) => json!({
            "success": out.status.success(),
            "exit_code": out.status.code().unwrap_or(-1),
            "output": String::from_utf8_lossy(&out.stdout),
            "stderr": String::from_utf8_lossy(&out.stderr),
        }),
        Ok(Err(e)) => json!({ "success": false, "error": e.to_string() }),
        Err(_) => {
            json!({ "success": false, "error": format!("action timed out after {timeout_secs}s") })
        }
    }
}

/// Run a whitelisted action.
pub async fn run_action(s: &Store, action: &str) -> Value {
    let Some((_, argv, timeout)) = ACTIONS.iter().find(|(id, _, _)| *id == action) else {
        return json!({
            "success": false,
            "error": format!("Unknown action: {action}"),
            "available": ACTIONS.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
        });
    };
    bin_magento(s, argv, *timeout).await
}

/// Store health: data-plane containers, cron backlog, indexers, mode.
pub async fn health(s: &Store) -> Value {
    // containers
    let compose_path = format!("{}/docker-compose.yml", s.root_path);
    let services = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_path,
            "ps",
            "--format",
            "{{json .}}",
        ])
        .output()
        .await
        .ok()
        .map(|out| {
            String::from_utf8_lossy(&out.stdout)
                .lines()
                .filter_map(|l| serde_json::from_str::<Value>(l).ok())
                .map(|c| {
                    json!({
                        "service": c["Service"],
                        "state": c["State"],
                        "health": c["Health"],
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    // cron_schedule status via the store DB container
    let db_container = format!("magento-{}-db", s.name);
    let cron_q = "SELECT status, COUNT(*) FROM cron_schedule \
                  WHERE scheduled_at > NOW() - INTERVAL 1 HOUR GROUP BY status;";
    let cron_out = Command::new("docker")
        .args([
            "exec",
            "-e",
            &format!("MYSQL_PWD={}", s.db_password_plain().unwrap_or_default()),
            &db_container,
            "mariadb",
            "-u",
            "magento",
            "-D",
            "magento",
            "--batch",
            "-e",
            cron_q,
        ])
        .output()
        .await;
    let cron: Value = match cron_out {
        Ok(out) if out.status.success() => {
            let mut m = serde_json::Map::new();
            for line in String::from_utf8_lossy(&out.stdout).lines().skip(1) {
                let mut parts = line.split('\t');
                if let (Some(status), Some(count)) = (parts.next(), parts.next()) {
                    m.insert(status.to_string(), json!(count.parse::<i64>().unwrap_or(0)));
                }
            }
            json!({ "available": true, "last_hour": m })
        }
        _ => json!({ "available": false }),
    };

    // indexer status
    let idx = bin_magento(s, &["indexer:status"], 60).await;
    let indexers: Vec<Value> = idx["output"]
        .as_str()
        .unwrap_or("")
        .lines()
        .filter(|l| l.contains('|'))
        .skip(2) // header + separator
        .filter_map(|l| {
            let cols: Vec<&str> = l.split('|').map(str::trim).collect();
            (cols.len() >= 4 && !cols[1].is_empty() && cols[1] != "ID").then(|| {
                json!({
                    "id": cols[1],
                    "title": cols[2],
                    "status": cols[3],
                })
            })
        })
        .collect();

    json!({
        "store": s.name,
        "status": s.status,
        "services": services,
        "cron": cron,
        "indexers": indexers,
    })
}

/// Tail of the provisioning log.
pub fn provision_log(s: &Store, lines: usize) -> Value {
    let path = format!("{}/provision.log", s.root_path);
    let content = std::fs::read_to_string(&path).unwrap_or_default();
    let all: Vec<&str> = content.lines().collect();
    let tail = &all[all.len().saturating_sub(lines)..];
    json!({ "lines": tail, "count": tail.len() })
}
