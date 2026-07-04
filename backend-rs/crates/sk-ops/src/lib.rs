//! sk-ops — log files, journald, and service control.
//! Ports `app/services/log_service.py` and the service-control half of
//! `app/services/process_service.py`.

pub mod cron;
pub mod logs;
pub mod services;

use serde_json::{json, Value};

/// `app/utils/system.py::sourced_result` — the standard shape for
/// fallback-chain endpoints (frontend shows a source-aware banner).
pub(crate) fn sourced_result(lines: Vec<String>, source: &str, source_label: &str) -> Value {
    json!({
        "success": true,
        "count": lines.len(),
        "lines": lines,
        "source": source,
        "source_label": source_label,
    })
}

pub(crate) async fn run(
    cmd: &str,
    args: &[&str],
    timeout_secs: u64,
) -> Result<(bool, String, String), String> {
    let fut = tokio::process::Command::new(cmd).args(args).output();
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut).await {
        Ok(Ok(out)) => Ok((
            out.status.success(),
            String::from_utf8_lossy(&out.stdout).into_owned(),
            String::from_utf8_lossy(&out.stderr).into_owned(),
        )),
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err("Command timed out".into()),
    }
}
