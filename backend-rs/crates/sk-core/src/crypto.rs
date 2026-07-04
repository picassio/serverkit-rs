//! At-rest secret encryption, Fernet-compatible with Flask's
//! `app/utils/crypto.py` (`cryptography.fernet.Fernet`).
//!
//! Key resolution:
//! 1. `SERVERKIT_ENCRYPTION_KEY` (a Fernet key: 32 bytes url-safe base64) —
//!    set this to the same value as the Flask install and every secret it
//!    wrote stays readable here.
//! 2. else a persisted key at `$SK_DATA_DIR/encryption.key` (generated once,
//!    stable across restarts). A warning is logged — fine for dev, but
//!    production should set the env var.

use fernet::Fernet;
use std::sync::OnceLock;

static KEY: OnceLock<String> = OnceLock::new();

fn data_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(std::env::var("SK_DATA_DIR").unwrap_or_else(|_| "data".into()))
}

/// Resolve (and cache) the Fernet key string.
fn key() -> &'static str {
    KEY.get_or_init(|| {
        if let Ok(k) = std::env::var("SERVERKIT_ENCRYPTION_KEY") {
            if Fernet::new(&k).is_some() {
                return k;
            }
            tracing::error!("SERVERKIT_ENCRYPTION_KEY is not a valid Fernet key — ignoring");
        }
        // persisted dev key
        let path = data_dir().join("encryption.key");
        if let Ok(existing) = std::fs::read_to_string(&path) {
            let trimmed = existing.trim().to_string();
            if Fernet::new(&trimmed).is_some() {
                return trimmed;
            }
        }
        let generated = Fernet::generate_key();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if std::fs::write(&path, &generated).is_ok() {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
            }
            tracing::warn!(
                path = %path.display(),
                "SERVERKIT_ENCRYPTION_KEY not set — generated a persisted key (set the env var in production)"
            );
        }
        generated
    })
}

fn fernet() -> Fernet {
    // key() guarantees a valid key string
    Fernet::new(key()).expect("valid fernet key")
}

/// `encrypt_secret` — returns a Fernet token string.
pub fn encrypt(plaintext: &str) -> String {
    fernet().encrypt(plaintext.as_bytes())
}

/// `decrypt_secret` — `None` if the token is invalid/tampered/wrong-key.
pub fn decrypt(token: &str) -> Option<String> {
    fernet()
        .decrypt(token)
        .ok()
        .and_then(|b| String::from_utf8(b).ok())
}

/// Migration-friendly: decrypt a stored value, but if it isn't a valid Fernet
/// token (e.g. a plaintext row written before encryption existed), return it
/// unchanged. Lets us encrypt-on-write without a data migration step.
pub fn decrypt_or_plain(value: &str) -> String {
    decrypt(value).unwrap_or_else(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let token = encrypt("magento-db-pw");
        assert_ne!(token, "magento-db-pw");
        assert_eq!(decrypt(&token).as_deref(), Some("magento-db-pw"));
    }

    #[test]
    fn plaintext_passthrough() {
        // legacy plaintext row survives decrypt_or_plain
        assert_eq!(decrypt_or_plain("not-a-token"), "not-a-token");
    }
}
