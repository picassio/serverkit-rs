//! sk-auth — Werkzeug-compatible password hashing and
//! flask-jwt-extended-compatible JWT issuance/verification.
//!
//! Compatibility is non-negotiable here: existing user rows were hashed by
//! `werkzeug.security.generate_password_hash`, and the React frontend stores
//! tokens minted by flask-jwt-extended. Both must keep working unchanged.

pub mod jwt;
pub mod password;
