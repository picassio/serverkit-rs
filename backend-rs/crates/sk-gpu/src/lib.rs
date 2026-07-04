use serde_json::{json, Value};

fn run(cmd: &str, args: &[&str]) -> Value {
    match std::process::Command::new(cmd).args(args).output() {
        Ok(o) => json!({
            "success": o.status.success(),
            "stdout": String::from_utf8_lossy(&o.stdout).trim(),
            "stderr": String::from_utf8_lossy(&o.stderr).trim(),
        }),
        Err(e) => json!({"success": false, "error": e.to_string()}),
    }
}
fn exists(cmd: &str) -> bool {
    std::process::Command::new("sh")
        .args(["-c", &format!("command -v {cmd}")])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn info() -> Value {
    let nvidia = if exists("nvidia-smi") {
        run(
            "nvidia-smi",
            &[
                "--query-gpu=index,name,uuid,memory.total,driver_version",
                "--format=csv,noheader",
            ],
        )
    } else {
        json!({"success": false, "error": "nvidia-smi not installed"})
    };
    let rocm = if exists("rocm-smi") {
        run("rocm-smi", &["--showproductname", "--showmeminfo", "vram"])
    } else {
        json!({"success": false, "error": "rocm-smi not installed"})
    };
    let lspci = if exists("lspci") {
        run("lspci", &["-nn"])
    } else {
        json!({"success": false, "error": "lspci not installed"})
    };
    let mut gpus = Vec::new();
    if let Some(out) = nvidia.get("stdout").and_then(Value::as_str) {
        for line in out.lines().filter(|l| !l.trim().is_empty()) {
            let parts: Vec<_> = line.split(',').map(str::trim).collect();
            gpus.push(json!({"vendor":"nvidia","index":parts.first().copied().unwrap_or(""),"name":parts.get(1).copied().unwrap_or(""),"uuid":parts.get(2).copied().unwrap_or(""),"memory_total":parts.get(3).copied().unwrap_or(""),"driver":parts.get(4).copied().unwrap_or("")}));
        }
    }
    if gpus.is_empty() {
        if let Some(out) = lspci.get("stdout").and_then(Value::as_str) {
            for line in out.lines().filter(|l| {
                let x = l.to_lowercase();
                x.contains(" vga ")
                    || x.contains("3d controller")
                    || x.contains("display controller")
            }) {
                gpus.push(json!({"vendor":"pci","raw":line}));
            }
        }
    }
    json!({
        "available": !gpus.is_empty(),
        "gpus": gpus,
        "tools": {"nvidia_smi": exists("nvidia-smi"), "rocm_smi": exists("rocm-smi"), "lspci": exists("lspci")},
        "nvidia": nvidia,
        "rocm": rocm,
        "lspci": lspci,
    })
}
