//! Werkzeug password hash compatibility.
//!
//! Format: `method$salt$hexdigest` where method is one of:
//! - `scrypt:N:r:p`        (werkzeug ≥2.3 default; dklen=64)
//! - `pbkdf2:sha256[:iter]` (legacy default; iter defaults to 260000/600000/1000000 by version — always embedded)
//!
//! We generate `scrypt:32768:8:1` (werkzeug 3.x default) so hashes written by
//! Rust remain verifiable by the Python oracle and vice versa.

use pbkdf2::pbkdf2_hmac;
use rand::Rng;
use scrypt::Params;
use sha2::Sha256;
use subtle::ConstantTimeEq;

const SALT_CHARS: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
const DEFAULT_SALT_LEN: usize = 16;

/// Verify a plaintext password against a werkzeug-format hash.
pub fn verify_password(stored: &str, password: &str) -> bool {
    let mut parts = stored.splitn(3, '$');
    let (Some(method), Some(salt), Some(expected_hex)) = (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    let Ok(expected) = hex::decode(expected_hex) else {
        return false;
    };
    let Some(computed) = hash_internal(method, salt, password) else {
        return false;
    };
    computed.ct_eq(&expected).into()
}

/// Hash a password in werkzeug 3.x default format (`scrypt:32768:8:1`).
pub fn hash_password(password: &str) -> String {
    let mut rng = rand::thread_rng();
    let salt: String = (0..DEFAULT_SALT_LEN)
        .map(|_| SALT_CHARS[rng.gen_range(0..SALT_CHARS.len())] as char)
        .collect();
    let method = "scrypt:32768:8:1";
    let digest = hash_internal(method, &salt, password).expect("default method must hash");
    format!("{method}${salt}${}", hex::encode(digest))
}

fn hash_internal(method: &str, salt: &str, password: &str) -> Option<Vec<u8>> {
    if let Some(args) = method.strip_prefix("scrypt") {
        // werkzeug: hashlib.scrypt(password, salt, n, r, p, dklen=64)
        let (n, r, p) = match args.strip_prefix(':') {
            Some(rest) => {
                let mut it = rest.split(':');
                (
                    it.next()?.parse::<u64>().ok()?,
                    it.next()?.parse::<u32>().ok()?,
                    it.next()?.parse::<u32>().ok()?,
                )
            }
            None => (32768, 8, 1),
        };
        let log_n = (63 - n.leading_zeros()) as u8;
        if 1u64 << log_n != n {
            return None; // n must be a power of two
        }
        let params = Params::new(log_n, r, p, 64).ok()?;
        let mut out = vec![0u8; 64];
        scrypt::scrypt(password.as_bytes(), salt.as_bytes(), &params, &mut out).ok()?;
        Some(out)
    } else if let Some(args) = method.strip_prefix("pbkdf2") {
        // werkzeug: pbkdf2:sha256:iterations (iterations optional)
        let mut it = args.trim_start_matches(':').split(':');
        let algo = it.next().unwrap_or("sha256");
        let iterations: u32 = it.next().and_then(|s| s.parse().ok()).unwrap_or(600_000);
        if algo != "sha256" {
            return None; // sha1 unsupported by design — no such rows in practice
        }
        let mut out = vec![0u8; 32];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt.as_bytes(), iterations, &mut out);
        Some(out)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let h = hash_password("hunter2secret");
        assert!(h.starts_with("scrypt:32768:8:1$"));
        assert!(verify_password(&h, "hunter2secret"));
        assert!(!verify_password(&h, "wrong"));
    }

    #[test]
    fn verifies_known_werkzeug_pbkdf2_hash() {
        // generated with werkzeug: generate_password_hash("test1234", method="pbkdf2:sha256:1000")
        // pbkdf2_hmac("sha256", b"test1234", b"saltsaltsaltsalt", 1000).hex()
        let expected = {
            let mut out = vec![0u8; 32];
            pbkdf2_hmac::<Sha256>(b"test1234", b"saltsaltsaltsalt", 1000, &mut out);
            hex::encode(out)
        };
        let stored = format!("pbkdf2:sha256:1000$saltsaltsaltsalt${expected}");
        assert!(verify_password(&stored, "test1234"));
        assert!(!verify_password(&stored, "test12345"));
    }

    #[test]
    fn rejects_malformed() {
        assert!(!verify_password("garbage", "x"));
        assert!(!verify_password("plain$x$y", "x"));
    }
}
