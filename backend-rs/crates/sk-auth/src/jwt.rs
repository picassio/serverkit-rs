//! flask-jwt-extended compatible JWTs (HS256).
//!
//! Token claims produced by flask-jwt-extended 4.x:
//! `{ fresh, iat, jti, type: "access"|"refresh", sub, nbf, exp, ...extra }`
//! `sub` is the user id. PyJWT ≥2.10 requires `sub` to be a string, so we
//! emit string subs (matching flask-jwt-extended ≥4.7) and accept both int
//! and string on decode (older Flask tokens carried ints).

use chrono::Utc;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, thiserror::Error)]
pub enum JwtError {
    #[error("invalid token: {0}")]
    Invalid(#[from] jsonwebtoken::errors::Error),
    #[error("wrong token type: expected {expected}, got {got}")]
    WrongType { expected: String, got: String },
    #[error("token is pending 2FA verification")]
    TwoFaPending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenType {
    Access,
    Refresh,
}

impl TokenType {
    fn as_str(self) -> &'static str {
        match self {
            TokenType::Access => "access",
            TokenType::Refresh => "refresh",
        }
    }
}

/// `sub` claim — Flask emits int identities; some deployments store strings.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Subject {
    Int(i64),
    Str(String),
}

impl Subject {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Subject::Int(i) => Some(*i),
            Subject::Str(s) => s.parse().ok(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub fresh: bool,
    pub iat: i64,
    pub jti: String,
    #[serde(rename = "type")]
    pub token_type: String,
    pub sub: Subject,
    pub nbf: i64,
    pub exp: i64,
    /// Set on the temporary token issued when a 2FA-enabled user passes
    /// password auth but has not yet supplied a TOTP code.
    #[serde(rename = "2fa_pending", skip_serializing_if = "Option::is_none")]
    pub twofa_pending: Option<bool>,
}

pub fn create_token(
    user_id: i64,
    token_type: TokenType,
    ttl_secs: i64,
    secret: &str,
    twofa_pending: bool,
) -> Result<String, JwtError> {
    let now = Utc::now().timestamp();
    let claims = Claims {
        fresh: false,
        iat: now,
        jti: Uuid::new_v4().to_string(),
        token_type: token_type.as_str().to_string(),
        sub: Subject::Str(user_id.to_string()),
        nbf: now,
        exp: now + ttl_secs,
        twofa_pending: twofa_pending.then_some(true),
    };
    Ok(encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )?)
}

/// Decode and validate a token, enforcing the expected type and rejecting
/// 2FA-pending temp tokens (unless `allow_2fa_pending`).
pub fn decode_token(
    token: &str,
    secret: &str,
    expected: TokenType,
    allow_2fa_pending: bool,
) -> Result<Claims, JwtError> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_required_spec_claims(&["exp"]);
    validation.validate_nbf = true;

    let data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &validation,
    )?;
    let claims = data.claims;

    if claims.token_type != expected.as_str() {
        return Err(JwtError::WrongType {
            expected: expected.as_str().to_string(),
            got: claims.token_type,
        });
    }
    if claims.twofa_pending == Some(true) && !allow_2fa_pending {
        return Err(JwtError::TwoFaPending);
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_access() {
        let t = create_token(42, TokenType::Access, 900, "s3cret", false).unwrap();
        let c = decode_token(&t, "s3cret", TokenType::Access, false).unwrap();
        assert_eq!(c.sub.as_i64(), Some(42));
        assert_eq!(c.token_type, "access");
    }

    #[test]
    fn refresh_rejected_as_access() {
        let t = create_token(1, TokenType::Refresh, 900, "k", false).unwrap();
        assert!(decode_token(&t, "k", TokenType::Access, false).is_err());
    }

    #[test]
    fn twofa_pending_blocked_by_default() {
        let t = create_token(1, TokenType::Access, 900, "k", true).unwrap();
        assert!(decode_token(&t, "k", TokenType::Access, false).is_err());
        assert!(decode_token(&t, "k", TokenType::Access, true).is_ok());
    }

    #[test]
    fn accepts_string_sub_from_flask() {
        // flask-jwt-extended ≥4.7 emits string subs
        let now = Utc::now().timestamp();
        let claims = serde_json::json!({
            "fresh": false, "iat": now, "jti": "x", "type": "access",
            "sub": "7", "nbf": now, "exp": now + 900
        });
        let t = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(b"k"),
        )
        .unwrap();
        let c = decode_token(&t, "k", TokenType::Access, false).unwrap();
        assert_eq!(c.sub.as_i64(), Some(7));
    }
}
