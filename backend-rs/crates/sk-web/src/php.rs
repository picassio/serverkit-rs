//! `PHPService` port — multi-version PHP-FPM management.
//!
//! DIVERGENCE (Magento fork): SUPPORTED_VERSIONS extends upstream's
//! 8.0–8.3 with 7.4 (Magento 2.3.x/2.4.0–2.4.3) and 8.4 (Magento 2.4.8),
//! matching the magento-vm-provisioner PHP matrix. Both exist in ondrej/php.

use crate::{privileged, privileged_with_stdin, run, service_is_active};
use serde_json::{json, Map, Value};
use std::path::Path;

pub const SUPPORTED_VERSIONS: &[&str] = &["7.4", "8.0", "8.1", "8.2", "8.3", "8.4"];

const POOL_TEMPLATE: &str = r#"[{pool_name}]
user = {user}
group = {group}
listen = /run/php/php{version}-fpm-{pool_name}.sock
listen.owner = www-data
listen.group = www-data
listen.mode = 0660

pm = {pm_type}
pm.max_children = {max_children}
pm.start_servers = {start_servers}
pm.min_spare_servers = {min_spare}
pm.max_spare_servers = {max_spare}
pm.max_requests = {max_requests}

; Logging
php_admin_value[error_log] = /var/log/php/{pool_name}.error.log
php_admin_flag[log_errors] = on

; Security
php_admin_value[open_basedir] = {open_basedir}
php_admin_value[disable_functions] = {disable_functions}

; Performance
php_value[max_execution_time] = {max_execution_time}
php_value[max_input_time] = {max_input_time}
php_value[memory_limit] = {memory_limit}
php_value[post_max_size] = {post_max_size}
php_value[upload_max_filesize] = {upload_max_filesize}

; OPcache
php_value[opcache.enable] = {opcache_enable}
php_value[opcache.memory_consumption] = {opcache_memory}
php_value[opcache.max_accelerated_files] = {opcache_files}

; Environment
env[PATH] = /usr/local/bin:/usr/bin:/bin
env[TMP] = /tmp
env[TMPDIR] = /tmp
env[TEMP] = /tmp
"#;

fn pool_dir(version: &str) -> String {
    format!("/etc/php/{version}/fpm/pool.d")
}

fn fpm_service(version: &str) -> String {
    format!("php{version}-fpm")
}

fn valid_version(version: &str) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}

/// `PHPService.get_installed_versions`
pub async fn installed_versions() -> Vec<Value> {
    let mut versions = Vec::new();
    for version in SUPPORTED_VERSIONS {
        let php_bin = format!("/usr/bin/php{version}");
        if !Path::new(&php_bin).exists() {
            continue;
        }
        let full_version = run(&[&php_bin, "-v"], 10)
            .await
            .stdout
            .lines()
            .next()
            .unwrap_or(version)
            .to_string();
        let fpm_installed = Path::new(&format!("/usr/sbin/php-fpm{version}")).exists();
        let fpm_running = if fpm_installed {
            service_is_active(&fpm_service(version)).await
        } else {
            false
        };
        versions.push(json!({
            "version": version,
            "full_version": full_version,
            "cli_path": php_bin,
            "fpm_installed": fpm_installed,
            "fpm_running": fpm_running,
            "fpm_service": fpm_service(version),
        }));
    }
    versions
}

/// `PHPService.get_default_version`
pub async fn default_version() -> Option<String> {
    let r = run(&["php", "-v"], 10).await;
    if !r.ok {
        return None;
    }
    let re = regex::Regex::new(r"PHP (\d+\.\d+)").unwrap();
    re.captures(&r.stdout).map(|c| c[1].to_string())
}

/// `PHPService.set_default_version` — update-alternatives.
pub async fn set_default_version(version: &str) -> Value {
    if !valid_version(version) {
        return json!({ "success": false, "error": format!("Unsupported PHP version: {version}") });
    }
    let php_bin = format!("/usr/bin/php{version}");
    if !Path::new(&php_bin).exists() {
        return json!({ "success": false, "error": format!("PHP {version} is not installed") });
    }
    let r = privileged(&["update-alternatives", "--set", "php", &php_bin], 30).await;
    if r.ok {
        json!({ "success": true, "message": format!("Default PHP version set to {version}") })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `PHPService.install_version` — ondrej PPA + apt install with the common
/// extension set (the Magento-required set is a superset installed later
/// by sk-magento).
pub async fn install_version(version: &str) -> Value {
    if !valid_version(version) {
        return json!({ "success": false, "error": format!("Unsupported PHP version: {version}") });
    }

    privileged(&["add-apt-repository", "-y", "ppa:ondrej/php"], 120).await;
    privileged(&["apt-get", "update"], 120).await;

    let packages: Vec<String> = [
        "fpm", "cli", "common", "mysql", "xml", "xmlrpc", "curl", "gd", "imagick", "mbstring",
        "opcache", "soap", "zip", "intl", "bcmath",
    ]
    .iter()
    .map(|ext| format!("php{version}-{ext}"))
    .collect();

    let mut cmd: Vec<&str> = vec!["apt-get", "install", "-y"];
    cmd.extend(packages.iter().map(String::as_str));
    let r = privileged(&cmd, 600).await;

    if r.ok {
        let svc = fpm_service(version);
        privileged(&["systemctl", "enable", &svc], 30).await;
        privileged(&["systemctl", "start", &svc], 30).await;
        json!({ "success": true, "message": format!("PHP {version} installed successfully") })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `PHPService.get_extensions` — `php -m`.
pub async fn extensions(version: &str) -> Vec<Value> {
    let php_bin = format!("/usr/bin/php{version}");
    if !Path::new(&php_bin).exists() {
        return Vec::new();
    }
    let r = run(&[&php_bin, "-m"], 10).await;
    if !r.ok {
        return Vec::new();
    }
    r.stdout
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('['))
        .map(|ext| json!({ "name": ext, "enabled": true }))
        .collect()
}

/// `PHPService.install_extension`
pub async fn install_extension(version: &str, extension: &str) -> Value {
    if !valid_version(version) || !extension.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return json!({ "success": false, "error": "Invalid version or extension name" });
    }
    let package = format!("php{version}-{extension}");
    let r = privileged(&["apt-get", "install", "-y", &package], 120).await;
    if r.ok {
        restart_fpm(version).await;
        json!({ "success": true, "message": format!("Extension {extension} installed") })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `PHPService.get_pools`
pub fn pools(version: &str) -> Vec<Value> {
    let dir = pool_dir(version);
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut pools = Vec::new();
    for entry in entries.flatten() {
        let filename = entry.file_name().to_string_lossy().into_owned();
        let Some(pool_name) = filename.strip_suffix(".conf") else {
            continue;
        };
        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let mut config = Map::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with(';') {
                continue;
            }
            if let Some((k, v)) = line.split_once('=') {
                config.insert(k.trim().to_string(), json!(v.trim()));
            }
        }
        pools.push(json!({
            "name": pool_name,
            "file": entry.path().to_string_lossy(),
            "user": config.get("user").cloned().unwrap_or(json!("www-data")),
            "listen": config.get("listen").cloned().unwrap_or(json!("")),
            "pm": config.get("pm").cloned().unwrap_or(json!("dynamic")),
            "max_children": config.get("pm.max_children").cloned().unwrap_or(json!("5")),
        }));
    }
    pools
}

/// `PHPService.create_pool`
pub async fn create_pool(version: &str, pool_name: &str, config: &Map<String, Value>) -> Value {
    if !valid_version(version) {
        return json!({ "success": false, "error": format!("Unsupported PHP version: {version}") });
    }
    if !pool_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        || pool_name.is_empty()
    {
        return json!({ "success": false, "error": "Invalid pool name" });
    }
    let pool_file = format!("{}/{pool_name}.conf", pool_dir(version));
    if Path::new(&pool_file).exists() {
        return json!({ "success": false, "error": format!("Pool {pool_name} already exists") });
    }

    let get = |key: &str, default: &str| -> String {
        config
            .get(key)
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| v.to_string())
            })
            .unwrap_or_else(|| default.to_string())
    };

    let vars: Vec<(String, String)> = vec![
        ("pool_name".into(), pool_name.to_string()),
        ("version".into(), version.to_string()),
        ("user".into(), get("user", "www-data")),
        ("group".into(), get("group", "www-data")),
        ("pm_type".into(), get("pm_type", "dynamic")),
        ("max_children".into(), get("max_children", "10")),
        ("start_servers".into(), get("start_servers", "2")),
        ("min_spare".into(), get("min_spare", "1")),
        ("max_spare".into(), get("max_spare", "3")),
        ("max_requests".into(), get("max_requests", "500")),
        (
            "open_basedir".into(),
            get("open_basedir", "/var/www:/tmp:/usr/share"),
        ),
        (
            "disable_functions".into(),
            get(
                "disable_functions",
                "exec,passthru,shell_exec,system,proc_open,popen",
            ),
        ),
        (
            "max_execution_time".into(),
            get("max_execution_time", "300"),
        ),
        ("max_input_time".into(), get("max_input_time", "300")),
        ("memory_limit".into(), get("memory_limit", "256M")),
        ("post_max_size".into(), get("post_max_size", "64M")),
        (
            "upload_max_filesize".into(),
            get("upload_max_filesize", "64M"),
        ),
        ("opcache_enable".into(), get("opcache_enable", "1")),
        ("opcache_memory".into(), get("opcache_memory", "128")),
        ("opcache_files".into(), get("opcache_files", "10000")),
    ];

    let mut content = POOL_TEMPLATE.to_string();
    for (k, v) in &vars {
        content = content.replace(&format!("{{{k}}}"), v);
    }

    privileged(&["mkdir", "-p", "/var/log/php"], 15).await;
    let r = privileged_with_stdin(&["tee", &pool_file], Some(&content), 30).await;
    if r.ok {
        restart_fpm(version).await;
        json!({ "success": true, "message": format!("Pool {pool_name} created"), "file": pool_file })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `PHPService.delete_pool` — `www` is protected.
pub async fn delete_pool(version: &str, pool_name: &str) -> Value {
    if pool_name == "www" {
        return json!({ "success": false, "error": "Cannot delete default www pool" });
    }
    if !pool_name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return json!({ "success": false, "error": "Invalid pool name" });
    }
    let pool_file = format!("{}/{pool_name}.conf", pool_dir(version));
    if !Path::new(&pool_file).exists() {
        return json!({ "success": false, "error": format!("Pool {pool_name} not found") });
    }
    let r = privileged(&["rm", &pool_file], 15).await;
    if r.ok {
        restart_fpm(version).await;
        json!({ "success": true, "message": format!("Pool {pool_name} deleted") })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `PHPService.restart_fpm`
pub async fn restart_fpm(version: &str) -> Value {
    let svc = fpm_service(version);
    let r = privileged(&["systemctl", "restart", &svc], 30).await;
    json!({
        "success": r.ok,
        "message": if r.ok { format!("{svc} restarted") } else { r.stderr },
    })
}

/// `PHPService.reload_fpm`
pub async fn reload_fpm(version: &str) -> Value {
    let svc = fpm_service(version);
    let r = privileged(&["systemctl", "reload", &svc], 30).await;
    json!({
        "success": r.ok,
        "message": if r.ok { format!("{svc} reloaded") } else { r.stderr },
    })
}

/// `PHPService.get_fpm_status`
pub async fn fpm_status(version: &str) -> Value {
    let svc = fpm_service(version);
    let running = service_is_active(&svc).await;
    json!({
        "version": version,
        "service": svc,
        "running": running,
        "status": if running { "running" } else { "stopped" },
    })
}

/// `PHPService.get_php_info` — key ini values from `php -i`.
pub async fn php_info(version: &str) -> Value {
    let php_bin = format!("/usr/bin/php{version}");
    if !Path::new(&php_bin).exists() {
        return json!({ "error": format!("PHP {version} not found") });
    }
    let r = run(&[&php_bin, "-i"], 30).await;
    if !r.ok {
        return json!({ "error": r.stderr });
    }
    const KEYS: &[&str] = &[
        "memory_limit",
        "max_execution_time",
        "upload_max_filesize",
        "post_max_size",
        "max_input_time",
        "date.timezone",
    ];
    let mut info = Map::new();
    for line in r.stdout.lines() {
        if let Some((key, value)) = line.split_once("=>") {
            let key = key.trim();
            if KEYS.contains(&key) && !info.contains_key(key) {
                info.insert(key.to_string(), json!(value.trim()));
            }
        }
    }
    Value::Object(info)
}
