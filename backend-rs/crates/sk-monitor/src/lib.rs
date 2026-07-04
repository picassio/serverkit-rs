//! sk-monitor — threshold alerting on host metrics. Ports
//! `app/services/monitoring_service.py`: JSON config (thresholds, webhook,
//! email), stateless `check_thresholds`, JSONL alert history, webhook
//! delivery. The evaluator loop lives in the server (it owns the metrics).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

pub const DEFAULT_CHECK_INTERVAL: u64 = 60;
/// Don't re-fire the same alert type within this window (`ALERT_COOLDOWN`).
pub const ALERT_COOLDOWN_SECS: i64 = 300;

fn data_dir() -> PathBuf {
    PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
}
fn config_path() -> PathBuf {
    data_dir().join("alerts.json")
}
fn history_path() -> PathBuf {
    data_dir().join("alerts.log")
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Thresholds {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub disk_percent: f64,
    pub load_average: f64,
}
impl Default for Thresholds {
    fn default() -> Self {
        // DEFAULT_THRESHOLDS
        Self {
            cpu_percent: 80.0,
            memory_percent: 85.0,
            disk_percent: 90.0,
            load_average: 5.0,
        }
    }
}

/// Current metrics snapshot the evaluator hands us (extracted from sk-system).
pub struct Snapshot {
    pub cpu_percent: f64,
    pub memory_percent: f64,
    pub disk_percent: f64,
    pub load_1min: f64,
}

/// `MonitoringService.get_config` shape (defaults when the file is absent).
pub fn get_config() -> Value {
    if let Ok(s) = std::fs::read_to_string(config_path()) {
        if let Ok(v) = serde_json::from_str::<Value>(&s) {
            return v;
        }
    }
    json!({
        "enabled": false,
        "check_interval": DEFAULT_CHECK_INTERVAL,
        "thresholds": Thresholds::default(),
        "webhook": { "enabled": false, "url": "" },
        "email": {
            "enabled": false, "smtp_host": "", "smtp_port": 587,
            "smtp_user": "", "smtp_password": "", "from_email": "", "to_emails": []
        }
    })
}

pub fn save_config(config: &Value) -> Value {
    let path = config_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::write(
        &path,
        serde_json::to_string_pretty(config).unwrap_or_default(),
    ) {
        Ok(_) => json!({ "success": true, "message": "Configuration saved" }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

pub fn thresholds() -> Thresholds {
    serde_json::from_value(get_config()["thresholds"].clone()).unwrap_or_default()
}

/// `MonitoringService.check_thresholds` — stateless breach list.
pub fn check_thresholds(m: &Snapshot, t: &Thresholds) -> Vec<Value> {
    let mut alerts = Vec::new();
    let sev = |v: f64| if v < 95.0 { "warning" } else { "critical" };

    if m.cpu_percent > t.cpu_percent {
        alerts.push(json!({
            "type": "cpu", "severity": sev(m.cpu_percent),
            "message": format!("CPU usage at {:.1}% (threshold: {}%)", m.cpu_percent, t.cpu_percent),
            "value": m.cpu_percent, "threshold": t.cpu_percent,
        }));
    }
    if m.memory_percent > t.memory_percent {
        alerts.push(json!({
            "type": "memory", "severity": sev(m.memory_percent),
            "message": format!("Memory usage at {:.1}% (threshold: {}%)", m.memory_percent, t.memory_percent),
            "value": m.memory_percent, "threshold": t.memory_percent,
        }));
    }
    if m.disk_percent > t.disk_percent {
        alerts.push(json!({
            "type": "disk", "severity": sev(m.disk_percent),
            "message": format!("Disk usage at {:.1}% (threshold: {}%)", m.disk_percent, t.disk_percent),
            "value": m.disk_percent, "threshold": t.disk_percent,
        }));
    }
    if m.load_1min > t.load_average {
        alerts.push(json!({
            "type": "load", "severity": "warning",
            "message": format!("Load average at {:.2} (threshold: {})", m.load_1min, t.load_average),
            "value": m.load_1min, "threshold": t.load_average,
        }));
    }
    alerts
}

/// Append fired alerts to the JSONL history log (one object per alert).
pub fn record_alerts(alerts: &[Value]) {
    let path = history_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let now = chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S")
        .to_string();
    let mut buf = String::new();
    for a in alerts {
        let mut obj = a.clone();
        obj["timestamp"] = json!(now);
        buf.push_str(&serde_json::to_string(&obj).unwrap_or_default());
        buf.push('\n');
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        use std::io::Write;
        let _ = f.write_all(buf.as_bytes());
    }
}

/// `MonitoringService.get_alert_history` — newest first.
pub fn history(limit: usize) -> Vec<Value> {
    let Ok(content) = std::fs::read_to_string(history_path()) else {
        return Vec::new();
    };
    let mut lines: Vec<Value> = content
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect();
    let start = lines.len().saturating_sub(limit);
    let mut recent: Vec<Value> = lines.split_off(start);
    recent.reverse();
    recent
}

pub fn clear_history() -> Value {
    match std::fs::write(history_path(), "") {
        Ok(_) => json!({ "success": true }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

/// POST the alerts to a webhook URL as `{alerts, hostname, timestamp}`.
pub async fn deliver_webhook(url: &str, alerts: &[Value], hostname: &str) -> Value {
    let payload = json!({
        "alerts": alerts,
        "hostname": hostname,
        "timestamp": chrono::Local::now().naive_local().format("%Y-%m-%dT%H:%M:%S").to_string(),
    });
    match reqwest::Client::new()
        .post(url)
        .json(&payload)
        .timeout(std::time::Duration::from_secs(15))
        .send()
        .await
    {
        Ok(r) => json!({ "success": r.status().is_success(), "status": r.status().as_u16() }),
        Err(e) => json!({ "success": false, "error": e.to_string() }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn breaches() {
        let t = Thresholds::default();
        let m = Snapshot {
            cpu_percent: 96.0,
            memory_percent: 50.0,
            disk_percent: 91.0,
            load_1min: 1.0,
        };
        let a = check_thresholds(&m, &t);
        assert_eq!(a.len(), 2); // cpu(critical) + disk
        assert_eq!(a[0]["type"], "cpu");
        assert_eq!(a[0]["severity"], "critical");
        assert_eq!(a[1]["type"], "disk");
    }

    #[test]
    fn no_breach() {
        let t = Thresholds::default();
        let m = Snapshot {
            cpu_percent: 10.0,
            memory_percent: 20.0,
            disk_percent: 30.0,
            load_1min: 0.5,
        };
        assert!(check_thresholds(&m, &t).is_empty());
    }
}
