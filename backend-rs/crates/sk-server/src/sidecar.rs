//! AI sidecar supervisor. Spawns the bundled Node pi-SDK sidecar
//! (`ai-sidecar/start.sh`) on boot, waits for it to become healthy, and wires
//! `SK_SIDECAR_URL` / `SK_SIDECAR_TOKEN` into the process env so the `/ai/*`
//! routes use it — no manual setup. Returns the child (killed on drop).
//!
//! Opt-outs:
//! - `SK_SIDECAR_URL` already set  → assume an externally-managed sidecar.
//! - `SK_SIDECAR_AUTOSTART=0|false` → disable.
//! - `SK_SIDECAR_DIR`               → override the sidecar directory.

use std::path::PathBuf;
use std::process::Stdio;

/// Locate the `ai-sidecar` directory (must contain `start.sh`).
fn find_dir() -> Option<PathBuf> {
    if let Ok(d) = std::env::var("SK_SIDECAR_DIR") {
        let p = PathBuf::from(d);
        if p.join("start.sh").exists() {
            return Some(p);
        }
    }
    let mut candidates: Vec<PathBuf> = vec![
        PathBuf::from("ai-sidecar"),
        PathBuf::from("backend-rs/ai-sidecar"),
        PathBuf::from("../ai-sidecar"),
    ];
    // relative to the running executable: <exe_dir>/../../ai-sidecar etc.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for up in [
                "ai-sidecar",
                "../ai-sidecar",
                "../../ai-sidecar",
                "../../../ai-sidecar",
            ] {
                candidates.push(dir.join(up));
            }
        }
    }
    candidates.into_iter().find(|p| p.join("start.sh").exists())
}

fn random_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    (0..32)
        .map(|_| format!("{:x}", rng.gen_range(0..16)))
        .collect()
}

/// Spawn + health-wait the sidecar. Returns the child handle (kill on drop) on
/// success, or `None` if disabled/unavailable (the AI route then falls back to
/// the native pi CLI path).
pub async fn autostart() -> Option<tokio::process::Child> {
    if std::env::var("SK_SIDECAR_URL")
        .ok()
        .filter(|s| !s.is_empty())
        .is_some()
    {
        tracing::info!("SK_SIDECAR_URL set — using externally-managed AI sidecar");
        return None;
    }
    match std::env::var("SK_SIDECAR_AUTOSTART").as_deref() {
        Ok("0") | Ok("false") | Ok("no") => {
            tracing::info!("AI sidecar autostart disabled");
            return None;
        }
        _ => {}
    }
    let Some(dir) = find_dir() else {
        tracing::warn!(
            "AI sidecar directory not found — assistant will use the native pi CLI path"
        );
        return None;
    };

    let port: u16 = std::env::var("SK_SIDECAR_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(5056);
    let token = std::env::var("SK_SIDECAR_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
        .unwrap_or_else(random_token);
    let url = format!("http://127.0.0.1:{port}");

    tracing::info!(dir = %dir.display(), %port, "starting AI sidecar");
    // Build via std Command so we can install PR_SET_PDEATHSIG: the child (and
    // its exec'd node) receives SIGKILL when this process dies — even on
    // SIGKILL of the parent — so no orphaned sidecar survives us.
    let mut std_cmd = std::process::Command::new("bash");
    std_cmd
        .arg(dir.join("start.sh"))
        .current_dir(&dir)
        .env("SK_SIDECAR_PORT", port.to_string())
        .env("SK_SIDECAR_TOKEN", &token)
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(target_os = "linux")]
    unsafe {
        use std::os::unix::process::CommandExt;
        std_cmd.pre_exec(|| {
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
    let child = tokio::process::Command::from(std_cmd)
        .kill_on_drop(true)
        .spawn();

    let child = match child {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "failed to spawn AI sidecar (is `bash`/`node` installed?) — falling back to native pi CLI");
            return None;
        }
    };

    // Health-wait: first run may `npm install`, so allow generous time.
    let health = format!("{url}/health");
    let client = reqwest::Client::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let mut healthy = false;
    while std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if let Ok(resp) = client
            .get(&health)
            .header("x-sk-sidecar-token", &token)
            .send()
            .await
        {
            if resp.status().is_success() {
                healthy = true;
                break;
            }
        }
    }

    if !healthy {
        tracing::warn!("AI sidecar did not become healthy in time — falling back to native pi CLI");
        return None; // child dropped -> killed
    }

    // Wire the routes to use it.
    std::env::set_var("SK_SIDECAR_URL", &url);
    std::env::set_var("SK_SIDECAR_TOKEN", &token);
    tracing::info!(%url, "AI sidecar ready (pi SDK)");
    Some(child)
}
