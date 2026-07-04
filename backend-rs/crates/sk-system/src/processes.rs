//! `ProcessService` process listing/details/kill via sysinfo (psutil parity).

use serde_json::{json, Value};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System, Users};

fn refreshed_system() -> System {
    let mut sys = System::new();
    // memory must be refreshed too: total_memory() is the divisor for
    // memory_percent and defaults to 0 on a bare System::new()
    sys.refresh_memory();
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );
    // Second refresh after an interval so cpu_usage() is meaningful.
    std::thread::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL);
    sys.refresh_processes_specifics(
        ProcessesToUpdate::All,
        true,
        ProcessRefreshKind::everything(),
    );
    sys
}

fn username(users: &Users, proc_: &sysinfo::Process) -> String {
    proc_
        .user_id()
        .and_then(|uid| users.get_user_by_id(uid))
        .map(|u| u.name().to_string())
        .unwrap_or_else(|| "unknown".into())
}

fn status_str(proc_: &sysinfo::Process) -> String {
    // psutil uses lowercase status names ('running', 'sleeping', ...)
    proc_.status().to_string().to_lowercase()
}

/// `ProcessService.get_processes(limit, sort_by)` — active processes only,
/// sorted by cpu or memory.
pub async fn list(limit: usize, sort_by: &str) -> Vec<Value> {
    let sort_by = sort_by.to_string();
    tokio::task::spawn_blocking(move || {
        let sys = refreshed_system();
        let users = Users::new_with_refreshed_list();
        let total_mem = sys.total_memory().max(1) as f64;

        let mut procs: Vec<(f64, f64, Value)> = sys
            .processes()
            .values()
            .filter_map(|p| {
                // psutil lists processes only; sysinfo also surfaces Linux
                // tasks (threads) — skip them for parity.
                if p.thread_kind().is_some() {
                    return None;
                }
                let cpu = p.cpu_usage() as f64;
                let mem = p.memory() as f64 / total_mem * 100.0;
                // Flask filters idle processes (cpu>0 or mem>0.1)
                if cpu <= 0.0 && mem <= 0.1 {
                    return None;
                }
                let user = username(&users, p);
                Some((
                    cpu,
                    mem,
                    json!({
                        "pid": p.pid().as_u32(),
                        "name": p.name().to_string_lossy(),
                        "username": user,
                        // Superset over upstream: the Processes tab reads
                        // p.user and p.memory_info.rss, which Flask's list
                        // endpoint never provided (renders 'unknown'/'–'
                        // upstream). Additive fields, no parity break.
                        "user": user,
                        "memory_info": { "rss": p.memory(), "vms": p.virtual_memory() },
                        "cpu_percent": (cpu * 10.0).round() / 10.0,
                        "memory_percent": (mem * 100.0).round() / 100.0,
                        "status": status_str(p),
                        "create_time": p.start_time(),
                    }),
                ))
            })
            .collect();

        if sort_by == "memory" {
            procs.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        } else {
            procs.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        }
        procs.into_iter().take(limit).map(|(_, _, v)| v).collect()
    })
    .await
    .unwrap_or_default()
}

/// `ProcessService.get_process_details(pid)`
pub async fn details(pid: u32) -> Option<Value> {
    tokio::task::spawn_blocking(move || {
        let sys = refreshed_system();
        let users = Users::new_with_refreshed_list();
        let total_mem = sys.total_memory().max(1) as f64;
        let p = sys.process(sysinfo::Pid::from_u32(pid))?;
        let cmdline: Vec<String> = p
            .cmd()
            .iter()
            .map(|s| s.to_string_lossy().into_owned())
            .collect();
        Some(json!({
            "pid": pid,
            "name": p.name().to_string_lossy(),
            "status": status_str(p),
            "username": username(&users, p),
            "cpu_percent": (p.cpu_usage() as f64 * 10.0).round() / 10.0,
            "memory_percent": ((p.memory() as f64 / total_mem * 100.0) * 100.0).round() / 100.0,
            "create_time": p.start_time(),
            "cmdline": cmdline,
        }))
    })
    .await
    .ok()
    .flatten()
}

/// `ProcessService.kill_process(pid, force)` — SIGTERM, SIGKILL when forced.
pub async fn kill(pid: u32, force: bool) -> Value {
    tokio::task::spawn_blocking(move || {
        let sys = refreshed_system();
        let Some(p) = sys.process(sysinfo::Pid::from_u32(pid)) else {
            return json!({ "success": false, "error": format!("Process {pid} not found") });
        };
        let name = p.name().to_string_lossy().into_owned();
        let signal = if force {
            sysinfo::Signal::Kill
        } else {
            sysinfo::Signal::Term
        };
        match p.kill_with(signal) {
            Some(true) => json!({
                "success": true,
                "message": format!("Process {name} (PID: {pid}) terminated")
            }),
            Some(false) => json!({
                "success": false,
                "error": format!("Access denied to kill process {pid}")
            }),
            None => json!({ "success": false, "error": "Signal not supported on this platform" }),
        }
    })
    .await
    .unwrap_or_else(|e| json!({ "success": false, "error": e.to_string() }))
}

/// Running (name, pid) pairs for the monitored-services scan.
pub async fn running_names() -> Vec<(String, i64)> {
    tokio::task::spawn_blocking(|| {
        let mut sys = System::new();
        sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing(),
        );
        sys.processes()
            .values()
            .map(|p| {
                (
                    p.name().to_string_lossy().into_owned(),
                    p.pid().as_u32() as i64,
                )
            })
            .collect()
    })
    .await
    .unwrap_or_default()
}
