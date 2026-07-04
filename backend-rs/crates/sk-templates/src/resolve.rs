//! Variable resolution + compose rendering. Ports the `${SERVICE_*}` magic
//! tokens and declared-variable substitution from `template_service.py`.

use rand::Rng;
use regex::Regex;
use serde_json::{json, Value as J};
use std::collections::HashMap;

pub struct Rendered {
    pub compose_yaml: String,
    pub generated: J,    // magic + generated values (secrets) revealed once
    pub ports: Vec<u16>, // allocated host ports
}

fn magic_password() -> String {
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..24)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

fn magic_user(name: &str) -> String {
    let base: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect();
    let base = base.trim_matches('_');
    let base = if base.is_empty() { "service" } else { base };
    let suffix: String = (0..2)
        .map(|_| format!("{:02x}", rand::thread_rng().gen::<u8>()))
        .collect();
    format!("svc_{base}_{suffix}")
}

fn magic_base64() -> String {
    use std::fmt::Write;
    let bytes: [u8; 24] = rand::thread_rng().gen();
    // base64 standard
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        let _ = write!(
            out,
            "{}{}",
            T[((n >> 18) & 63) as usize] as char,
            T[((n >> 12) & 63) as usize] as char
        );
        if chunk.len() > 1 {
            out.push(T[((n >> 6) & 63) as usize] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(T[(n & 63) as usize] as char);
        } else {
            out.push('=');
        }
    }
    out
}

/// Bind-test a host port on 127.0.0.1 to check it's free.
fn port_free(port: u16) -> bool {
    std::net::TcpListener::bind(("127.0.0.1", port)).is_ok()
}

/// Allocate a free port in [20000, 29999], avoiding `taken`.
fn alloc_port(taken: &mut Vec<u16>) -> u16 {
    let mut rng = rand::thread_rng();
    for _ in 0..500 {
        let p = rng.gen_range(20000..30000);
        if !taken.contains(&p) && port_free(p) {
            taken.push(p);
            return p;
        }
    }
    // fallback: OS-assigned
    let p = std::net::TcpListener::bind("127.0.0.1:0")
        .ok()
        .and_then(|l| l.local_addr().ok())
        .map(|a| a.port())
        .unwrap_or(20000);
    taken.push(p);
    p
}

fn classify_magic(token: &str) -> Option<(&'static str, String)> {
    for (prefix, kind) in [
        ("SERVICE_PASSWORD_", "password"),
        ("SERVICE_USER_", "user"),
        ("SERVICE_FQDN_", "fqdn"),
        ("SERVICE_URL_", "url"),
        ("SERVICE_BASE64_", "base64"),
    ] {
        if let Some(name) = token.strip_prefix(prefix) {
            return Some((kind, name.to_string()));
        }
    }
    None
}

/// Resolve every `${...}` token in the template's compose section and render
/// it to a docker-compose YAML string.
pub fn render_compose(template: &J, app_name: &str, user_vars: &J) -> Result<Rendered, String> {
    let compose = template
        .get("compose")
        .ok_or("template has no compose section")?;
    let compose_str =
        serde_yaml::to_string(compose).map_err(|e| format!("compose serialize: {e}"))?;

    // Declared variables: name -> (type, default)
    let mut declared: HashMap<String, (String, String)> = HashMap::new();
    if let Some(list) = template.get("variables").and_then(|v| v.as_array()) {
        for v in list {
            if let Some(name) = v.get("name").and_then(|x| x.as_str()) {
                let ty = v
                    .get("type")
                    .and_then(|x| x.as_str())
                    .unwrap_or("string")
                    .to_string();
                let def = v
                    .get("default")
                    .map(|d| {
                        d.as_str()
                            .map(str::to_string)
                            .unwrap_or_else(|| d.to_string())
                    })
                    .unwrap_or_default();
                declared.insert(name.to_string(), (ty, def));
            }
        }
    }

    let token_re = Regex::new(r"\$\{([A-Z0-9_]+)\}").unwrap();
    let user = user_vars.as_object();
    let mut values: HashMap<String, String> = HashMap::new();
    let mut generated = serde_json::Map::new();
    let mut ports: Vec<u16> = Vec::new();
    let mut taken_ports: Vec<u16> = Vec::new();

    // collect unique tokens
    let mut tokens: Vec<String> = token_re
        .captures_iter(&compose_str)
        .map(|c| c[1].to_string())
        .collect();
    tokens.sort();
    tokens.dedup();

    for token in tokens {
        if values.contains_key(&token) {
            continue;
        }
        // 1. APP_NAME
        if token == "APP_NAME" {
            values.insert(token, app_name.to_string());
            continue;
        }
        // 2. magic SERVICE_* tokens
        if let Some((kind, name)) = classify_magic(&token) {
            let val = match kind {
                "password" => magic_password(),
                "user" => magic_user(&name),
                "base64" => magic_base64(),
                "fqdn" => app_name.to_string(),
                "url" => format!("http://{app_name}"),
                _ => String::new(),
            };
            generated.insert(token.clone(), json!(val));
            values.insert(token, val);
            continue;
        }
        // 3. user-provided value wins
        if let Some(v) = user.and_then(|u| u.get(&token)).and_then(|v| {
            v.as_str()
                .map(str::to_string)
                .or_else(|| Some(v.to_string()))
        }) {
            if !v.is_empty() && v != "null" {
                // a user-provided port is honored as a fixed port
                if declared
                    .get(&token)
                    .map(|(t, _)| t == "port")
                    .unwrap_or(false)
                {
                    if let Ok(p) = v.parse::<u16>() {
                        ports.push(p);
                        taken_ports.push(p);
                    }
                }
                values.insert(token, v);
                continue;
            }
        }
        // 4. declared variable
        if let Some((ty, def)) = declared.get(&token).cloned() {
            if ty == "port" {
                let p = alloc_port(&mut taken_ports);
                ports.push(p);
                values.insert(token, p.to_string());
            } else if ty == "password" {
                let v = magic_password();
                generated.insert(token.clone(), json!(v));
                values.insert(token, v);
            } else {
                values.insert(token, def);
            }
            continue;
        }
        // 5. unknown token -> leave empty
        values.insert(token, String::new());
    }

    let rendered = token_re
        .replace_all(&compose_str, |c: &regex::Captures| {
            values.get(&c[1]).cloned().unwrap_or_default()
        })
        .into_owned();

    Ok(Rendered {
        compose_yaml: rendered,
        generated: J::Object(generated),
        ports,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resolves_app_name_and_port() {
        let t = json!({
            "variables": [{ "name": "HTTP_PORT", "type": "port", "default": "3001" }],
            "compose": {
                "services": { "app": {
                    "image": "louislam/uptime-kuma:1",
                    "container_name": "${APP_NAME}",
                    "ports": ["${HTTP_PORT}:3001"]
                }}
            }
        });
        let r = render_compose(&t, "mykuma", &json!({})).unwrap();
        assert!(r.compose_yaml.contains("container_name: mykuma"));
        assert_eq!(r.ports.len(), 1);
        assert!(r.compose_yaml.contains(&format!("{}:3001", r.ports[0])));
        assert!(!r.compose_yaml.contains("${"));
    }

    #[test]
    fn resolves_magic_password() {
        let t = json!({
            "compose": { "services": { "db": {
                "image": "postgres:16",
                "environment": { "POSTGRES_PASSWORD": "${SERVICE_PASSWORD_DB}" }
            }}}
        });
        let r = render_compose(&t, "app1", &json!({})).unwrap();
        assert!(r.generated.get("SERVICE_PASSWORD_DB").is_some());
        assert!(!r.compose_yaml.contains("${SERVICE_PASSWORD_DB}"));
    }
}
