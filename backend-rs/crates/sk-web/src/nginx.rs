//! `NginxService` port — vhost templates, sites-available/enabled management.
//! Templates copied verbatim from the Flask service (they are the contract
//! for what lands in /etc/nginx).

use crate::{privileged, privileged_with_stdin, run, service_is_active};
use serde_json::{json, Value};
use std::path::Path;

const SITES_AVAILABLE: &str = "/etc/nginx/sites-available";
const SITES_ENABLED: &str = "/etc/nginx/sites-enabled";

pub const SUPPORTED_NGINX_TARGETS: &[&str] = &["distro", "stable", "mainline", "1.28", "1.30"];

const PHP_SITE_TEMPLATE: &str = r#"server {
    listen 80;
    listen [::]:80;
    server_name {domains};

    root {root_path};
    index index.php index.html index.htm;

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {
        try_files $uri $uri/ /index.php?$query_string;
    }

    location ~ \.php$ {
        fastcgi_pass unix:/run/php/php{php_version}-fpm.sock;
        fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;
        include fastcgi_params;
        fastcgi_intercept_errors on;
        fastcgi_buffer_size 16k;
        fastcgi_buffers 4 16k;
    }

    location ~ /\.ht {
        deny all;
    }

    location = /favicon.ico {
        log_not_found off;
        access_log off;
    }

    location = /robots.txt {
        log_not_found off;
        access_log off;
        allow all;
    }

    location ~* \.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2)$ {
        expires 1y;
        log_not_found off;
    }
}
"#;

const PROXY_SITE_TEMPLATE: &str = r#"server {
    listen 80;
    listen [::]:80;
    server_name {domains};

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {
        proxy_pass http://127.0.0.1:{port};
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }
{python_static}}
"#;

const PYTHON_STATIC_BLOCK: &str = r#"
    location /static {
        alias {root_path}/static;
        expires 1y;
    }
"#;

const STATIC_SITE_TEMPLATE: &str = r#"server {
    listen 80;
    listen [::]:80;
    server_name {domains};

    root {root_path};
    index index.html index.htm;

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {
        try_files $uri $uri/ =404;
    }

    location ~* \.(css|gif|ico|jpeg|jpg|js|png|svg|woff|woff2)$ {
        expires 1y;
        log_not_found off;
    }
}
"#;

const SSL_BLOCK: &str = r#"
    listen 443 ssl http2;
    listen [::]:443 ssl http2;

    ssl_certificate {ssl_cert};
    ssl_certificate_key {ssl_key};
    ssl_session_timeout 1d;
    ssl_session_cache shared:SSL:50m;
    ssl_session_tickets off;

    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers off;
    ssl_ecdh_curve X25519:secp384r1;

    add_header Strict-Transport-Security "max-age=63072000; includeSubDomains; preload" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header Referrer-Policy "strict-origin-when-cross-origin" always;
    add_header Content-Security-Policy "default-src 'self' 'unsafe-inline' 'unsafe-eval' data: blob: https:; frame-ancestors 'self'; upgrade-insecure-requests" always;
"#;

const SSL_REDIRECT_TEMPLATE: &str = r#"server {
    listen 80;
    listen [::]:80;
    server_name {domains};
    return 301 https://$server_name$request_uri;
}
"#;

fn tmpl(template: &str, vars: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in vars {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

fn validate_domain(domain: &str) -> bool {
    let re =
        regex::Regex::new(r"^(\*\.)?([a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?\.)+[a-zA-Z]{2,}$")
            .unwrap();
    re.is_match(domain) || domain == "localhost"
}

/// Config names must be simple filenames — no traversal.
fn validate_name(name: &str) -> bool {
    !name.is_empty() && !name.contains('/') && !name.contains("..") && !name.starts_with('.')
}

pub async fn installed_version() -> Option<String> {
    let r = run(&["nginx", "-v"], 10).await;
    let out = if r.stderr.is_empty() {
        r.stdout
    } else {
        r.stderr
    };
    out.split("nginx/").nth(1).map(|v| v.trim().to_string())
}

pub async fn install_version(target: &str) -> Value {
    if !SUPPORTED_NGINX_TARGETS.contains(&target) {
        return json!({ "success": false, "error": format!("Unsupported nginx target: {target}") });
    }

    let script = if target == "distro" {
        "set -e; apt-get update; apt-get install -y nginx; systemctl enable nginx; systemctl restart nginx".to_string()
    } else {
        let channel = if target == "stable" || target == "1.28" {
            "stable"
        } else {
            "mainline"
        };
        let exact_filter = if target
            .chars()
            .next()
            .map(|c| c.is_ascii_digit())
            .unwrap_or(false)
        {
            format!(" | grep -m1 '{}.'", target)
        } else {
            String::new()
        };
        format!(
            r#"set -e
apt-get update
apt-get install -y curl gnupg2 ca-certificates lsb-release ubuntu-keyring
curl -fsSL https://nginx.org/keys/nginx_signing.key | gpg --dearmor -o /usr/share/keyrings/nginx-archive-keyring.gpg
CODENAME=$(lsb_release -cs)
echo 'deb [signed-by=/usr/share/keyrings/nginx-archive-keyring.gpg] http://nginx.org/packages/{channel}/ubuntu/ '$CODENAME' nginx' > /etc/apt/sources.list.d/nginx-org.list
apt-get update
VERSION=$(apt-cache madison nginx{exact_filter} | awk '{{print $3; exit}}')
if [ -n "$VERSION" ]; then apt-get install -y nginx="$VERSION"; else apt-get install -y nginx; fi
systemctl enable nginx
systemctl restart nginx
"#
        )
    };
    let r = privileged(&["/bin/sh", "-lc", &script], 900).await;
    if r.ok {
        json!({ "success": true, "message": format!("nginx target {target} installed"), "version": installed_version().await })
    } else {
        json!({ "success": false, "error": r.stderr, "stdout": r.stdout })
    }
}

/// `NginxService.get_status`
pub async fn status() -> Value {
    let running = service_is_active("nginx").await;
    let details = run(&["systemctl", "status", "nginx", "--no-pager"], 30).await;
    json!({
        "running": running,
        "status": if running { "running" } else { "stopped" },
        "version": installed_version().await,
        "supported_targets": SUPPORTED_NGINX_TARGETS,
        "details": details.stdout,
    })
}

/// `NginxService.test_config` — `nginx -t`.
pub async fn test_config() -> Value {
    let r = privileged(&["/usr/sbin/nginx", "-t"], 30).await;
    json!({ "success": r.ok, "message": r.stderr })
}

/// `NginxService.reload` — test first, then systemctl reload.
pub async fn reload() -> Value {
    let test = test_config().await;
    if !test["success"].as_bool().unwrap_or(false) {
        return json!({
            "success": false,
            "error": format!("Config test failed: {}", test["message"].as_str().unwrap_or(""))
        });
    }
    let r = privileged(&["systemctl", "reload", "nginx"], 30).await;
    json!({
        "success": r.ok,
        "message": if r.ok { "Nginx reloaded successfully".into() } else { r.stderr },
    })
}

/// `NginxService.restart`
pub async fn restart() -> Value {
    let r = privileged(&["systemctl", "restart", "nginx"], 30).await;
    json!({
        "success": r.ok,
        "message": if r.ok { "Nginx restarted successfully".into() } else { r.stderr },
    })
}

/// `NginxService.list_sites` — parse sites-available, mark enabled.
pub fn list_sites() -> Vec<Value> {
    let Ok(available) = std::fs::read_dir(SITES_AVAILABLE) else {
        return Vec::new();
    };
    let enabled: std::collections::HashSet<String> = std::fs::read_dir(SITES_ENABLED)
        .map(|it| {
            it.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();

    let server_name_re = regex::Regex::new(r"server_name\s+([^;]+);").unwrap();
    let root_re = regex::Regex::new(r"root\s+([^;]+);").unwrap();

    let mut sites = Vec::new();
    for entry in available.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !entry.path().is_file() {
            continue;
        }
        let content = std::fs::read_to_string(entry.path()).unwrap_or_default();
        let domains: Vec<String> = server_name_re
            .captures(&content)
            .map(|c| {
                c[1].split_whitespace()
                    .filter(|d| *d != "_")
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let root = root_re.captures(&content).map(|c| c[1].trim().to_string());
        sites.push(json!({
            "name": name,
            "enabled": enabled.contains(&name),
            "domains": domains,
            "root": root,
            "ssl": content.contains("listen 443"),
        }));
    }
    sites
}

pub struct SiteSpec {
    pub name: String,
    pub app_type: String,
    pub domains: Vec<String>,
    pub root_path: String,
    pub port: Option<u16>,
    pub php_version: String,
    pub ssl_cert: Option<String>,
    pub ssl_key: Option<String>,
}

/// `NginxService.create_site`
pub async fn create_site(spec: &SiteSpec) -> Value {
    if spec.domains.is_empty() {
        return json!({ "success": false, "error": "At least one domain is required" });
    }
    if !validate_name(&spec.name) {
        return json!({ "success": false, "error": "Invalid site name" });
    }
    for d in &spec.domains {
        if d == "_" {
            return json!({ "success": false, "error": "Wildcard server_name \"_\" is reserved for the ServerKit panel" });
        }
        if !validate_domain(d) {
            return json!({ "success": false, "error": format!("Invalid domain name: {d}") });
        }
    }

    let domains = spec.domains.join(" ");
    let port = spec.port.map(|p| p.to_string()).unwrap_or_default();

    let mut config = match spec.app_type.as_str() {
        "php" | "wordpress" | "magento" => tmpl(
            PHP_SITE_TEMPLATE,
            &[
                ("name", &spec.name),
                ("domains", &domains),
                ("root_path", &spec.root_path),
                ("php_version", &spec.php_version),
            ],
        ),
        "flask" | "django" | "python" => {
            if spec.port.is_none() {
                return json!({ "success": false, "error": "Port is required for Python apps" });
            }
            let static_block = tmpl(PYTHON_STATIC_BLOCK, &[("root_path", &spec.root_path)]);
            tmpl(
                PROXY_SITE_TEMPLATE,
                &[
                    ("name", &spec.name),
                    ("domains", &domains),
                    ("port", &port),
                    ("python_static", &static_block),
                ],
            )
        }
        "docker" => {
            if spec.port.is_none() {
                return json!({ "success": false, "error": "Port is required for Docker apps" });
            }
            tmpl(
                PROXY_SITE_TEMPLATE,
                &[
                    ("name", &spec.name),
                    ("domains", &domains),
                    ("port", &port),
                    ("python_static", ""),
                ],
            )
        }
        "static" => tmpl(
            STATIC_SITE_TEMPLATE,
            &[
                ("name", &spec.name),
                ("domains", &domains),
                ("root_path", &spec.root_path),
            ],
        ),
        other => return json!({ "success": false, "error": format!("Unknown app type: {other}") }),
    };

    // HTTPS variant: swap :80 listens for the TLS block + prepend redirect
    if let (Some(cert), Some(key)) = (&spec.ssl_cert, &spec.ssl_key) {
        let ssl = tmpl(SSL_BLOCK, &[("ssl_cert", cert), ("ssl_key", key)]);
        config = config.replacen(
            "    listen 80;\n    listen [::]:80;",
            ssl.trim_matches('\n'),
            1,
        );
        let redirect = tmpl(SSL_REDIRECT_TEMPLATE, &[("domains", &domains)]);
        config = format!("{redirect}\n{config}");
    }

    let config_path = format!("{SITES_AVAILABLE}/{}", spec.name);
    let r = privileged_with_stdin(&["tee", &config_path], Some(&config), 30).await;
    if r.ok {
        json!({ "success": true, "message": format!("Site {} created", spec.name), "path": config_path })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `NginxService.enable_site` — symlink + reload.
pub async fn enable_site(name: &str) -> Value {
    if !validate_name(name) {
        return json!({ "success": false, "error": "Invalid site name" });
    }
    let available = format!("{SITES_AVAILABLE}/{name}");
    let enabled = format!("{SITES_ENABLED}/{name}");
    if !Path::new(&available).exists() {
        return json!({ "success": false, "error": format!("Site {name} not found in sites-available") });
    }
    let r = privileged(&["ln", "-sf", &available, &enabled], 15).await;
    if !r.ok {
        return json!({ "success": false, "error": r.stderr });
    }
    let reload_result = reload().await;
    if reload_result["success"].as_bool().unwrap_or(false) {
        json!({ "success": true, "message": format!("Site {name} enabled") })
    } else {
        reload_result
    }
}

/// `NginxService.disable_site`
pub async fn disable_site(name: &str) -> Value {
    if !validate_name(name) {
        return json!({ "success": false, "error": "Invalid site name" });
    }
    let enabled = format!("{SITES_ENABLED}/{name}");
    let r = privileged(&["rm", "-f", &enabled], 15).await;
    if !r.ok {
        return json!({ "success": false, "error": r.stderr });
    }
    let reload_result = reload().await;
    if reload_result["success"].as_bool().unwrap_or(false) {
        json!({ "success": true, "message": format!("Site {name} disabled") })
    } else {
        reload_result
    }
}

/// `NginxService.delete_site`
pub async fn delete_site(name: &str) -> Value {
    if !validate_name(name) {
        return json!({ "success": false, "error": "Invalid site name" });
    }
    let _ = disable_site(name).await;
    let available = format!("{SITES_AVAILABLE}/{name}");
    let r = privileged(&["rm", "-f", &available], 15).await;
    if r.ok {
        json!({ "success": true, "message": format!("Site {name} deleted") })
    } else {
        json!({ "success": false, "error": r.stderr })
    }
}

/// `NginxService.add_ssl_to_site`
pub async fn add_ssl_to_site(name: &str, cert_path: &str, key_path: &str) -> Value {
    if !validate_name(name) {
        return json!({ "success": false, "error": "Invalid site name" });
    }
    let config_path = format!("{SITES_AVAILABLE}/{name}");
    let Ok(content) = std::fs::read_to_string(&config_path) else {
        return json!({ "success": false, "error": format!("Site {name} not found") });
    };

    let server_name_re = regex::Regex::new(r"server_name\s+([^;]+);").unwrap();
    let domains = server_name_re
        .captures(&content)
        .map(|c| c[1].trim().to_string())
        .unwrap_or_else(|| name.to_string());

    let ssl_config = tmpl(SSL_BLOCK, &[("ssl_cert", cert_path), ("ssl_key", key_path)]);
    let redirect = tmpl(SSL_REDIRECT_TEMPLATE, &[("domains", &domains)]);
    let new_content = content.replace("listen 80;", &format!("listen 80;\n{ssl_config}"));
    let final_content = format!("{redirect}\n{new_content}");

    let r = privileged_with_stdin(&["tee", &config_path], Some(&final_content), 30).await;
    if !r.ok {
        return json!({ "success": false, "error": r.stderr });
    }
    let reload_result = reload().await;
    if reload_result["success"].as_bool().unwrap_or(false) {
        json!({ "success": true, "message": format!("SSL added to site {name}") })
    } else {
        reload_result
    }
}

pub fn site_by_name(name: &str) -> Option<Value> {
    list_sites()
        .into_iter()
        .find(|s| s["name"].as_str() == Some(name))
}

pub fn raw_site_config(name: &str) -> Option<String> {
    if !validate_name(name) {
        return None;
    }
    std::fs::read_to_string(format!("{SITES_AVAILABLE}/{name}")).ok()
}

pub async fn resolve_domain(domain: &str) -> Value {
    match tokio::process::Command::new("getent")
        .args(["ahosts", domain])
        .output()
        .await
    {
        Ok(o) if o.status.success() => {
            json!({"success":true,"output":String::from_utf8_lossy(&o.stdout).to_string()})
        }
        Ok(o) => json!({"success":false,"error":String::from_utf8_lossy(&o.stderr).to_string()}),
        Err(e) => json!({"success":false,"error":e.to_string()}),
    }
}

pub fn lb_methods() -> Value {
    json!({"methods":[
        {"key":"round_robin","name":"Round robin","nginx_directive":null},
        {"key":"least_conn","name":"Least connections","nginx_directive":"least_conn;"},
        {"key":"ip_hash","name":"IP hash","nginx_directive":"ip_hash;"},
        {"key":"hash","name":"Consistent hash","nginx_directive":"hash $request_uri consistent;"}
    ]})
}

pub async fn proxy_rules(name: &str) -> Value {
    let Some(config) = raw_site_config(name) else {
        return json!({"success":false,"error":"site not found","rules":[]});
    };
    let proxy_re = regex::Regex::new(r"proxy_pass\s+([^;]+);").unwrap();
    let rules: Vec<Value> = proxy_re
        .captures_iter(&config)
        .map(|c| json!({"proxy_pass":c[1].to_string()}))
        .collect();
    json!({"success":true,"site":name,"rules":rules,"config":config})
}

pub async fn create_proxy_site(name: &str, domain: &str, upstream: &str) -> Value {
    if !validate_name(name) || !validate_domain(domain) {
        return json!({"success":false,"error":"invalid name or domain"});
    }
    let config = format!(
        r#"server {{
    listen 80;
    listen [::]:80;
    server_name {domain};

    access_log /var/log/nginx/{name}.access.log;
    error_log /var/log/nginx/{name}.error.log;

    location / {{
        proxy_pass {upstream};
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }}
}}
"#
    );
    let path = format!("{SITES_AVAILABLE}/{name}");
    let r = privileged_with_stdin(&["tee", &path], Some(&config), 30).await;
    if r.ok {
        json!({"success":true,"site":name,"path":path})
    } else {
        json!({"success":false,"error":r.stderr})
    }
}

pub async fn diff_site_config(name: &str, proposed: &str) -> Value {
    let current = raw_site_config(name).unwrap_or_default();
    let current_lines: Vec<&str> = current.lines().collect();
    let proposed_lines: Vec<&str> = proposed.lines().collect();
    let mut diff = Vec::new();
    let max = current_lines.len().max(proposed_lines.len());
    for i in 0..max {
        match (current_lines.get(i), proposed_lines.get(i)) {
            (Some(a), Some(b)) if a == b => diff.push(format!(" {}", a)),
            (Some(a), Some(b)) => {
                diff.push(format!("-{}", a));
                diff.push(format!("+{}", b));
            }
            (Some(a), None) => diff.push(format!("-{}", a)),
            (None, Some(b)) => diff.push(format!("+{}", b)),
            (None, None) => {}
        }
    }
    json!({"success":true,"site":name,"changed":current != proposed,"current":current,"proposed":proposed,"diff":diff.join("\n")})
}

pub fn vhost_logs(name: &str, kind: &str, lines: usize) -> Value {
    if !validate_name(name) {
        return json!({"success":false,"error":"invalid site name"});
    }
    let file = format!(
        "/var/log/nginx/{name}.{}.log",
        if kind == "error" { "error" } else { "access" }
    );
    let output = std::process::Command::new("tail")
        .args(["-n", &lines.min(1000).to_string(), &file])
        .output();
    match output {
        Ok(o) if o.status.success() => {
            json!({"success":true,"path":file,"lines":String::from_utf8_lossy(&o.stdout).lines().collect::<Vec<_>>() })
        }
        Ok(o) => {
            json!({"success":false,"path":file,"error":String::from_utf8_lossy(&o.stderr).trim()})
        }
        Err(e) => json!({"success":false,"path":file,"error":e.to_string()}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_validation() {
        assert!(validate_domain(
            "store.magento.local".replace(".local", ".com").as_str()
        ));
        assert!(validate_domain("*.example.com"));
        assert!(!validate_domain("bad domain"));
        assert!(!validate_domain("-bad.com"));
    }

    #[test]
    fn name_validation() {
        assert!(validate_name("store.example.com"));
        assert!(!validate_name("../../etc/passwd"));
        assert!(!validate_name(".hidden"));
    }

    #[test]
    fn php_template_renders() {
        let out = tmpl(
            PHP_SITE_TEMPLATE,
            &[
                ("name", "shop"),
                ("domains", "shop.test"),
                ("root_path", "/var/www/shop"),
                ("php_version", "8.3"),
            ],
        );
        assert!(out.contains("fastcgi_pass unix:/run/php/php8.3-fpm.sock;"));
        assert!(out.contains("server_name shop.test;"));
        assert!(!out.contains('{') || out.contains("location"));
    }
}
