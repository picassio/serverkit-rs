//! `CronService` port — user crontab management with a JSON metadata
//! sidecar (names/descriptions/enabled state), same as Flask's
//! `data/cron_jobs.json`.

use crate::run;
use serde_json::{json, Map, Value};
use std::path::PathBuf;
use tokio::process::Command;

pub const PRESETS: &[(&str, &str)] = &[
    ("every_minute", "* * * * *"),
    ("every_5_minutes", "*/5 * * * *"),
    ("every_15_minutes", "*/15 * * * *"),
    ("every_30_minutes", "*/30 * * * *"),
    ("hourly", "0 * * * *"),
    ("daily", "0 0 * * *"),
    ("daily_midnight", "0 0 * * *"),
    ("daily_noon", "0 12 * * *"),
    ("weekly", "0 0 * * 0"),
    ("monthly", "0 0 1 * *"),
    ("yearly", "0 0 1 1 *"),
];

/// Shell metacharacters banned from cron commands (`BLOCKED_PATTERNS`).
const BLOCKED_PATTERNS: &[&str] = &[";", "&&", "||", "|", "`", "$(", ">", "<", "\n", "\r"];

fn jobs_file() -> PathBuf {
    let dir = std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into());
    PathBuf::from(dir).join("cron_jobs.json")
}

fn load_metadata() -> Value {
    std::fs::read_to_string(jobs_file())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({ "jobs": {} }))
}

fn save_metadata(data: &Value) {
    let file = jobs_file();
    if let Some(parent) = file.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(file, serde_json::to_string_pretty(data).unwrap_or_default());
}

async fn read_crontab() -> String {
    let r = run("crontab", &["-l"], 10).await;
    match r {
        Ok((true, stdout, _)) => stdout,
        _ => String::new(),
    }
}

async fn write_crontab(content: &str) -> Result<(), String> {
    let mut child = Command::new("crontab")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    if let Some(mut pipe) = child.stdin.take() {
        use tokio::io::AsyncWriteExt;
        pipe.write_all(content.as_bytes())
            .await
            .map_err(|e| e.to_string())?;
        drop(pipe);
    }
    let out = child.wait_with_output().await.map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr);
        Err(if stderr.trim().is_empty() {
            "Failed to install crontab".into()
        } else {
            stderr.into_owned()
        })
    }
}

fn describe_schedule(schedule: &str) -> String {
    for (name, preset) in PRESETS {
        if *preset == schedule {
            return name.replace('_', " ");
        }
    }
    schedule.to_string()
}

/// `_validate_schedule` — 5 fields; `*`, `*/n`, digits, ranges, lists.
pub fn validate_schedule(schedule: &str) -> bool {
    let parts: Vec<&str> = schedule.split_whitespace().collect();
    if parts.len() != 5 {
        return false;
    }
    parts.iter().all(|part| {
        part.split(',').all(|p| {
            let p = p.trim();
            p == "*"
                || (p
                    .strip_prefix("*/")
                    .map(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
                    .unwrap_or(false))
                || p.chars().all(|c| c.is_ascii_digit())
                || {
                    // range like 1-5 (optionally /step)
                    let base = p.split('/').next().unwrap_or("");
                    let mut it = base.split('-');
                    match (it.next(), it.next(), it.next()) {
                        (Some(a), Some(b), None) => {
                            !a.is_empty()
                                && !b.is_empty()
                                && a.chars().all(|c| c.is_ascii_digit())
                                && b.chars().all(|c| c.is_ascii_digit())
                        }
                        _ => false,
                    }
                }
        })
    })
}

/// `_validate_command` — absolute path, no shell operators.
pub fn validate_command(command: &str) -> bool {
    if BLOCKED_PATTERNS.iter().any(|p| command.contains(p)) {
        return false;
    }
    command
        .split_whitespace()
        .next()
        .map(|w| w.starts_with('/'))
        .unwrap_or(false)
}

/// `CronService.get_status`
pub async fn status() -> Value {
    let active = run("systemctl", &["is-active", "cron"], 5)
        .await
        .map(|(_, out, _)| out.trim() == "active")
        .unwrap_or(false)
        || run("pgrep", &["-x", "cron"], 5)
            .await
            .map(|(ok, _, _)| ok)
            .unwrap_or(false);
    json!({ "success": true, "running": active, "platform": "linux" })
}

/// `CronService.list_jobs` — crontab lines merged with metadata.
pub async fn list_jobs() -> Value {
    let metadata = load_metadata();
    let meta_jobs = metadata["jobs"].as_object().cloned().unwrap_or_default();
    let crontab = read_crontab().await;

    let mut jobs = Vec::new();
    for (i, line) in crontab.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let fields: Vec<&str> = line
            .splitn(6, char::is_whitespace)
            .filter(|s| !s.is_empty())
            .collect();
        if fields.len() < 6 {
            continue;
        }
        let schedule = fields[..5].join(" ");
        let command = fields[5].to_string();

        // Match metadata by schedule+command to recover name/id
        let matched = meta_jobs.iter().find(|(_, j)| {
            j["schedule"].as_str() == Some(schedule.as_str())
                && j["command"].as_str() == Some(command.as_str())
        });

        jobs.push(json!({
            "id": matched.map(|(id, _)| id.clone()).unwrap_or_else(|| format!("cron_{i}")),
            "name": matched.and_then(|(_, j)| j["name"].as_str()).unwrap_or(&command),
            "schedule": schedule,
            "command": command,
            "minute": fields[0],
            "hour": fields[1],
            "day": fields[2],
            "month": fields[3],
            "weekday": fields[4],
            "enabled": true,
            "description": matched
                .and_then(|(_, j)| j["description"].as_str().map(str::to_string))
                .unwrap_or_else(|| describe_schedule(&schedule)),
            "source": "crontab",
        }));
    }

    // Disabled jobs exist only in metadata (removed from crontab on toggle-off)
    for (id, j) in &meta_jobs {
        if j["enabled"].as_bool() == Some(false) {
            jobs.push(json!({
                "id": id,
                "name": j["name"],
                "schedule": j["schedule"],
                "command": j["command"],
                "enabled": false,
                "description": j["description"],
                "source": "metadata",
            }));
        }
    }

    json!({ "success": true, "jobs": jobs, "count": jobs.len() })
}

/// `CronService.add_job`
pub async fn add_job(
    schedule: &str,
    command: &str,
    name: Option<&str>,
    description: Option<&str>,
) -> Value {
    if !validate_schedule(schedule) {
        return json!({ "success": false, "error": "Invalid cron schedule format" });
    }
    if command.trim().is_empty() {
        return json!({ "success": false, "error": "Command cannot be empty" });
    }
    if !validate_command(command) {
        return json!({ "success": false, "error": "Invalid command: must use absolute paths and cannot contain shell operators (;, &&, ||, |, `, $())" });
    }

    let job_id = format!("job_{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
    let current = read_crontab().await;
    let comment = format!("# ServerKit Job: {}", name.unwrap_or(&job_id));
    let new_crontab = format!("{}\n{comment}\n{schedule} {command}\n", current.trim_end());

    if let Err(e) = write_crontab(&new_crontab).await {
        return json!({ "success": false, "error": e });
    }

    let now = chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string();
    let mut metadata = load_metadata();
    metadata["jobs"][&job_id] = json!({
        "name": name.map(str::to_string).unwrap_or_else(|| format!("Job {job_id}")),
        "schedule": schedule,
        "command": command,
        "description": description.map(str::to_string).unwrap_or_else(|| describe_schedule(schedule)),
        "enabled": true,
        "created_at": now,
        "updated_at": now,
    });
    save_metadata(&metadata);

    json!({ "success": true, "job_id": job_id, "message": "Job created successfully" })
}

fn crontab_without(current: &str, schedule: &str, command: &str, name: &str) -> String {
    let job_line = format!("{schedule} {command}");
    let comment = format!("# ServerKit Job: {name}");
    let lines: Vec<&str> = current
        .lines()
        .filter(|l| l.trim() != job_line && l.trim() != comment)
        .collect();
    format!("{}\n", lines.join("\n").trim_end())
}

/// `CronService.remove_job`
pub async fn remove_job(job_id: &str) -> Value {
    let mut metadata = load_metadata();
    let Some(job) = metadata["jobs"].get(job_id).cloned() else {
        return json!({ "success": false, "error": "Job not found" });
    };
    let current = read_crontab().await;
    let new_crontab = crontab_without(
        &current,
        job["schedule"].as_str().unwrap_or(""),
        job["command"].as_str().unwrap_or(""),
        job["name"].as_str().unwrap_or(job_id),
    );
    if let Err(e) = write_crontab(&new_crontab).await {
        return json!({ "success": false, "error": e });
    }
    metadata["jobs"].as_object_mut().map(|m| m.remove(job_id));
    save_metadata(&metadata);
    json!({ "success": true, "message": "Job removed" })
}

/// `CronService.update_job` — remove + re-add with merged fields.
pub async fn update_job(
    job_id: &str,
    name: Option<&str>,
    command: Option<&str>,
    schedule: Option<&str>,
    description: Option<&str>,
) -> Value {
    let metadata = load_metadata();
    let Some(job) = metadata["jobs"].get(job_id).cloned() else {
        return json!({ "success": false, "error": "Job not found" });
    };
    let new_schedule = schedule.unwrap_or(job["schedule"].as_str().unwrap_or(""));
    let new_command = command.unwrap_or(job["command"].as_str().unwrap_or(""));
    let new_name = name.or(job["name"].as_str());
    let new_desc = description.or(job["description"].as_str());

    let removed = remove_job(job_id).await;
    if !removed["success"].as_bool().unwrap_or(false) {
        return removed;
    }
    add_job(new_schedule, new_command, new_name, new_desc).await
}

/// `CronService.toggle_job` — disabled jobs live only in metadata.
pub async fn toggle_job(job_id: &str, enabled: bool) -> Value {
    let mut metadata = load_metadata();
    let Some(job) = metadata["jobs"].get(job_id).cloned() else {
        return json!({ "success": false, "error": "Job not found" });
    };
    let schedule = job["schedule"].as_str().unwrap_or("").to_string();
    let command = job["command"].as_str().unwrap_or("").to_string();
    let name = job["name"].as_str().unwrap_or(job_id).to_string();

    let current = read_crontab().await;
    let job_line = format!("{schedule} {command}");
    let new_crontab = if enabled {
        if current.lines().any(|l| l.trim() == job_line) {
            current.clone()
        } else {
            format!(
                "{}\n# ServerKit Job: {name}\n{job_line}\n",
                current.trim_end()
            )
        }
    } else {
        crontab_without(&current, &schedule, &command, &name)
    };
    if let Err(e) = write_crontab(&new_crontab).await {
        return json!({ "success": false, "error": e });
    }

    metadata["jobs"][job_id]["enabled"] = json!(enabled);
    metadata["jobs"][job_id]["updated_at"] = json!(chrono::Local::now()
        .naive_local()
        .format("%Y-%m-%dT%H:%M:%S%.6f")
        .to_string());
    save_metadata(&metadata);
    json!({ "success": true, "message": format!("Job {}", if enabled { "enabled" } else { "disabled" }) })
}

/// `CronService.run_job_now` — 60s limit, no shell.
pub async fn run_job_now(job_id: &str) -> Value {
    let metadata = load_metadata();
    let Some(job) = metadata["jobs"].get(job_id) else {
        return json!({ "success": false, "error": "Job not found" });
    };
    let command = job["command"].as_str().unwrap_or("");
    if command.is_empty() {
        return json!({ "success": false, "error": "Job has no command" });
    }
    let parts: Vec<&str> = command.split_whitespace().collect();
    match run(parts[0], &parts[1..], 60).await {
        Ok((ok, stdout, stderr)) => json!({
            "success": true,
            "exit_code": if ok { 0 } else { 1 },
            "stdout": stdout.chars().take(10000).collect::<String>(),
            "stderr": stderr.chars().take(10000).collect::<String>(),
            "message": "Job executed",
        }),
        Err(e) if e.contains("timed out") => {
            json!({ "success": false, "error": "Job execution timed out (60s limit)" })
        }
        Err(e) => json!({ "success": false, "error": e }),
    }
}

/// `CronService.get_presets`
pub fn presets() -> Value {
    let mut map = Map::new();
    for (name, schedule) in PRESETS {
        map.insert(name.to_string(), json!(schedule));
    }
    json!({ "success": true, "presets": map })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schedule_validation() {
        assert!(validate_schedule("* * * * *"));
        assert!(validate_schedule("*/5 0-6 1,15 * 0"));
        assert!(!validate_schedule("* * * *"));
        assert!(!validate_schedule("bad * * * *"));
    }

    #[test]
    fn command_validation() {
        assert!(validate_command(
            "/usr/bin/php /var/www/magento/bin/magento cron:run"
        ));
        assert!(!validate_command("php artisan schedule:run")); // relative
        assert!(!validate_command("/bin/true; rm -rf /"));
        assert!(!validate_command("/bin/echo hi > /tmp/x"));
    }
}
