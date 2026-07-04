//! sk-core — configuration, database pool, shared time helpers.
//!
//! Mirrors the semantics of ServerKit's Flask `config.py` so a Rust panel can
//! run against a database created by either backend.

pub mod config;
pub mod crypto;
pub mod db;
pub mod time;

pub use config::Config;
