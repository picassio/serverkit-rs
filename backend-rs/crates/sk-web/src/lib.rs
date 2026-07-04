//! sk-web — nginx vhost management and PHP-FPM management.
//! Ports `app/services/nginx_service.py` and `app/services/php_service.py`.

pub mod domains;
pub mod nginx;
pub mod php;

use tokio::process::Command;

pub(crate) struct Cli {
    pub ok: bool,
    pub stdout: String,
    pub stderr: String,
}

fn is_root() -> bool {
    extern "C" {
        fn geteuid() -> u32;
    }
    // SAFETY: geteuid has no preconditions
    unsafe { geteuid() == 0 }
}

/// `run_privileged` — prepend `sudo -n` when not root.
pub(crate) async fn privileged(cmd: &[&str], timeout_secs: u64) -> Cli {
    privileged_with_stdin(cmd, None, timeout_secs).await
}

pub(crate) async fn privileged_with_stdin(
    cmd: &[&str],
    stdin: Option<&str>,
    timeout_secs: u64,
) -> Cli {
    let mut full: Vec<&str> = Vec::new();
    if !is_root() {
        full.extend(["sudo", "-n"]);
    }
    full.extend(cmd);

    let mut command = Command::new(full[0]);
    command.args(&full[1..]);
    if stdin.is_some() {
        command.stdin(std::process::Stdio::piped());
    }
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());

    let run = async {
        let mut child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                return Cli {
                    ok: false,
                    stdout: String::new(),
                    stderr: e.to_string(),
                };
            }
        };
        if let (Some(input), Some(mut pipe)) = (stdin, child.stdin.take()) {
            use tokio::io::AsyncWriteExt;
            let _ = pipe.write_all(input.as_bytes()).await;
            drop(pipe);
        }
        match child.wait_with_output().await {
            Ok(out) => Cli {
                ok: out.status.success(),
                stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
            },
            Err(e) => Cli {
                ok: false,
                stdout: String::new(),
                stderr: e.to_string(),
            },
        }
    };

    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), run).await {
        Ok(cli) => cli,
        Err(_) => Cli {
            ok: false,
            stdout: String::new(),
            stderr: "Command timed out".into(),
        },
    }
}

/// Plain (unprivileged) command.
pub(crate) async fn run(cmd: &[&str], timeout_secs: u64) -> Cli {
    let fut = Command::new(cmd[0]).args(&cmd[1..]).output();
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut).await {
        Ok(Ok(out)) => Cli {
            ok: out.status.success(),
            stdout: String::from_utf8_lossy(&out.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        },
        Ok(Err(e)) => Cli {
            ok: false,
            stdout: String::new(),
            stderr: e.to_string(),
        },
        Err(_) => Cli {
            ok: false,
            stdout: String::new(),
            stderr: "Command timed out".into(),
        },
    }
}

pub(crate) async fn service_is_active(service: &str) -> bool {
    run(&["systemctl", "is-active", service], 10)
        .await
        .stdout
        .trim()
        == "active"
}
