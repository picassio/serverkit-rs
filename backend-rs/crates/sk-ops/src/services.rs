//! Service-control slice of `ProcessService` (systemctl with `service`
//! fallback) + monitored-service status list.

use crate::run;
use serde_json::{json, Value};

/// `ProcessService.MONITORED_SERVICES`
pub const MONITORED_SERVICES: &[&str] = &[
    "nginx",
    "mysql",
    "mysqld",
    "mariadb",
    "postgresql",
    "postgres",
    "redis",
    "redis-server",
    "docker",
    "dockerd",
    "php-fpm",
    "php-fpm8.2",
    "php-fpm8.1",
    "php-fpm8.0",
    "gunicorn",
    "supervisor",
    "supervisord",
];

const VALID_ACTIONS: &[&str] = &["start", "stop", "restart", "reload", "status"];

/// `ProcessService.control_service()` — systemctl, `service` fallback.
pub async fn control_service(name: &str, action: &str) -> Value {
    if !VALID_ACTIONS.contains(&action) {
        return json!({
            "success": false,
            "error": format!("Invalid action. Must be one of: {VALID_ACTIONS:?}")
        });
    }

    match run("systemctl", &[action, name], 30).await {
        Ok((true, _, _)) => {
            json!({ "success": true, "message": format!("Service {name} {action} successful") })
        }
        Ok((false, _, systemctl_err)) => {
            // Fallback to SysV `service`
            match run("service", &[name, action], 30).await {
                Ok((true, _, _)) => json!({
                    "success": true,
                    "message": format!("Service {name} {action} successful")
                }),
                Ok((false, _, stderr)) => {
                    let err = if stderr.trim().is_empty() {
                        systemctl_err
                    } else {
                        stderr
                    };
                    json!({ "success": false, "error": if err.trim().is_empty() { "Command failed".into() } else { err } })
                }
                Err(e) => json!({ "success": false, "error": e }),
            }
        }
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// `ProcessService.get_services_status()` — process-name matching, same
/// semantics as the psutil scan.
pub fn services_status(running: &[(String, i64)]) -> Vec<Value> {
    MONITORED_SERVICES
        .iter()
        .map(|service| {
            let hit = running
                .iter()
                .find(|(name, _)| name.to_lowercase().contains(&service.to_lowercase()));
            json!({
                "name": service,
                "status": if hit.is_some() { "running" } else { "stopped" },
                "pid": hit.map(|(_, pid)| *pid),
            })
        })
        .collect()
}
