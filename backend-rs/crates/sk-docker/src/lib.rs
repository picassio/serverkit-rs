//! sk-docker — Docker management, ported from `app/services/docker_service.py`.
//!
//! Deliberately shells out to the `docker` CLI with `--format '{{json .}}'`
//! exactly like the Flask oracle, so response shapes match by construction
//! (the CLI JSON keys — ID/Names/Status/... — are the contract the frontend
//! consumes). Streaming stats/logs can move to bollard later without
//! changing these shapes.

use serde_json::{json, Map, Value};
use tokio::process::Command;

/// ServerKit's own containers must never be lifecycle-controlled from the
/// panel (`DockerService.PROTECTED_CONTAINER_NAMES`).
const PROTECTED_CONTAINER_NAMES: &[&str] = &[
    "serverkit-frontend",
    "serverkit_frontend",
    "serverkit-backend",
    "serverkit_backend",
    "serverkit",
];

pub fn is_protected_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let normalized = name.to_lowercase().replace('/', "");
    PROTECTED_CONTAINER_NAMES
        .iter()
        .any(|p| normalized.contains(p))
}

pub async fn is_protected_container(container_id: &str) -> bool {
    let name = inspect_container(container_id)
        .await
        .and_then(|c| c.get("Name").and_then(|n| n.as_str()).map(str::to_string))
        .unwrap_or_default();
    is_protected_name(&name) || is_protected_name(container_id)
}

struct Cli {
    ok: bool,
    stdout: String,
    stderr: String,
    code: i32,
}

async fn docker(args: &[&str]) -> Cli {
    match Command::new("docker").args(args).output().await {
        Ok(out) => Cli {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            code: out.status.code().unwrap_or(-1),
        },
        Err(e) => Cli {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
            code: -1,
        },
    }
}

/// Parse `--format '{{json .}}'` line-delimited output.
fn json_lines(stdout: &str) -> Vec<Value> {
    stdout
        .lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

fn ok_or_err(r: Cli) -> Value {
    if r.ok {
        json!({ "success": true })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

// ==================== STATUS / INFO ====================

/// `DockerService.is_docker_installed()`
pub async fn status() -> Value {
    let r = docker(&["version", "--format", "json"]).await;
    if r.ok {
        match serde_json::from_str::<Value>(&r.stdout) {
            Ok(info) => json!({ "installed": true, "info": info }),
            Err(e) => json!({ "installed": false, "error": e.to_string() }),
        }
    } else {
        json!({ "installed": false, "error": r.stderr })
    }
}

/// `DockerService.get_docker_info()`
pub async fn info() -> Option<Value> {
    let r = docker(&["info", "--format", "{{json .}}"]).await;
    if r.ok {
        serde_json::from_str(&r.stdout).ok()
    } else {
        None
    }
}

/// `DockerService.get_disk_usage()`
pub async fn disk_usage() -> Vec<Value> {
    let r = docker(&["system", "df", "--format", "{{json .}}"]).await;
    if r.ok {
        json_lines(&r.stdout)
    } else {
        Vec::new()
    }
}

// ==================== CONTAINERS ====================

/// `DockerService.list_containers()`
pub async fn list_containers(all: bool) -> Vec<Value> {
    let args: &[&str] = if all {
        &["ps", "-a", "--format", "{{json .}}"]
    } else {
        &["ps", "--format", "{{json .}}"]
    };
    let r = docker(args).await;
    if !r.ok {
        return Vec::new();
    }
    json_lines(&r.stdout)
        .into_iter()
        .map(|c| {
            let name = c.get("Names").and_then(|v| v.as_str()).unwrap_or("");
            json!({
                "id": c.get("ID"),
                "name": name,
                "image": c.get("Image"),
                "status": c.get("Status"),
                "state": c.get("State"),
                "ports": c.get("Ports"),
                "created": c.get("CreatedAt"),
                "size": c.get("Size"),
                "protected": is_protected_name(name),
            })
        })
        .collect()
}

/// `DockerService.get_container()` — full `docker inspect` object.
pub async fn inspect_container(container_id: &str) -> Option<Value> {
    let r = docker(&["inspect", container_id]).await;
    if !r.ok {
        return None;
    }
    serde_json::from_str::<Vec<Value>>(&r.stdout)
        .ok()?
        .into_iter()
        .next()
}

pub struct ContainerSpec {
    pub image: String,
    pub name: Option<String>,
    pub ports: Vec<String>,
    pub volumes: Vec<String>,
    pub env: Map<String, Value>,
    pub network: Option<String>,
    pub restart_policy: Option<String>,
    pub command: Option<String>,
}

fn container_args(base: &str, spec: &ContainerSpec, detach: bool) -> Vec<String> {
    let mut cmd: Vec<String> = vec![base.to_string()];
    if detach && base == "run" {
        cmd.push("-d".into());
    }
    if let Some(name) = &spec.name {
        cmd.extend(["--name".into(), name.clone()]);
    }
    for p in &spec.ports {
        cmd.extend(["-p".into(), p.clone()]);
    }
    for v in &spec.volumes {
        cmd.extend(["-v".into(), v.clone()]);
    }
    for (k, v) in &spec.env {
        let val = v
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| v.to_string());
        cmd.extend(["-e".into(), format!("{k}={val}")]);
    }
    if let Some(n) = &spec.network {
        cmd.extend(["--network".into(), n.clone()]);
    }
    if let Some(r) = &spec.restart_policy {
        cmd.extend(["--restart".into(), r.clone()]);
    }
    cmd.push(spec.image.clone());
    if let Some(c) = &spec.command {
        cmd.extend(shell_words::split(c).unwrap_or_else(|_| vec![c.clone()]));
    }
    cmd
}

async fn docker_owned(args: Vec<String>) -> Cli {
    let refs: Vec<&str> = args.iter().map(String::as_str).collect();
    docker(&refs).await
}

/// `DockerService.create_container()`
pub async fn create_container(spec: &ContainerSpec) -> Value {
    let r = docker_owned(container_args("create", spec, false)).await;
    if r.ok {
        json!({ "success": true, "container_id": r.stdout.trim() })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `DockerService.run_container()`
pub async fn run_container(spec: &ContainerSpec) -> Value {
    let r = docker_owned(container_args("run", spec, true)).await;
    if r.ok {
        json!({ "success": true, "container_id": r.stdout.trim() })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

pub async fn start_container(id: &str) -> Value {
    ok_or_err(docker(&["start", id]).await)
}

pub async fn stop_container(id: &str, timeout: i64) -> Value {
    ok_or_err(docker(&["stop", "-t", &timeout.to_string(), id]).await)
}

pub async fn restart_container(id: &str, timeout: i64) -> Value {
    ok_or_err(docker(&["restart", "-t", &timeout.to_string(), id]).await)
}

pub async fn remove_container(id: &str, force: bool, volumes: bool) -> Value {
    let mut args = vec!["rm"];
    if force {
        args.push("-f");
    }
    if volumes {
        args.push("-v");
    }
    args.push(id);
    ok_or_err(docker(&args).await)
}

/// `DockerService.get_container_logs()` — stdout+stderr combined, like Flask.
pub async fn container_logs(id: &str, tail: i64, since: Option<&str>) -> Value {
    let tail_s = tail.to_string();
    let mut args = vec!["logs", "--tail", &tail_s];
    if let Some(s) = since {
        args.extend(["--since", s]);
    }
    args.push("-t");
    args.push(id);
    let r = docker(&args).await;
    json!({ "success": true, "logs": format!("{}{}", r.stdout, r.stderr) })
}

/// `DockerService.get_container_stats()`
pub async fn container_stats(id: &str) -> Option<Value> {
    let r = docker(&["stats", "--no-stream", "--format", "{{json .}}", id]).await;
    if r.ok && !r.stdout.trim().is_empty() {
        serde_json::from_str(r.stdout.trim()).ok()
    } else {
        None
    }
}

/// `DockerService.get_containers_stats()` — keyed by ID, Container and Name.
pub async fn containers_stats(ids: &[String]) -> Value {
    let cleaned: Vec<&str> = ids
        .iter()
        .map(String::as_str)
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return json!({});
    }
    let mut args = vec!["stats", "--no-stream", "--format", "{{json .}}"];
    args.extend(&cleaned);
    let r = docker(&args).await;
    if !r.ok {
        tracing::error!(stderr = %r.stderr.trim(), "failed to get container stats");
        return json!({});
    }
    let mut map = Map::new();
    for stats in json_lines(&r.stdout) {
        for key in ["ID", "Container", "Name"] {
            if let Some(k) = stats.get(key).and_then(|v| v.as_str()) {
                if !k.is_empty() {
                    map.insert(k.to_string(), stats.clone());
                }
            }
        }
    }
    Value::Object(map)
}

/// `DockerService.exec_command()` — 60s timeout like Flask.
pub async fn exec_command(id: &str, command: &str) -> Value {
    let parts = match shell_words::split(command) {
        Ok(p) => p,
        Err(e) => return json!({ "success": false, "error": e.to_string() }),
    };
    let mut args: Vec<String> = vec!["exec".into(), id.to_string()];
    args.extend(parts);

    let fut = docker_owned(args);
    match tokio::time::timeout(std::time::Duration::from_secs(60), fut).await {
        Ok(r) => json!({
            "success": r.ok,
            "stdout": r.stdout,
            "stderr": r.stderr,
            "return_code": r.code,
        }),
        Err(_) => json!({ "success": false, "error": "Command timed out" }),
    }
}

// ==================== IMAGES ====================

/// `DockerService.list_images()`
pub async fn list_images() -> Vec<Value> {
    let r = docker(&["images", "--format", "{{json .}}"]).await;
    if !r.ok {
        return Vec::new();
    }
    json_lines(&r.stdout)
        .into_iter()
        .map(|i| {
            json!({
                "id": i.get("ID"),
                "repository": i.get("Repository"),
                "tag": i.get("Tag"),
                "size": i.get("Size"),
                "created": i.get("CreatedAt"),
            })
        })
        .collect()
}

/// `DockerService.pull_image()` (anonymous pull; registry login is P2).
pub async fn pull_image(image: &str, tag: &str) -> Value {
    let full = if tag.is_empty() {
        image.to_string()
    } else {
        format!("{image}:{tag}")
    };
    let r = docker(&["pull", &full]).await;
    if r.ok {
        json!({ "success": true, "output": r.stdout })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

pub async fn remove_image(id: &str, force: bool) -> Value {
    let mut args = vec!["rmi"];
    if force {
        args.push("-f");
    }
    args.push(id);
    ok_or_err(docker(&args).await)
}

pub async fn tag_image(source: &str, target: &str) -> Value {
    ok_or_err(docker(&["tag", source, target]).await)
}

// ==================== NETWORKS ====================

/// `DockerService.list_networks()`
pub async fn list_networks() -> Vec<Value> {
    let r = docker(&["network", "ls", "--format", "{{json .}}"]).await;
    if !r.ok {
        return Vec::new();
    }
    json_lines(&r.stdout)
        .into_iter()
        .map(|n| {
            json!({
                "id": n.get("ID"),
                "name": n.get("Name"),
                "driver": n.get("Driver"),
                "scope": n.get("Scope"),
            })
        })
        .collect()
}

pub async fn create_network(name: &str, driver: &str) -> Value {
    let r = docker(&["network", "create", "--driver", driver, name]).await;
    if r.ok {
        json!({ "success": true, "network_id": r.stdout.trim() })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

pub async fn remove_network(id: &str) -> Value {
    ok_or_err(docker(&["network", "rm", id]).await)
}

// ==================== VOLUMES ====================

/// `DockerService.list_volumes()`
pub async fn list_volumes() -> Vec<Value> {
    let r = docker(&["volume", "ls", "--format", "{{json .}}"]).await;
    if !r.ok {
        return Vec::new();
    }
    json_lines(&r.stdout)
        .into_iter()
        .map(|v| {
            json!({
                "name": v.get("Name"),
                "driver": v.get("Driver"),
                "mountpoint": v.get("Mountpoint"),
            })
        })
        .collect()
}

pub async fn create_volume(name: &str, driver: &str) -> Value {
    let r = docker(&["volume", "create", "--driver", driver, name]).await;
    if r.ok {
        json!({ "success": true, "volume_name": r.stdout.trim() })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

pub async fn remove_volume(name: &str, force: bool) -> Value {
    let mut args = vec!["volume", "rm"];
    if force {
        args.push("-f");
    }
    args.push(name);
    ok_or_err(docker(&args).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_names() {
        assert!(is_protected_name("/serverkit-backend"));
        assert!(is_protected_name("myproj_serverkit_frontend_1"));
        assert!(!is_protected_name("magento-redis"));
        assert!(!is_protected_name(""));
    }
}
