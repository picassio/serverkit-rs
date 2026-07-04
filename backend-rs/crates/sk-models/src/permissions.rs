//! Role → permission templates, ported verbatim from `app/models/user.py`.

use serde_json::{json, Value};

pub const PERMISSION_FEATURES: &[&str] = &[
    "applications",
    "databases",
    "docker",
    "domains",
    "files",
    "monitoring",
    "backups",
    "security",
    "email",
    "git",
    "cron",
    "terminal",
    "users",
    "settings",
    "servers",
];

fn perm(read: bool, write: bool) -> Value {
    json!({ "read": read, "write": write })
}

pub fn role_template(role: &str) -> Value {
    match role {
        "admin" => {
            let mut m = serde_json::Map::new();
            for f in PERMISSION_FEATURES {
                m.insert(f.to_string(), perm(true, true));
            }
            Value::Object(m)
        }
        "developer" => json!({
            "applications": perm(true, true),
            "databases":    perm(true, true),
            "docker":       perm(true, true),
            "domains":      perm(true, true),
            "files":        perm(true, true),
            "email":        perm(true, true),
            "git":          perm(true, true),
            "cron":         perm(true, true),
            "monitoring":   perm(true, false),
            "backups":      perm(true, false),
            "security":     perm(true, false),
            "terminal":     perm(true, false),
            "servers":      perm(true, false),
            "users":        perm(false, false),
            "settings":     perm(false, false),
        }),
        "viewer" => json!({
            "applications": perm(true, false),
            "databases":    perm(true, false),
            "docker":       perm(true, false),
            "domains":      perm(true, false),
            "files":        perm(true, false),
            "email":        perm(true, false),
            "git":          perm(true, false),
            "cron":         perm(true, false),
            "monitoring":   perm(true, false),
            "backups":      perm(true, false),
            "security":     perm(true, false),
            "terminal":     perm(false, false),
            "users":        perm(false, false),
            "settings":     perm(false, false),
            "servers":      perm(true, false),
        }),
        _ => json!({}),
    }
}
