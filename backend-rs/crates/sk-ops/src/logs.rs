//! `LogService` port: log file listing, tail/head reads, grep search,
//! journald, truncation. Same path-allowlist protection as Flask.

use crate::{run, sourced_result};
use serde_json::{json, Value};

/// `LogService.LOG_PATHS`
const LOG_PATHS: &[(&str, &str)] = &[
    ("nginx_access", "/var/log/nginx/access.log"),
    ("nginx_error", "/var/log/nginx/error.log"),
    ("php_fpm", "/var/log/php*-fpm.log"),
    ("mysql", "/var/log/mysql/error.log"),
    ("postgresql", "/var/log/postgresql/postgresql-*-main.log"),
    ("syslog", "/var/log/syslog"),
    ("auth", "/var/log/auth.log"),
];

/// `LogService.ALLOWED_LOG_DIRECTORIES` (SERVERKIT_DIR ~ /opt/serverkit is
/// covered by /opt).
const ALLOWED_DIRS: &[&str] = &["/var/log", "/var/www", "/home", "/opt"];

/// Path-traversal protection: resolve symlinks, then prefix-match.
pub fn is_path_allowed(filepath: &str) -> bool {
    match std::fs::canonicalize(filepath) {
        Ok(real) => {
            let real = real.to_string_lossy();
            ALLOWED_DIRS.iter().any(|d| real.starts_with(d))
        }
        Err(_) => false,
    }
}

fn format_size(bytes: u64) -> String {
    let mut val = bytes as f64;
    for unit in ["B", "KB", "MB", "GB"] {
        if val < 1024.0 {
            return format!("{val:.1} {unit}");
        }
        val /= 1024.0;
    }
    format!("{val:.1} TB")
}

/// `LogService.get_log_files()`
pub fn log_files() -> Vec<Value> {
    let mut logs = Vec::new();
    for (name, pattern) in LOG_PATHS {
        let Ok(paths) = glob::glob(pattern) else {
            continue;
        };
        for path in paths.flatten() {
            let Ok(meta) = std::fs::metadata(&path) else {
                continue;
            };
            let modified = meta
                .modified()
                .ok()
                .map(|t| {
                    chrono::DateTime::<chrono::Local>::from(t)
                        .naive_local()
                        .format("%Y-%m-%dT%H:%M:%S%.6f")
                        .to_string()
                })
                .unwrap_or_default();
            logs.push(json!({
                "name": name,
                "path": path.to_string_lossy(),
                "size": meta.len(),
                "size_human": format_size(meta.len()),
                "modified": modified,
            }));
        }
    }
    logs
}

/// `LogService.read_log()` — tail/head with Rust-I/O fallback.
pub async fn read_log(filepath: &str, lines: i64, from_end: bool) -> Value {
    if !is_path_allowed(filepath) {
        return json!({ "success": false, "error": "Access denied: path not in allowed directories" });
    }
    if !std::path::Path::new(filepath).exists() {
        return json!({ "success": false, "error": "Log file not found" });
    }

    let tool = if from_end { "tail" } else { "head" };
    match run(tool, &["-n", &lines.to_string(), filepath], 30).await {
        Ok((true, stdout, _)) => {
            let log_lines: Vec<String> = stdout.split('\n').map(str::to_string).collect();
            let mut result = sourced_result(log_lines, tool, tool);
            result["filepath"] = json!(filepath);
            result
        }
        Ok((false, _, stderr)) => json!({ "success": false, "error": stderr }),
        Err(_) => {
            // Fallback: direct read (tail/head unavailable)
            match std::fs::read_to_string(filepath) {
                Ok(content) => {
                    let all: Vec<&str> = content.lines().collect();
                    let n = lines.max(0) as usize;
                    let slice: Vec<String> = if from_end {
                        all.iter()
                            .rev()
                            .take(n)
                            .rev()
                            .map(|s| s.to_string())
                            .collect()
                    } else {
                        all.iter().take(n).map(|s| s.to_string()).collect()
                    };
                    let mut result = sourced_result(slice, "rust", "direct file read");
                    result["filepath"] = json!(filepath);
                    result
                }
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            }
        }
    }
}

/// `LogService.search_log()` — case-insensitive grep, max N matches.
pub async fn search_log(filepath: &str, pattern: &str, lines: i64) -> Value {
    if !is_path_allowed(filepath) {
        return json!({ "success": false, "error": "Access denied: path not in allowed directories" });
    }
    if !std::path::Path::new(filepath).exists() {
        return json!({ "success": false, "error": "Log file not found" });
    }

    match run(
        "grep",
        &["-i", "-m", &lines.to_string(), pattern, filepath],
        60,
    )
    .await
    {
        // grep exit 1 = no matches, not an error
        Ok((ok, stdout, stderr)) => {
            if ok || stdout.is_empty() && stderr.is_empty() {
                let matches: Vec<&str> = stdout.lines().filter(|l| !l.is_empty()).collect();
                json!({
                    "success": true,
                    "matches": matches,
                    "count": matches.len(),
                    "pattern": pattern,
                })
            } else {
                json!({ "success": false, "error": stderr })
            }
        }
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// `LogService.get_journalctl_logs()` (journalctl source; syslog fallback).
pub async fn journal_logs(
    unit: Option<&str>,
    lines: i64,
    since: Option<&str>,
    priority: Option<&str>,
) -> Value {
    let lines_s = lines.to_string();
    let mut args: Vec<&str> = vec!["-n", &lines_s, "--no-pager", "-o", "short-iso"];
    if let Some(u) = unit {
        args.extend(["-u", u]);
    }
    if let Some(s) = since {
        args.extend(["--since", s]);
    }
    if let Some(p) = priority {
        args.extend(["-p", p]);
    }

    match run("journalctl", &args, 60).await {
        Ok((true, stdout, _)) => {
            let log_lines: Vec<String> = stdout.split('\n').map(str::to_string).collect();
            sourced_result(log_lines, "journalctl", "systemd journal")
        }
        Ok((false, _, stderr)) => json!({ "success": false, "error": stderr }),
        Err(_) => {
            // Syslog fallback
            read_log("/var/log/syslog", lines, true).await
        }
    }
}

/// `LogService.clear_log()` — truncate to zero.
pub async fn clear_log(filepath: &str) -> Value {
    if !is_path_allowed(filepath) {
        return json!({ "success": false, "error": "Access denied: path not in allowed directories" });
    }
    if !std::path::Path::new(filepath).exists() {
        return json!({ "success": false, "error": "Log file not found" });
    }
    match run("truncate", &["-s", "0", filepath], 15).await {
        Ok((true, _, _)) => {
            json!({ "success": true, "message": format!("Log file {filepath} cleared") })
        }
        Ok((false, _, stderr)) => json!({ "success": false, "error": stderr }),
        Err(e) => json!({ "success": false, "error": e }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowlist_blocks_traversal() {
        assert!(!is_path_allowed("/etc/shadow"));
        assert!(!is_path_allowed("/var/log/../../etc/passwd"));
    }
}
