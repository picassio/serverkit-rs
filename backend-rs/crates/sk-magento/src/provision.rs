//! Store provisioning orchestrator — async task that mirrors the
//! magento-vm-provisioner module order:
//!
//! 1. data plane up (compose: mariadb/opensearch/redis/mailpit) + health wait
//! 2. exact-version composer.phar
//! 3. `composer create-project` (mage-os repo by default — no auth keys)
//! 4. `bin/magento setup:install` (redis sessions/cache/FPC, opensearch)
//! 5. nginx vhost (Magento's nginx.conf.sample) + enable + reload
//! 6. crontab entry (`bin/magento cron:run` every minute)
//!
//! Progress lands in `magento_stores.status_detail` and `{root}/provision.log`.

use crate::store::{self, Store};
use sqlx::SqlitePool;
use std::io::Write;
use std::process::Stdio;
use tokio::process::Command;

pub struct ProvisionSpec {
    pub base_dir: String, // e.g. /srv/serverkit/stacks
}

fn log_line(root: &str, msg: &str) {
    let line = format!("[{}] {msg}\n", chrono::Local::now().format("%H:%M:%S"));
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(format!("{root}/provision.log"))
    {
        let _ = f.write_all(line.as_bytes());
    }
    tracing::info!(target: "magento", "{msg}");
}

async fn run_logged(
    root: &str,
    program: &str,
    args: &[&str],
    cwd: Option<&str>,
    envs: &[(&str, &str)],
    timeout_secs: u64,
) -> Result<String, String> {
    log_line(root, &format!("$ {program} {}", args.join(" ")));
    let mut cmd = Command::new(program);
    cmd.args(args).stdout(Stdio::piped()).stderr(Stdio::piped());
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    for (k, v) in envs {
        cmd.env(k, v);
    }
    let fut = cmd.output();
    match tokio::time::timeout(std::time::Duration::from_secs(timeout_secs), fut).await {
        Ok(Ok(out)) => {
            let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
            // keep the log readable — last 40 lines of each stream
            for chunk in [&stdout, &stderr] {
                let lines: Vec<&str> = chunk.lines().collect();
                let tail = &lines[lines.len().saturating_sub(40)..];
                for l in tail {
                    log_line(root, &format!("  {l}"));
                }
            }
            if out.status.success() {
                Ok(stdout)
            } else {
                Err(format!(
                    "{program} exited {}: {}",
                    out.status.code().unwrap_or(-1),
                    stderr.lines().rev().take(5).collect::<Vec<_>>().join(" | ")
                ))
            }
        }
        Ok(Err(e)) => Err(e.to_string()),
        Err(_) => Err(format!("{program} timed out after {timeout_secs}s")),
    }
}

async fn wait_healthy(root: &str, container: &str, tries: u32) -> Result<(), String> {
    log_line(root, &format!("waiting for {container} to be healthy..."));
    for _ in 0..tries {
        let out = Command::new("docker")
            .args(["inspect", "--format", "{{.State.Health.Status}}", container])
            .output()
            .await;
        if let Ok(out) = out {
            if String::from_utf8_lossy(&out.stdout).trim() == "healthy" {
                log_line(root, &format!("{container} healthy"));
                return Ok(());
            }
        }
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    }
    Err(format!("{container} did not become healthy"))
}

/// Spawn the provisioning task for a freshly inserted store row.
pub fn spawn(pool: SqlitePool, store: Store, spec: ProvisionSpec) {
    tokio::spawn(async move {
        let id = store.id;
        if let Err(err) = run(pool.clone(), &store, &spec).await {
            store::set_status(&pool, id, "failed", &err).await;
        }
    });
}

fn bool_extra(v: &serde_json::Value, path: &[&str]) -> bool {
    let mut cur = v;
    for key in path {
        cur = &cur[*key];
    }
    cur.as_bool().unwrap_or(false)
}

fn string_array_extra(v: &serde_json::Value, path: &[&str]) -> Vec<String> {
    let mut cur = v;
    for key in path {
        cur = &cur[*key];
    }
    cur.as_array()
        .map(|a| {
            a.iter()
                .filter_map(|x| x.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn safe_snippet_line(line: &str) -> bool {
    !line.contains('\0') && !line.contains("include /etc/passwd")
}

fn escape_nginx_regex_literal(input: &str) -> String {
    let mut out = String::new();
    for ch in input.chars() {
        if matches!(
            ch,
            '.' | '^' | '$' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '\\'
        ) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn valid_location_path(path: &str) -> bool {
    path.starts_with('/')
        && !path.contains("..")
        && !path.contains('{')
        && !path.contains('}')
        && !path.contains(';')
        && !path.chars().any(char::is_whitespace)
}

fn location_access_snippet(item: &serde_json::Value) -> String {
    let mut out = String::new();
    if item["satisfy_any"].as_bool().unwrap_or(false) {
        out.push_str("        satisfy any;\n");
    }
    for ip in string_array_extra(item, &["allow"]) {
        if safe_snippet_line(&ip) {
            out.push_str(&format!("        allow {ip};\n"));
        }
    }
    for ip in string_array_extra(item, &["deny"]) {
        if safe_snippet_line(&ip) {
            out.push_str(&format!("        deny {ip};\n"));
        }
    }
    out
}

fn cors_snippet(item: &serde_json::Value, indent: &str) -> String {
    if !item["cors"].as_bool().unwrap_or(false) {
        return String::new();
    }
    let cors = &item["cors_config"];
    let origin = cors["origin"].as_str().unwrap_or("*").replace('"', "");
    let methods = cors["methods"]
        .as_str()
        .unwrap_or("POST, GET, OPTIONS")
        .replace('"', "");
    let headers = cors["headers"].as_str().unwrap_or("Accept,Authorization,Cache-Control,Content-Type,DNT,If-Modified-Since,Keep-Alive,Origin,User-Agent,X-Requested-With,X-Cache-Hash").replace('"', "");
    format!(
        "{indent}if ($request_method = 'OPTIONS') {{\n{indent}    add_header 'Access-Control-Allow-Origin' '{origin}' always;\n{indent}    add_header 'Access-Control-Allow-Methods' '{methods}' always;\n{indent}    add_header 'Access-Control-Allow-Headers' '{headers}' always;\n{indent}    add_header 'Access-Control-Max-Age' 1728000;\n{indent}    add_header 'Content-Type' 'text/plain charset=UTF-8';\n{indent}    add_header 'Content-Length' 0;\n{indent}    return 204;\n{indent}}}\n{indent}add_header 'Access-Control-Allow-Origin' '{origin}' always;\n"
    )
}

fn nginx_extra_locations_snippet(extras: &serde_json::Value) -> String {
    let mut out = String::new();
    let Some(items) = extras["extra_locations"].as_array() else {
        return out;
    };
    for item in items {
        if item["enabled"].as_bool() == Some(false) {
            continue;
        }
        let path = item["path"].as_str().unwrap_or("").trim();
        let kind = item["kind"].as_str().unwrap_or("alias");
        let target = item["target"].as_str().unwrap_or("").trim();
        if !valid_location_path(path) || target.is_empty() || !safe_snippet_line(target) {
            continue;
        }
        let matcher = if item["match"].as_str() == Some("exact") {
            format!("= {path}")
        } else if item["match"].as_str() == Some("regex") {
            format!("~ {path}")
        } else {
            format!("^~ {path}")
        };
        out.push_str("\n    # ServerKit extra location\n");
        match kind {
            "proxy" if target.starts_with("http://") || target.starts_with("https://") => {
                out.push_str(&format!(
                    "    location {matcher} {{\n        proxy_pass {target};\n        proxy_http_version 1.1;\n        proxy_set_header Host $host;\n        proxy_set_header X-Forwarded-Host $host;\n        proxy_set_header X-Real-IP $remote_addr;\n        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;\n        proxy_set_header X-Forwarded-Proto $scheme;\n        proxy_read_timeout {};\n        proxy_connect_timeout {};\n        proxy_send_timeout {};\n{}{}    }}\n",
                    item["proxy_read_timeout"].as_str().unwrap_or("600"),
                    item["proxy_connect_timeout"].as_str().unwrap_or("600"),
                    item["proxy_send_timeout"].as_str().unwrap_or("600"),
                    location_access_snippet(item),
                    cors_snippet(item, "        "),
                ));
            }
            "return" => {
                out.push_str(&format!(
                    "    location {matcher} {{\n{}        return {target};\n    }}\n",
                    location_access_snippet(item)
                ));
            }
            _ if target.starts_with('/') && !target.contains("..") => {
                out.push_str(&format!(
                    "    location {matcher} {{\n        alias {target};\n"
                ));
                out.push_str(&location_access_snippet(item));
                if item["autoindex"].as_bool().unwrap_or(false) {
                    out.push_str("        autoindex on;\n        autoindex_localtime on;\n");
                }
                out.push_str(&cors_snippet(item, "        "));
                if let Some(cache) = item["cache_control"].as_str() {
                    if safe_snippet_line(cache) {
                        out.push_str(&format!("        add_header Cache-Control \"{cache}\";\n"));
                    }
                }
                if item["x_frame_sameorigin"].as_bool().unwrap_or(false) {
                    out.push_str("        add_header X-Frame-Options \"SAMEORIGIN\";\n");
                }
                if let Some(expires) = item["expires"].as_str() {
                    if safe_snippet_line(expires) {
                        out.push_str(&format!("        expires {expires};\n"));
                    }
                }
                if let Some(fallback) = item["try_files_fallback"].as_str() {
                    if safe_snippet_line(fallback) {
                        out.push_str(&format!("        try_files $uri $uri/ {fallback};\n"));
                    }
                }
                out.push_str("    }\n");
            }
            _ => {}
        }
    }
    out
}

fn safe_var_suffix(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

fn nginx_extras_snippet(s: &Store) -> String {
    let extras = s.nginx_extras_value();
    let suffix = safe_var_suffix(&s.name);
    let mut out = String::new();
    if let Some(size) = extras["client_max_body_size"].as_str() {
        if safe_snippet_line(size) {
            out.push_str(&format!("    client_max_body_size {size};\n"));
        }
    }
    if let Some(value) = extras["x_robots_tag"].as_str() {
        if safe_snippet_line(value) {
            out.push_str(&format!(
                "    add_header X-Robots-Tag \"{}\" always;\n",
                value.replace('"', "")
            ));
        }
    }
    if bool_extra(&extras, &["proxy_tuning", "enabled"]) {
        let p = &extras["proxy_tuning"];
        out.push_str(&format!(
            "    proxy_buffer_size {};\n    proxy_buffers {};\n    proxy_busy_buffers_size {};\n    proxy_read_timeout {};\n    proxy_connect_timeout {};\n    proxy_send_timeout {};\n",
            p["buffer_size"].as_str().unwrap_or("128k"),
            p["buffers"].as_str().unwrap_or("4 256k"),
            p["busy_buffers_size"].as_str().unwrap_or("256k"),
            p["read_timeout"].as_str().unwrap_or("600"),
            p["connect_timeout"].as_str().unwrap_or("600"),
            p["send_timeout"].as_str().unwrap_or("600"),
        ));
    }
    out.push_str(&nginx_extra_locations_snippet(&extras));
    if bool_extra(&extras, &["badbot", "enabled"]) {
        out.push_str(&format!(
            "\n    if ($serverkit_bad_bot_{suffix}) {{ return 444; }}\n"
        ));
    }
    if bool_extra(&extras, &["ip_filter", "enabled"]) {
        for ip in string_array_extra(&extras, &["ip_filter", "allow"]) {
            if safe_snippet_line(&ip) {
                out.push_str(&format!("    allow {ip};\n"));
            }
        }
        for ip in string_array_extra(&extras, &["ip_filter", "deny"]) {
            if safe_snippet_line(&ip) {
                out.push_str(&format!("    deny {ip};\n"));
            }
        }
    }
    if bool_extra(&extras, &["htpasswd", "enabled"]) {
        let realm = extras["htpasswd"]["realm"]
            .as_str()
            .unwrap_or("Restricted")
            .replace('"', "");
        let file = extras["htpasswd"]["file"]
            .as_str()
            .unwrap_or("/etc/nginx/.htpasswd");
        if file.starts_with('/') && !file.contains("..") {
            out.push_str(&format!(
                "    auth_basic \"{realm}\";\n    auth_basic_user_file {file};\n"
            ));
            if extras["htpasswd"]["satisfy_any"].as_bool().unwrap_or(false) {
                out.push_str("    satisfy any;\n");
            }
        }
    }
    if bool_extra(&extras, &["maintenance", "enabled"]) {
        let dir = format!("/etc/nginx/serverkit/{}", s.name);
        out.push_str(&format!(
            "    set $serverkit_maintenance_{suffix} off;\n    if ($serverkit_whitelist_{suffix} = 0) {{ set $serverkit_maintenance_{suffix} on; }}\n    if ($serverkit_maintenance_{suffix} = on) {{ return 503; }}\n    error_page 503 @serverkit_maintenance;\n    location @serverkit_maintenance {{ root {dir}; rewrite ^ /maintenance.html break; }}\n"
        ));
    }
    if let Some(snippet) = extras["custom_server_snippet"].as_str() {
        if snippet.lines().all(safe_snippet_line) {
            out.push_str("\n    # Custom ServerKit nginx extras\n");
            for line in snippet.lines() {
                out.push_str("    ");
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

async fn write_nginx_extra_assets(s: &Store) -> Result<(), String> {
    let extras = s.nginx_extras_value();
    let suffix = safe_var_suffix(&s.name);
    let dir = format!("/etc/nginx/serverkit/{}", s.name);
    run_privileged(&["mkdir", "-p", &dir]).await?;
    if bool_extra(&extras, &["maintenance", "enabled"]) {
        let html = extras["maintenance"]["html"].as_str().unwrap_or(
            "<!doctype html><title>Maintenance</title><style>body{text-align:center;padding:150px;font:20px Helvetica,sans-serif;color:#333;background:#f5f5f5}h1{font-size:50px}</style><h1>We'll be back soon!</h1><p>Performing scheduled maintenance.</p>",
        );
        write_privileged(&format!("{dir}/maintenance.html"), html).await?;
    }
    if bool_extra(&extras, &["badbot", "enabled"]) {
        let mut map = format!(
            "map $http_user_agent $serverkit_bad_bot_{suffix} {{\n    default 0;\n    ~*^Lynx 0;\n"
        );
        for p in string_array_extra(&extras, &["badbot", "patterns"]) {
            if safe_snippet_line(&p) {
                map.push_str(&format!("    ~*{} 1;\n", escape_nginx_regex_literal(&p)));
            }
        }
        map.push_str("}\n");
        write_privileged(
            &format!("/etc/nginx/conf.d/serverkit-{}-badbot.conf", s.name),
            &map,
        )
        .await?;
    }
    if bool_extra(&extras, &["maintenance", "enabled"])
        || bool_extra(&extras, &["htpasswd", "enabled"])
    {
        let mut geo = format!("geo $serverkit_whitelist_{suffix} {{\n    default 0;\n");
        let mut wl = string_array_extra(&extras, &["maintenance", "whitelist"]);
        wl.extend(string_array_extra(&extras, &["htpasswd", "allow"]));
        for ip in wl {
            if safe_snippet_line(&ip) {
                geo.push_str(&format!("    {ip} 1;\n"));
            }
        }
        geo.push_str("}\n");
        write_privileged(
            &format!("/etc/nginx/conf.d/serverkit-{}-whitelist.conf", s.name),
            &geo,
        )
        .await?;
    }
    Ok(())
}

fn preserve_manual_vhost(s: &Store) -> bool {
    bool_extra(&s.nginx_extras_value(), &["manual_vhost", "preserve"])
}

async fn run(pool: SqlitePool, s: &Store, _spec: &ProvisionSpec) -> Result<(), String> {
    let root = &s.root_path;
    let base = crate::port_base(s.id);
    let db_pw = s.db_password_plain().unwrap_or_default();
    let admin_pw = s.admin_password_plain().unwrap_or_default();
    let php = format!("php{}", s.php_version);
    let src = s.magento_src();
    let https = s.ssl_mode != "none";
    let scheme = if https { "https" } else { "http" };

    std::fs::create_dir_all(root).map_err(|e| e.to_string())?;
    log_line(
        root,
        &format!(
            "provisioning store '{}' — Magento {} ({}) / PHP {} / Composer {}",
            s.name, s.magento_version, s.distribution, s.php_version, s.composer_version
        ),
    );

    // ── 1. data plane ────────────────────────────────────────────────
    store::set_status(
        &pool,
        s.id,
        "provisioning",
        "starting data services (db/opensearch/redis/mailpit)",
    )
    .await;
    let compose_path = format!("{root}/docker-compose.yml");
    if s.use_varnish {
        // bootstrap VCL so the varnish container can start pre-install
        std::fs::write(
            format!("{root}/default.vcl"),
            crate::compose::bootstrap_vcl(base + 7),
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::write(
        &compose_path,
        crate::compose::compose_yaml(
            &s.name,
            base,
            &db_pw,
            s.use_rabbitmq,
            s.use_varnish,
            &s.service_versions_map(),
        ),
    )
    .map_err(|e| e.to_string())?;
    run_logged(
        root,
        "docker",
        &["compose", "-f", &compose_path, "up", "-d"],
        None,
        &[],
        600,
    )
    .await?;
    wait_healthy(root, &format!("magento-{}-db", s.name), 60).await?;
    wait_healthy(root, &format!("magento-{}-opensearch", s.name), 90).await?;
    if s.use_rabbitmq {
        wait_healthy(root, &format!("magento-{}-rabbitmq", s.name), 60).await?;
    }

    // ── 2. exact composer (only when ServerKit initializes Magento) ──
    let composer = format!("{root}/composer.phar");
    if s.install_magento {
        store::set_status(
            &pool,
            s.id,
            "provisioning",
            &format!("installing composer {}", s.composer_version),
        )
        .await;
        if !std::path::Path::new(&composer).exists() {
            let url = format!(
                "https://getcomposer.org/download/{}/composer.phar",
                s.composer_version
            );
            run_logged(
                root,
                "curl",
                &["-fsSL", "-o", &composer, &url],
                None,
                &[],
                300,
            )
            .await?;
        }
    }

    // ── 3. optional create-project ───────────────────────────────────
    let magento_bin = format!("{src}/bin/magento");
    if s.install_magento && !std::path::Path::new(&magento_bin).exists() {
        store::set_status(
            &pool,
            s.id,
            "provisioning",
            "composer create-project (this takes several minutes)",
        )
        .await;
        // mage-os = the Mage-OS *mirror* of the official packages: identical
        // magento/project-community-edition at exact Magento versions, no
        // repo.magento.com auth keys needed.
        let (repo, package) = match s.distribution.as_str() {
            "magento" => (
                "https://repo.magento.com/",
                "magento/project-community-edition",
            ),
            _ => (
                "https://mirror.mage-os.org/",
                "magento/project-community-edition",
            ),
        };
        let version_arg = s.magento_version.clone();
        let repo_arg = format!("--repository-url={repo}");
        let mut args: Vec<&str> = vec![
            &composer,
            "create-project",
            &repo_arg,
            package,
            &src,
            "--no-interaction",
        ];
        if version_arg != "latest" {
            args.push(&version_arg);
        }
        run_logged(root, &php, &args, None, &[("COMPOSER_HOME", root)], 3600).await?;
    } else if std::path::Path::new(&magento_bin).exists() {
        log_line(
            root,
            &format!("{magento_bin} already present — skipping create-project"),
        );
    } else {
        store::set_status(
            &pool,
            s.id,
            "running",
            "data services ready; Magento initialization skipped",
        )
        .await;
        log_line(
            root,
            "DONE — data services are running; Magento init skipped",
        );
        return Ok(());
    }

    // ── 4. optional setup:install ────────────────────────────────────
    let env_php = format!("{src}/app/etc/env.php");
    let mut admin_path = "/admin".to_string();
    if s.install_magento {
        store::set_status(&pool, s.id, "provisioning", "bin/magento setup:install").await;
        // The data plane is always freshly created by this pipeline, so a stale
        // env.php from a previous attempt is invalid (old crypt key/db config)
        // and makes setup:install fail with merged-config errors. Remove it.
        if std::path::Path::new(&env_php).exists() {
            log_line(root, "removing stale app/etc/env.php from previous attempt");
            let _ = std::fs::remove_file(&env_php);
            let _ = run_privileged(&["rm", "-f", &env_php]).await; // in case www-data owns it
        }
        let db_port = base.to_string();
        let os_port = (base + 1).to_string();
        let redis_port = (base + 2).to_string();
        let amqp_port = (base + 3).to_string();
        let base_url = format!("{scheme}://{}/", s.domain);
        let admin_email = format!("admin@{}", s.domain);
        let db_host = format!("127.0.0.1:{db_port}");

        let mut install_args: Vec<String> = vec![
            format!("{src}/bin/magento"),
            "setup:install".into(),
            format!("--base-url={base_url}"),
            format!("--db-host={db_host}"),
            "--db-name=magento".into(),
            "--db-user=magento".into(),
            format!("--db-password={db_pw}"),
            "--admin-firstname=Admin".into(),
            "--admin-lastname=User".into(),
            format!("--admin-email={admin_email}"),
            "--admin-user=admin".into(),
            format!("--admin-password={admin_pw}"),
            "--language=en_US".into(),
            "--currency=USD".into(),
            "--timezone=UTC".into(),
            "--use-rewrites=1".into(),
            "--search-engine=opensearch".into(),
            "--opensearch-host=127.0.0.1".into(),
            format!("--opensearch-port={os_port}"),
            "--session-save=redis".into(),
            "--session-save-redis-host=127.0.0.1".into(),
            format!("--session-save-redis-port={redis_port}"),
            "--session-save-redis-db=2".into(),
            "--cache-backend=redis".into(),
            "--cache-backend-redis-server=127.0.0.1".into(),
            format!("--cache-backend-redis-port={redis_port}"),
            "--cache-backend-redis-db=0".into(),
            "--page-cache=redis".into(),
            "--page-cache-redis-server=127.0.0.1".into(),
            format!("--page-cache-redis-port={redis_port}"),
            "--page-cache-redis-db=1".into(),
            "--no-interaction".into(),
        ];
        if https {
            install_args.push(format!("--base-url-secure={base_url}"));
            install_args.push("--use-secure=1".into());
            install_args.push("--use-secure-admin=1".into());
        }
        if s.use_rabbitmq {
            install_args.extend([
                "--amqp-host=127.0.0.1".into(),
                format!("--amqp-port={amqp_port}"),
                "--amqp-user=magento".into(),
                format!("--amqp-password={db_pw}"),
                "--amqp-virtualhost=/".into(),
            ]);
        }
        let arg_refs: Vec<&str> = install_args.iter().map(String::as_str).collect();
        let out = run_logged(root, &php, &arg_refs, Some(&src), &[], 1800).await?;

        // capture the generated admin URI (line: "Magento Admin URI: /admin_xyz")
        if let Some(line) = out.lines().find(|l| l.contains("Admin URI")) {
            if let Some(uri) = line.split("Admin URI:").nth(1) {
                admin_path = uri.trim().to_string();
                let admin_url = format!("{scheme}://{}{}", s.domain, admin_path);
                store::set_admin_url(&pool, s.id, &admin_url).await;
                log_line(root, &format!("admin url: {admin_url}"));
            }
        }
    }

    // sensible dev defaults for a fresh initialized box
    if s.install_magento {
        let _ = run_logged(
            root,
            &php,
            &[
                &format!("{src}/bin/magento"),
                "deploy:mode:set",
                "developer",
            ],
            Some(&src),
            &[],
            300,
        )
        .await;
    }

    // ── 4b. per-store PHP-FPM pool (as run_user) ─────────────────────
    // A dedicated pool lets each store run PHP as any user (e.g. `ubuntu`
    // on a dev box) instead of the shared www-data pool. The vhost upstream
    // points at this pool's socket (php{ver}-fpm-{pool}.sock).
    store::set_status(
        &pool,
        s.id,
        "provisioning",
        &format!("creating php-fpm pool as {}", s.run_user),
    )
    .await;
    {
        let mut cfg = serde_json::Map::new();
        cfg.insert("user".into(), serde_json::json!(s.run_user));
        cfg.insert("group".into(), serde_json::json!(s.run_user));
        cfg.insert(
            "open_basedir".into(),
            serde_json::json!(format!("{src}:/tmp:/usr/share")),
        );
        // Magento needs these; the default pool template disables them
        cfg.insert("disable_functions".into(), serde_json::json!(""));
        cfg.insert("memory_limit".into(), serde_json::json!("2G"));
        let r = sk_web::php::create_pool(&s.php_version, &s.fpm_pool(), &cfg).await;
        if !r["success"].as_bool().unwrap_or(false) {
            // pool may already exist on re-provision — tolerate that
            let err = r["error"].as_str().unwrap_or("");
            if !err.contains("already exists") {
                return Err(format!("php-fpm pool creation failed: {err}"));
            }
        }
    }

    // ── 4c. permissions ──────────────────────────────────────
    store::set_status(
        &pool,
        s.id,
        "provisioning",
        "fixing file permissions (ACLs)",
    )
    .await;
    for note in repair_permissions(s).await? {
        log_line(root, &note);
    }

    // ── 5. nginx vhost ───────────────────────────────────────────────
    store::set_status(&pool, s.id, "provisioning", "configuring nginx worker user").await;
    let nginx_user = sk_web::nginx::set_worker_user(&s.run_user, &s.run_user).await;
    if !nginx_user["success"].as_bool().unwrap_or(false) {
        return Err(format!(
            "nginx worker user update failed: {}",
            nginx_user["error"].as_str().unwrap_or("unknown error")
        ));
    }
    log_line(
        root,
        &format!("nginx worker user set to {}:{}", s.run_user, s.run_user),
    );

    store::set_status(&pool, s.id, "provisioning", "creating nginx vhost").await;
    write_nginx_extra_assets(s).await?;
    let server_extras = nginx_extras_snippet(s);
    let vhost_path = format!("/etc/nginx/sites-available/{}", s.name);
    if preserve_manual_vhost(s) && std::path::Path::new(&vhost_path).exists() {
        log_line(
            root,
            "manual vhost preservation enabled — skipping generated Magento vhost overwrite",
        );
        let reload = sk_web::nginx::reload().await;
        if !reload["success"].as_bool().unwrap_or(false) {
            return Err(format!(
                "nginx reload failed: {}",
                reload["error"]
                    .as_str()
                    .or(reload["message"].as_str())
                    .unwrap_or("?")
            ));
        }
        store::set_status(&pool, s.id, "running", "manual nginx vhost preserved").await;
        return Ok(());
    }
    // Per-store copy of nginx.conf.sample with the upstream reference
    // rewritten to this store's unique name (see compose::upstream_name).
    let sample = std::fs::read_to_string(format!("{src}/nginx.conf.sample"))
        .map_err(|e| format!("nginx.conf.sample missing: {e}"))?;
    let rewritten = sample.replace("fastcgi_backend", &crate::compose::upstream_name(&s.name));
    std::fs::write(format!("{src}/nginx.conf.serverkit"), rewritten).map_err(|e| e.to_string())?;
    let cert_paths = if https {
        Some(issue_cert(s).await?)
    } else {
        None
    };
    if cert_paths.is_some() {
        log_line(
            root,
            "self-signed certificate issued (SANs: all store domains)",
        );
    }
    let ssl_ref = cert_paths.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));

    let vhost = if s.headless_mode == "shared" {
        crate::compose::magento_vhost_headless_shared(
            &s.name,
            &s.domain,
            &src,
            &s.php_version,
            base + 7,
            &admin_path,
            s.frontend_port,
            s.frontend_root.as_deref(),
            &s.custom_routes(),
            &server_extras,
            ssl_ref,
        )
    } else if s.headless_mode == "split" {
        let api_domain = s
            .api_domain
            .as_deref()
            .ok_or_else(|| "api_domain is required for split headless mode".to_string())?;
        crate::compose::magento_vhost_headless_split(
            &s.name,
            api_domain,
            s.admin_domain
                .as_deref()
                .unwrap_or(&format!("admin.{api_domain}")),
            &src,
            &s.php_version,
            base + 7,
            &admin_path,
            &s.split_route_mode,
            &server_extras,
            ssl_ref,
        )
    } else if s.use_varnish {
        crate::compose::magento_vhost_varnish(
            &s.name,
            &s.domain,
            &src,
            &s.php_version,
            base + 6,
            base + 7,
            ssl_ref,
        )
    } else if let Some((cert, key)) = &cert_paths {
        crate::compose::magento_vhost_ssl(&s.name, &s.domain, &src, &s.php_version, cert, key)
    } else {
        crate::compose::magento_vhost(&s.name, &s.domain, &src, &s.php_version)
    };
    write_privileged(&vhost_path, &vhost).await?;
    run_privileged(&[
        "ln",
        "-sf",
        &vhost_path,
        &format!("/etc/nginx/sites-enabled/{}", s.name),
    ])
    .await?;
    // separate/split headless modes: additional vhost for the frontend domain
    if matches!(s.headless_mode.as_str(), "separate" | "split") {
        if let Some(fd) = &s.frontend_domain {
            let fe_vhost = crate::compose::frontend_vhost(
                &s.name,
                fd,
                s.frontend_port,
                s.frontend_root.as_deref(),
                &server_extras,
                ssl_ref,
            );
            let fe_path = format!("/etc/nginx/sites-available/{}-frontend", s.name);
            write_privileged(&fe_path, &fe_vhost).await?;
            run_privileged(&[
                "ln",
                "-sf",
                &fe_path,
                &format!("/etc/nginx/sites-enabled/{}-frontend", s.name),
            ])
            .await?;
            log_line(root, &format!("frontend vhost for {fd} enabled"));
        }
    }

    let reload = sk_web::nginx::reload().await;
    if !reload["success"].as_bool().unwrap_or(false) {
        return Err(format!(
            "nginx reload failed: {}",
            reload["error"].as_str().unwrap_or("?")
        ));
    }
    log_line(root, "nginx vhost enabled");

    // ── 5b. varnish FPC wiring ──────────────────────────────────
    if s.use_varnish {
        store::set_status(&pool, s.id, "provisioning", "configuring varnish FPC").await;
        let magento_bin = format!("{src}/bin/magento");
        if !std::path::Path::new(&env_php).exists() {
            log_line(
                root,
                "warning: varnish selected but Magento is not initialized; skipping FPC config",
            );
            store::set_status(
                &pool,
                s.id,
                "running",
                "stack ready; Magento source configured but not initialized",
            )
            .await;
            return Ok(());
        }
        // FPC engine = Varnish
        run_logged(
            root,
            &php,
            &[
                &magento_bin,
                "config:set",
                "system/full_page_cache/caching_application",
                "2",
            ],
            Some(&src),
            &[],
            120,
        )
        .await?;
        // purge target
        let cache_hosts = format!("--http-cache-hosts=127.0.0.1:{}", base + 6);
        run_logged(
            root,
            &php,
            &[
                &magento_bin,
                "setup:config:set",
                &cache_hosts,
                "--no-interaction",
            ],
            Some(&src),
            &[],
            120,
        )
        .await?;
        // canonical VCL against the nginx cache backend
        let backend_port = (base + 7).to_string();
        let vcl = run_logged(
            root,
            &php,
            &[
                &magento_bin,
                "varnish:vcl:generate",
                "--export-version=6",
                "--backend-host=host.docker.internal",
                "--backend-port",
                &backend_port,
                "--access-list=localhost",
                "--grace-period=300",
            ],
            Some(&src),
            &[],
            120,
        )
        .await?;
        // widen the purge ACL: purges arrive from the compose network
        // gateway, not localhost
        let vcl = vcl.replace("\"localhost\";", "\"localhost\";\n    \"172.16.0.0\"/12;");
        std::fs::write(format!("{root}/default.vcl"), vcl).map_err(|e| e.to_string())?;
        run_logged(
            root,
            "docker",
            &["restart", &format!("magento-{}-varnish", s.name)],
            None,
            &[],
            120,
        )
        .await?;
        run_logged(
            root,
            &php,
            &[&magento_bin, "cache:flush"],
            Some(&src),
            &[],
            120,
        )
        .await?;
        log_line(
            root,
            "varnish FPC active (nginx proxy → varnish → nginx backend)",
        );
    }

    // ── 5c. managed frontend process ───────────────────────────
    if s.frontend_cmd.is_some() {
        store::set_status(&pool, s.id, "provisioning", "starting managed frontend").await;
        if let Err(e) = apply_frontend_unit(s).await {
            log_line(root, &format!("warning: frontend unit failed: {e}"));
        }
    }

    // ── 6. cron ──────────────────────────────────────────────────────
    store::set_status(&pool, s.id, "provisioning", "installing magento cron").await;
    let cron_cmd = format!("/usr/bin/{php} {src}/bin/magento cron:run");
    let cron = sk_ops::cron::add_job(
        "* * * * *",
        &cron_cmd,
        Some(&format!("magento-{}", s.name)),
        None,
    )
    .await;
    if !cron["success"].as_bool().unwrap_or(false) {
        log_line(
            root,
            &format!(
                "warning: cron install failed: {}",
                cron["error"].as_str().unwrap_or("?")
            ),
        );
    }

    store::set_status(&pool, s.id, "running", "store provisioned").await;
    log_line(root, "DONE — store is running");
    Ok(())
}

async fn write_privileged(path: &str, content: &str) -> Result<(), String> {
    let mut cmd = Command::new("sudo");
    cmd.args(["-n", "tee", path])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
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
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

async fn run_privileged(args: &[&str]) -> Result<(), String> {
    let out = Command::new("sudo")
        .arg("-n")
        .args(args)
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).into_owned())
    }
}

fn my_cnf_value(value: &str) -> String {
    value.replace(['\n', '\r'], "")
}

pub async fn write_mysql_client_config(s: &Store) -> Result<serde_json::Value, String> {
    if s.run_user.is_empty()
        || !s
            .run_user
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("run_user must be a valid unix username".into());
    }
    let out = Command::new("getent")
        .args(["passwd", &s.run_user])
        .output()
        .await
        .map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("user {} does not exist on this VM", s.run_user));
    }
    let passwd = String::from_utf8_lossy(&out.stdout);
    let home = passwd
        .trim()
        .split(':')
        .nth(5)
        .filter(|h| h.starts_with('/'))
        .ok_or_else(|| format!("could not resolve home directory for {}", s.run_user))?;
    let path = format!("{home}/.my.cnf");
    let port = crate::port_base(s.id);
    let password = my_cnf_value(&s.db_password_plain().unwrap_or_default());
    let content = format!(
        "[client]\nhost=127.0.0.1\nport={port}\nuser=magento\npassword={password}\ndatabase=magento\nprotocol=tcp\n\n[mysql]\ndatabase=magento\n"
    );
    write_privileged(&path, &content).await?;
    run_privileged(&["chown", &format!("{}:{}", s.run_user, s.run_user), &path]).await?;
    run_privileged(&["chmod", "600", &path]).await?;
    Ok(serde_json::json!({
        "success": true,
        "path": path,
        "owner": s.run_user,
        "mode": "0600",
        "command": "mysql magento"
    }))
}

pub async fn repair_permissions(s: &Store) -> Result<Vec<String>, String> {
    let src = s.magento_src();
    if !std::path::Path::new(&src).exists() {
        return Ok(vec![format!(
            "source path {src} does not exist yet; permissions skipped"
        )]);
    }
    let panel_user = std::env::var("USER").unwrap_or_else(|_| "ubuntu".into());
    let run_group = format!("{}:{}", s.run_user, s.run_user);
    let acl_run = format!("u:{}:rwX", s.run_user);
    let acl_nginx = "u:www-data:rwX".to_string();
    let acl_panel = format!("u:{panel_user}:rwX");
    run_privileged(&["chown", "-R", &run_group, &src]).await?;

    // nginx must traverse every parent directory to read a source under /srv or /home.
    run_privileged(&["setfacl", "-m", "u:www-data:--x", &src])
        .await
        .ok();
    let mut current = std::path::PathBuf::from(&src);
    while current.pop() {
        if current.as_os_str().is_empty() || current == std::path::Path::new("/") {
            break;
        }
        if let Some(p) = current.to_str() {
            let _ = run_privileged(&["setfacl", "-m", "u:www-data:--x", p]).await;
        }
    }

    for dir in ["var", "generated", "pub/static", "pub/media", "app/etc"] {
        let path = format!("{src}/{dir}");
        if std::path::Path::new(&path).exists() {
            run_privileged(&[
                "setfacl", "-R", "-m", &acl_run, "-m", &acl_nginx, "-m", &acl_panel, &path,
            ])
            .await?;
            run_privileged(&[
                "setfacl", "-dR", "-m", &acl_run, "-m", &acl_nginx, "-m", &acl_panel, &path,
            ])
            .await?;
        }
    }
    Ok(vec![format!(
        "permissions fixed (owner {}, ACLs for nginx + panel)",
        s.run_user
    )])
}

/// Days until the cert at `cert_path` expires (None if missing/unparseable).
pub fn cert_days_remaining(cert_path: &str) -> Option<i64> {
    let out = std::process::Command::new("openssl")
        .args(["x509", "-enddate", "-noout", "-in", cert_path])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout);
    let date = s.trim().strip_prefix("notAfter=")?.trim();
    // e.g. "Oct  2 05:30:44 2026 GMT"
    let dt = chrono::NaiveDateTime::parse_from_str(date, "%b %e %H:%M:%S %Y GMT").ok()?;
    Some((dt - chrono::Utc::now().naive_utc()).num_days())
}

/// Renew a store's Let's Encrypt cert if it expires within `threshold_days`
/// (or `force`). Re-issues to the same paths and reloads nginx.
pub async fn renew_cert(
    s: &Store,
    force: bool,
    threshold_days: i64,
) -> Result<serde_json::Value, String> {
    use serde_json::json;
    if s.ssl_mode != "letsencrypt" {
        return Ok(json!({ "renewed": false, "reason": "ssl_mode is not letsencrypt" }));
    }
    let cert = format!("/etc/ssl/serverkit/{}.crt", s.name);
    let days = cert_days_remaining(&cert);
    let needs = force || days.map(|d| d < threshold_days).unwrap_or(true);
    if !needs {
        return Ok(json!({ "renewed": false, "days_remaining": days }));
    }
    issue_cert(s).await?;
    let reload = sk_web::nginx::reload().await;
    if !reload["success"].as_bool().unwrap_or(false) {
        return Err(format!(
            "nginx reload failed after renewal: {}",
            reload["error"]
                .as_str()
                .or(reload["message"].as_str())
                .unwrap_or("?")
        ));
    }
    let new_days = cert_days_remaining(&cert);
    Ok(json!({ "renewed": true, "days_before": days, "days_after": new_days }))
}

/// All public domains a store serves (main + frontend + admin), deduped.
fn store_domains(s: &Store) -> Vec<String> {
    let mut v = vec![s.domain.clone()];
    if let Some(fd) = &s.frontend_domain {
        if !v.contains(fd) {
            v.push(fd.clone());
        }
    }
    if let Some(ad) = &s.admin_domain {
        if !v.contains(ad) {
            v.push(ad.clone());
        }
    }
    v
}

/// Certificate dispatcher: self-signed or Let's Encrypt (DNS-01 default,
/// HTTP-01 when `le_challenge == "http"`). Returns (cert_path, key_path).
pub async fn issue_cert(s: &Store) -> Result<(String, String), String> {
    if s.ssl_mode != "letsencrypt" {
        return issue_self_signed(s).await;
    }
    let cert_dir = "/etc/ssl/serverkit";
    let cert = format!("{cert_dir}/{}.crt", s.name);
    let key = format!("{cert_dir}/{}.key", s.name);
    let domains = store_domains(s);
    let email = s
        .le_email
        .clone()
        .unwrap_or_else(|| format!("admin@{}", s.domain));
    let challenge = if s.le_challenge == "http" {
        // shared webroot; store vhosts serve /.well-known/acme-challenge from here
        sk_acme::Challenge::Http01 {
            webroot: "/var/www/letsencrypt".into(),
        }
    } else {
        sk_acme::Challenge::Dns01
    };
    let issued = sk_acme::issue(&domains, &email, challenge, false)
        .await
        .map_err(|e| format!("Let's Encrypt issuance failed: {e}"))?;
    run_privileged(&["mkdir", "-p", cert_dir]).await?;
    write_privileged(&cert, &issued.cert_pem).await?;
    write_privileged(&key, &issued.key_pem).await?;
    Ok((cert, key))
}

/// Issue (or re-issue) the store's self-signed cert with SANs covering every
/// domain the store serves (main + frontend + admin).
pub async fn issue_self_signed(s: &Store) -> Result<(String, String), String> {
    let cert_dir = "/etc/ssl/serverkit";
    let cert = format!("{cert_dir}/{}.crt", s.name);
    let key = format!("{cert_dir}/{}.key", s.name);
    let mut sans = vec![format!("DNS:{}", s.domain)];
    if let Some(fd) = &s.frontend_domain {
        sans.push(format!("DNS:{fd}"));
    }
    if let Some(ad) = &s.admin_domain {
        sans.push(format!("DNS:{ad}"));
    }
    run_privileged(&["mkdir", "-p", cert_dir]).await?;
    run_privileged(&[
        "openssl",
        "req",
        "-x509",
        "-nodes",
        "-newkey",
        "rsa:2048",
        "-days",
        "825",
        "-keyout",
        &key,
        "-out",
        &cert,
        "-subj",
        &format!("/CN={}", s.domain),
        "-addext",
        &format!("subjectAltName={}", sans.join(",")),
    ])
    .await?;
    Ok((cert, key))
}

fn unit_name(store_name: &str) -> String {
    format!("serverkit-fe-{store_name}.service")
}

/// Command validation for frontend processes — same rules as cron commands:
/// absolute path, no shell metacharacters.
pub fn valid_frontend_cmd(cmd: &str) -> bool {
    const BLOCKED: &[&str] = &[";", "&&", "||", "|", "`", "$(", ">", "<", "\n", "\r"];
    if BLOCKED.iter().any(|p| cmd.contains(p)) {
        return false;
    }
    cmd.split_whitespace()
        .next()
        .map(|w| w.starts_with('/'))
        .unwrap_or(false)
}

/// (Re)create + start the systemd unit for a store's managed frontend.
pub async fn apply_frontend_unit(s: &Store) -> Result<(), String> {
    let cmd = s.frontend_cmd.as_deref().ok_or("frontend_cmd is not set")?;
    if !valid_frontend_cmd(cmd) {
        return Err(
            "Invalid frontend_cmd: must use an absolute path and no shell operators".into(),
        );
    }
    let root = s
        .frontend_root
        .as_deref()
        .ok_or("frontend_root is required for a managed frontend")?;
    let unit = unit_name(&s.name);
    let unit_path = format!("/etc/systemd/system/{unit}");
    let content = crate::compose::frontend_unit(&s.name, root, cmd, s.frontend_port, &s.run_user);
    write_privileged(&unit_path, &content).await?;
    run_privileged(&["systemctl", "daemon-reload"]).await?;
    run_privileged(&["systemctl", "enable", "--now", &unit]).await?;
    run_privileged(&["systemctl", "restart", &unit]).await?;
    Ok(())
}

/// systemctl wrapper for the frontend unit (start/stop/restart/status).
pub async fn frontend_ctl(s: &Store, action: &str) -> serde_json::Value {
    use serde_json::json;
    let unit = unit_name(&s.name);
    match action {
        "start" | "stop" | "restart" => match run_privileged(&["systemctl", action, &unit]).await {
            Ok(_) => json!({ "success": true, "message": format!("{unit} {action} ok") }),
            Err(e) => json!({ "success": false, "error": e }),
        },
        "status" => {
            let active = Command::new("systemctl")
                .args(["is-active", &unit])
                .output()
                .await
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .unwrap_or_else(|_| "unknown".into());
            json!({ "success": true, "unit": unit, "active": active == "active", "state": active })
        }
        "logs" => {
            let out = Command::new("journalctl")
                .args(["-u", &unit, "-n", "50", "--no-pager", "-o", "short-iso"])
                .output()
                .await;
            match out {
                Ok(o) => json!({
                    "success": true,
                    "lines": String::from_utf8_lossy(&o.stdout).lines().collect::<Vec<_>>(),
                }),
                Err(e) => json!({ "success": false, "error": e.to_string() }),
            }
        }
        other => json!({ "success": false, "error": format!("Unknown frontend action: {other}") }),
    }
}

/// Regenerate all web-facing config (vhosts + certs + frontend unit) from
/// the store's current fields — the backing for PATCH + apply.
pub async fn apply_web(s: &Store) -> Result<Vec<String>, String> {
    let mut notes: Vec<String> = Vec::new();
    let src = s.magento_src();
    let base = crate::port_base(s.id);
    // stored admin path from admin_url
    let admin_path = s
        .admin_url
        .as_deref()
        .and_then(|u| u.splitn(4, '/').nth(3).map(|p| format!("/{p}")))
        .unwrap_or_else(|| "/admin".to_string());

    // (re)issue the cert so newly added domains get SAN coverage
    let cert_paths = if s.ssl_mode != "none" {
        let issued = issue_cert(s).await?;
        notes.push(format!(
            "{} cert issued for {} domain(s)",
            s.ssl_mode,
            store_domains(s).len()
        ));
        Some(issued)
    } else {
        None
    };
    let ssl_ref = cert_paths.as_ref().map(|(c, k)| (c.as_str(), k.as_str()));

    write_nginx_extra_assets(s).await?;
    let server_extras = nginx_extras_snippet(s);
    let vhost_path = format!("/etc/nginx/sites-available/{}", s.name);
    if preserve_manual_vhost(s) && std::path::Path::new(&vhost_path).exists() {
        let reload = sk_web::nginx::reload().await;
        if !reload["success"].as_bool().unwrap_or(false) {
            return Err(format!(
                "nginx reload failed: {}",
                reload["error"]
                    .as_str()
                    .or(reload["message"].as_str())
                    .unwrap_or("?")
            ));
        }
        notes.push("manual nginx vhost preserved".into());
        return Ok(notes);
    }

    let nginx_user = sk_web::nginx::set_worker_user(&s.run_user, &s.run_user).await;
    if !nginx_user["success"].as_bool().unwrap_or(false) {
        return Err(format!(
            "nginx worker user update failed: {}",
            nginx_user["error"].as_str().unwrap_or("unknown error")
        ));
    }
    notes.push(format!(
        "nginx worker user set to {}:{}",
        s.run_user, s.run_user
    ));

    let vhost = match s.headless_mode.as_str() {
        "shared" => crate::compose::magento_vhost_headless_shared(
            &s.name,
            &s.domain,
            &src,
            &s.php_version,
            base + 7,
            &admin_path,
            s.frontend_port,
            s.frontend_root.as_deref(),
            &s.custom_routes(),
            &server_extras,
            ssl_ref,
        ),
        "split" => {
            let api_domain = s
                .api_domain
                .as_deref()
                .ok_or_else(|| "api_domain is required for split headless mode".to_string())?;
            crate::compose::magento_vhost_headless_split(
                &s.name,
                api_domain,
                s.admin_domain
                    .as_deref()
                    .unwrap_or(&format!("admin.{api_domain}")),
                &src,
                &s.php_version,
                base + 7,
                &admin_path,
                &s.split_route_mode,
                &server_extras,
                ssl_ref,
            )
        }
        _ if s.use_varnish => crate::compose::magento_vhost_varnish(
            &s.name,
            &s.domain,
            &src,
            &s.php_version,
            base + 6,
            base + 7,
            ssl_ref,
        ),
        _ if ssl_ref.is_some() => {
            let (cert, key) = cert_paths.as_ref().unwrap();
            crate::compose::magento_vhost_ssl(&s.name, &s.domain, &src, &s.php_version, cert, key)
        }
        _ => crate::compose::magento_vhost(&s.name, &s.domain, &src, &s.php_version),
    };

    write_privileged(&vhost_path, &vhost).await?;
    run_privileged(&[
        "ln",
        "-sf",
        &vhost_path,
        &format!("/etc/nginx/sites-enabled/{}", s.name),
    ])
    .await?;

    let fe_available = format!("/etc/nginx/sites-available/{}-frontend", s.name);
    let fe_enabled = format!("/etc/nginx/sites-enabled/{}-frontend", s.name);
    if matches!(s.headless_mode.as_str(), "separate" | "split") {
        if let Some(fd) = &s.frontend_domain {
            let fe_vhost = crate::compose::frontend_vhost(
                &s.name,
                fd,
                s.frontend_port,
                s.frontend_root.as_deref(),
                &server_extras,
                ssl_ref,
            );
            write_privileged(&fe_available, &fe_vhost).await?;
            run_privileged(&["ln", "-sf", &fe_available, &fe_enabled]).await?;
            notes.push(format!("frontend vhost for {fd} written"));
        }
    } else {
        let _ = run_privileged(&["rm", "-f", &fe_enabled]).await;
        let _ = run_privileged(&["rm", "-f", &fe_available]).await;
    }

    let reload = sk_web::nginx::reload().await;
    if !reload["success"].as_bool().unwrap_or(false) {
        return Err(format!(
            "nginx reload failed: {}",
            reload["error"]
                .as_str()
                .or(reload["message"].as_str())
                .unwrap_or("?")
        ));
    }
    notes.push("nginx reloaded".into());

    if s.frontend_cmd.is_some() {
        apply_frontend_unit(s).await?;
        notes.push("frontend unit applied".into());
    }
    Ok(notes)
}

/// Editable vhost update with a safety net: backup → write → `nginx -t` →
/// rollback on failure → reload on success.
pub async fn update_vhost(site_file: &str, content: &str) -> serde_json::Value {
    use serde_json::json;
    if site_file.contains('/') || site_file.contains("..") {
        return json!({ "success": false, "error": "Invalid site file name" });
    }
    let path = format!("/etc/nginx/sites-available/{site_file}");
    let backup = std::fs::read_to_string(&path).ok();
    if backup.is_none() {
        return json!({ "success": false, "error": "vhost does not exist" });
    }

    if let Err(e) = write_privileged(&path, content).await {
        return json!({ "success": false, "error": e });
    }

    let test = sk_web::nginx::test_config().await;
    if !test["success"].as_bool().unwrap_or(false) {
        // rollback
        if let Some(prev) = backup {
            let _ = write_privileged(&path, &prev).await;
        }
        return json!({
            "success": false,
            "error": format!("nginx config test failed — rolled back: {}",
                test["message"].as_str().unwrap_or("")),
        });
    }

    let reload = sk_web::nginx::reload().await;
    if reload["success"].as_bool().unwrap_or(false) {
        json!({ "success": true, "message": "vhost updated and nginx reloaded", "path": path })
    } else {
        reload
    }
}

/// Tear down a store: cron line, vhost, containers+volumes. Files/DB row are
/// removed by the caller according to flags.
pub async fn teardown(s: &Store, remove_files: bool) -> Vec<String> {
    let mut warnings = Vec::new();
    let root = &s.root_path;

    // cron (find by name via metadata list)
    let jobs = sk_ops::cron::list_jobs().await;
    if let Some(list) = jobs["jobs"].as_array() {
        for j in list {
            if j["name"].as_str() == Some(&format!("magento-{}", s.name)) {
                if let Some(jid) = j["id"].as_str() {
                    let r = sk_ops::cron::remove_job(jid).await;
                    if !r["success"].as_bool().unwrap_or(false) {
                        warnings.push("failed to remove cron job".into());
                    }
                }
            }
        }
    }

    // per-store php-fpm pool
    let _ = sk_web::php::delete_pool(&s.php_version, &s.fpm_pool()).await;

    // vhost(s)
    let _ = run_privileged(&["rm", "-f", &format!("/etc/nginx/sites-enabled/{}", s.name)]).await;
    let _ = run_privileged(&[
        "rm",
        "-f",
        &format!("/etc/nginx/sites-available/{}", s.name),
    ])
    .await;
    let _ = run_privileged(&[
        "rm",
        "-f",
        &format!("/etc/nginx/sites-enabled/{}-frontend", s.name),
    ])
    .await;
    let _ = run_privileged(&[
        "rm",
        "-f",
        &format!("/etc/nginx/sites-available/{}-frontend", s.name),
    ])
    .await;
    let _ = sk_web::nginx::reload().await;

    // managed frontend unit
    let unit = unit_name(&s.name);
    if std::path::Path::new(&format!("/etc/systemd/system/{unit}")).exists() {
        let _ = run_privileged(&["systemctl", "disable", "--now", &unit]).await;
        let _ = run_privileged(&["rm", "-f", &format!("/etc/systemd/system/{unit}")]).await;
        let _ = run_privileged(&["systemctl", "daemon-reload"]).await;
    }

    // containers + volumes
    let compose_path = format!("{root}/docker-compose.yml");
    if std::path::Path::new(&compose_path).exists() {
        let out = Command::new("docker")
            .args(["compose", "-f", &compose_path, "down", "-v"])
            .output()
            .await;
        if out.map(|o| !o.status.success()).unwrap_or(true) {
            warnings.push("docker compose down failed".into());
        }
    }

    if remove_files {
        if let Err(e) = std::fs::remove_dir_all(root) {
            warnings.push(format!("failed to remove {root}: {e}"));
        }
    }
    warnings
}
