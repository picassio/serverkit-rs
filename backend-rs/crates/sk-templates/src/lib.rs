//! sk-templates — the marketplace app-template engine. Ports
//! `app/services/template_service.py`: read YAML templates, resolve
//! `${SERVICE_*}` magic vars + declared vars + auto-allocated ports, render
//! the compose section, and deploy via `docker compose`.

mod resolve;

use serde_json::{json, Value as J};
use std::path::PathBuf;

fn templates_dir() -> PathBuf {
    PathBuf::from(
        std::env::var("SK_TEMPLATES_DIR").unwrap_or_else(|_| "../backend/templates".into()),
    )
}

fn installed_root() -> PathBuf {
    PathBuf::from(std::env::var("SK_APPS_DIR").unwrap_or_else(|_| "/var/www/serverkit-apps".into()))
}

fn registry_path() -> PathBuf {
    PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
        .join("installed_templates.json")
}

/// Parse a template YAML file into a serde_json Value (via serde_yaml).
fn parse_file(path: &std::path::Path) -> Option<J> {
    let text = std::fs::read_to_string(path).ok()?;
    let yaml: serde_yaml::Value = serde_yaml::from_str(&text).ok()?;
    serde_json::to_value(yaml).ok()
}

/// A compact catalog summary for one template.
fn summary(t: &J, id: &str) -> J {
    json!({
        "id": id,
        "name": t.get("name").cloned().unwrap_or(json!(id)),
        "version": t.get("version").cloned().unwrap_or(json!("")),
        "description": t.get("description").cloned().unwrap_or(json!("")),
        "icon": t.get("icon").cloned().unwrap_or(J::Null),
        "categories": t.get("categories").cloned().unwrap_or(json!([])),
        "website": t.get("website").cloned().unwrap_or(J::Null),
    })
}

/// `list_local_templates` — every YAML in the templates dir.
pub fn catalog() -> Vec<J> {
    let dir = templates_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    for e in entries.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if !(name.ends_with(".yaml") || name.ends_with(".yml")) {
            continue;
        }
        let id = name
            .rsplit_once('.')
            .map(|(a, _)| a)
            .unwrap_or(&name)
            .to_string();
        if let Some(t) = parse_file(&e.path()) {
            out.push(summary(&t, &id));
        }
    }
    out.sort_by(|a, b| {
        a["name"]
            .as_str()
            .unwrap_or("")
            .cmp(b["name"].as_str().unwrap_or(""))
    });
    out
}

/// `list_all_templates(category, search)`.
pub fn list(category: Option<&str>, search: Option<&str>) -> Vec<J> {
    catalog()
        .into_iter()
        .filter(|t| {
            let cat_ok = category.is_none_or(|c| {
                t["categories"]
                    .as_array()
                    .is_some_and(|arr| arr.iter().any(|x| x.as_str() == Some(c)))
            });
            let search_ok = search.is_none_or(|s| {
                let s = s.to_lowercase();
                let hay = format!(
                    "{} {}",
                    t["name"].as_str().unwrap_or(""),
                    t["description"].as_str().unwrap_or("")
                )
                .to_lowercase();
                hay.contains(&s)
            });
            cat_ok && search_ok
        })
        .collect()
}

/// `get_categories` — unique, sorted.
pub fn categories() -> Vec<String> {
    let mut set = std::collections::BTreeSet::new();
    for t in catalog() {
        if let Some(arr) = t["categories"].as_array() {
            for c in arr {
                if let Some(s) = c.as_str() {
                    set.insert(s.to_string());
                }
            }
        }
    }
    set.into_iter().collect()
}

const AUTO_TYPES: &[&str] = &["port", "password", "random", "uuid"];

/// `get_template` detail shape (variables normalized, has_compose, ports…).
pub fn detail(id: &str) -> Option<J> {
    if id.contains('/') || id.contains("..") {
        return None;
    }
    let path = templates_dir().join(format!("{id}.yaml"));
    let path = if path.exists() {
        path
    } else {
        templates_dir().join(format!("{id}.yml"))
    };
    let t = parse_file(&path)?;

    let mut variables = Vec::new();
    if let Some(list) = t.get("variables").and_then(|v| v.as_array()) {
        for v in list {
            let ty = v.get("type").and_then(|x| x.as_str()).unwrap_or("string");
            variables.push(json!({
                "name": v.get("name").cloned().unwrap_or(json!("")),
                "description": v.get("description").cloned().unwrap_or(json!("")),
                "type": ty,
                "default": v.get("default").cloned().unwrap_or(json!("")),
                "required": v.get("required").and_then(|x| x.as_bool()).unwrap_or(false),
                "options": v.get("options").cloned().unwrap_or(J::Null),
                "auto_generated": AUTO_TYPES.contains(&ty),
                "hidden": ty == "port" || v.get("hidden").and_then(|x| x.as_bool()).unwrap_or(false),
            }));
        }
    }

    Some(json!({
        "id": id,
        "name": t.get("name"),
        "version": t.get("version"),
        "description": t.get("description"),
        "icon": t.get("icon"),
        "categories": t.get("categories").cloned().unwrap_or(json!([])),
        "website": t.get("website"),
        "documentation": t.get("documentation"),
        "variables": variables,
        "has_compose": t.get("compose").is_some(),
        "ports": t.get("ports").cloned().unwrap_or(json!([])),
        "requirements": t.get("requirements").cloned().unwrap_or(json!({})),
        "post_install": t.get("post_install"),
    }))
}

pub fn app_name_valid(name: &str) -> bool {
    name.len() >= 3
        && name.len() <= 63
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// Render + deploy a template as `app_name`. Returns generated vars + ports.
pub async fn install(id: &str, app_name: &str, user_vars: &J) -> J {
    if !app_name_valid(app_name) {
        return json!({ "success": false, "error": "app_name must be 3-63 chars, lowercase/digits/hyphens, starting with a letter" });
    }
    let path = templates_dir().join(format!("{id}.yaml"));
    let path = if path.exists() {
        path
    } else {
        templates_dir().join(format!("{id}.yml"))
    };
    let Some(t) = parse_file(&path) else {
        return json!({ "success": false, "error": "template not found" });
    };
    let app_dir = installed_root().join(app_name);
    if app_dir.exists() {
        return json!({ "success": false, "error": format!("app '{app_name}' already exists") });
    }

    let rendered = match resolve::render_compose(&t, app_name, user_vars) {
        Ok(r) => r,
        Err(e) => return json!({ "success": false, "error": e }),
    };

    if let Err(e) = std::fs::create_dir_all(&app_dir) {
        return json!({ "success": false, "error": e.to_string() });
    }
    let compose_path = app_dir.join("docker-compose.yml");
    if let Err(e) = std::fs::write(&compose_path, &rendered.compose_yaml) {
        return json!({ "success": false, "error": e.to_string() });
    }

    // deploy
    let out = tokio::process::Command::new("docker")
        .args([
            "compose",
            "-p",
            app_name,
            "-f",
            &compose_path.to_string_lossy(),
            "up",
            "-d",
        ])
        .output()
        .await;
    match out {
        Ok(o) if o.status.success() => {
            record_install(
                app_name,
                id,
                &app_dir.to_string_lossy(),
                &rendered.ports,
                &rendered.generated,
            );
            json!({
                "success": true,
                "app_name": app_name,
                "template_id": id,
                "ports": rendered.ports,
                "generated": rendered.generated, // creator sees secrets once
                "path": compose_path.to_string_lossy(),
            })
        }
        Ok(o) => {
            let _ = std::fs::remove_dir_all(&app_dir);
            json!({ "success": false, "error": String::from_utf8_lossy(&o.stderr) })
        }
        Err(e) => {
            let _ = std::fs::remove_dir_all(&app_dir);
            json!({ "success": false, "error": e.to_string() })
        }
    }
}

fn load_registry() -> J {
    std::fs::read_to_string(registry_path())
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_else(|| json!({}))
}

fn save_registry(reg: &J) {
    let path = registry_path();
    if let Some(p) = path.parent() {
        let _ = std::fs::create_dir_all(p);
    }
    let _ = std::fs::write(path, serde_json::to_string_pretty(reg).unwrap_or_default());
}

fn record_install(app_name: &str, template_id: &str, dir: &str, ports: &[u16], generated: &J) {
    let mut reg = load_registry();
    reg[app_name] = json!({
        "template_id": template_id,
        "dir": dir,
        "ports": ports,
        // secrets are masked in the registry; the install response revealed them once
        "generated_keys": generated.as_object().map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default(),
        "created_at": chrono::Local::now().naive_local().format("%Y-%m-%dT%H:%M:%S").to_string(),
    });
    save_registry(&reg);
}

pub fn list_installed() -> Vec<J> {
    load_registry()
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(name, v)| {
                    let mut o = v.clone();
                    o["name"] = json!(name);
                    o
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Tear down an installed template app (compose down -v + files + registry).
pub async fn uninstall(app_name: &str) -> J {
    if !app_name_valid(app_name) {
        return json!({ "success": false, "error": "invalid app name" });
    }
    let reg = load_registry();
    let Some(entry) = reg.get(app_name) else {
        return json!({ "success": false, "error": "app not found" });
    };
    let dir = entry["dir"].as_str().unwrap_or("").to_string();
    let compose = format!("{dir}/docker-compose.yml");
    if std::path::Path::new(&compose).exists() {
        let _ = tokio::process::Command::new("docker")
            .args(["compose", "-p", app_name, "-f", &compose, "down", "-v"])
            .output()
            .await;
    }
    if !dir.is_empty() {
        let _ = std::fs::remove_dir_all(&dir);
    }
    let mut reg = load_registry();
    reg.as_object_mut().map(|m| m.remove(app_name));
    save_registry(&reg);
    json!({ "success": true, "message": format!("{app_name} uninstalled") })
}

#[cfg(test)]
mod tests {
    #[test]
    fn app_names() {
        assert!(super::app_name_valid("uptime-kuma"));
        assert!(!super::app_name_valid("Ab"));
        assert!(!super::app_name_valid("-bad"));
        assert!(!super::app_name_valid("a"));
    }
}
