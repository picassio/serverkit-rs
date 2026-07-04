//! sk-magento — Magento store lifecycle management.
//!
//! The reason this fork exists. Ports the magento-vm-provisioner architecture
//! into the panel: nginx + PHP-FPM native, data services (MariaDB, OpenSearch,
//! Redis, Mailpit) per-store in Docker Compose, exact Composer-per-Magento
//! version matrix, `bin/magento` quick actions, cron + indexer health.
//!
//! Distributions: `mage-os` (default — installs without repo.magento.com auth
//! keys) and `magento` (requires COMPOSER_AUTH / auth.json).

pub mod actions;
pub mod backup;
pub mod compose;
pub mod provision;
pub mod store;

use serde_json::{json, Value};

/// Magento → (exact Composer, recommended PHP, OpenSearch supported).
/// From the magento-vm-provisioner matrix.
pub const VERSION_MATRIX: &[(&str, &str, &str)] = &[
    // (magento_series, composer, php)
    ("2.4.8", "2.9.3", "8.3"),
    ("2.4.7", "2.7.9", "8.3"),
    ("2.4.6", "2.2.22", "8.2"),
    ("2.4.5", "2.2.22", "8.1"),
    ("2.4.4", "2.2.22", "8.1"),
    ("2.4.3", "2.2.22", "7.4"),
    ("2.4.2", "2.2.22", "7.4"),
    ("2.4.1", "2.2.22", "7.4"),
    ("2.4.0", "2.2.22", "7.4"),
];

pub fn matrix_lookup(magento_version: &str) -> Option<(&'static str, &'static str)> {
    let series = magento_version
        .split('.')
        .take(3)
        .collect::<Vec<_>>()
        .join(".");
    VERSION_MATRIX
        .iter()
        .find(|(m, _, _)| series.starts_with(m))
        .map(|(_, composer, php)| (*composer, *php))
}

pub fn versions_payload() -> Value {
    json!({
        "versions": VERSION_MATRIX.iter().map(|(m, c, p)| json!({
            "magento": m, "composer": c, "php": p
        })).collect::<Vec<_>>(),
        "distributions": [
            { "id": "mage-os", "label": "Mage-OS (no auth keys required)", "repository": "https://repo.mage-os.org/" },
            { "id": "magento", "label": "Magento Open Source (requires repo.magento.com keys)", "repository": "https://repo.magento.com/" },
        ],
        "search_engines": ["opensearch"],
        // Data-plane image defaults — override any per store via service_versions.
        "service_versions": compose::default_service_versions(),
        "ssl_modes": ["none", "self-signed", "letsencrypt"],
        "le_challenges": ["dns", "http"],
        "run_user_default": "www-data",
    })
}

/// Deterministic host-port block per store: base + {0:db, 1:search, 2:redis,
/// 3:amqp, 4:smtp, 5:mail-ui}. Stores get non-overlapping 10-port windows.
pub fn port_base(store_id: i64) -> u16 {
    (34000 + store_id * 10) as u16
}

pub fn generate_password(len: usize) -> String {
    use rand::Rng;
    const CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| CHARS[rng.gen_range(0..CHARS.len())] as char)
        .collect()
}

/// Store slugs become dirs, container names and DB identifiers.
pub fn valid_store_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 32
        && name
            .chars()
            .next()
            .map(|c| c.is_ascii_lowercase())
            .unwrap_or(false)
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix() {
        assert_eq!(matrix_lookup("2.4.8"), Some(("2.9.3", "8.3")));
        assert_eq!(matrix_lookup("2.4.8-p1"), Some(("2.9.3", "8.3")));
        assert_eq!(matrix_lookup("2.4.4"), Some(("2.2.22", "8.1")));
        assert_eq!(matrix_lookup("1.9.0"), None);
    }

    #[test]
    fn store_names() {
        assert!(valid_store_name("shop1"));
        assert!(valid_store_name("my-store"));
        assert!(!valid_store_name("My_Store"));
        assert!(!valid_store_name("-bad"));
        assert!(!valid_store_name(""));
    }

    #[test]
    fn ports() {
        assert_eq!(port_base(1), 34010);
        assert_eq!(port_base(2), 34020);
    }
}
