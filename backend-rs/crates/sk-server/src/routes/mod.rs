pub mod ai;
pub mod apps;
pub mod auth;
pub mod backups;
pub mod cloud;
pub mod cloudflare;
pub mod compat;
pub mod db;
pub mod deploy;
pub mod dns;
pub mod docker;
pub mod email;
pub mod events;
pub mod files;
pub mod fleet;
pub mod ftp;
pub mod git;
pub mod gpu;
pub mod jobs;
pub mod magento;
pub mod mobile;
pub mod monitoring;
pub mod ops;
pub mod performance;
pub mod plugins;
pub mod projects;
pub mod runtimes;
pub mod security;
pub mod servers;
pub mod sso;
pub mod status;
pub mod stubs;
pub mod system;
pub mod templates;
pub mod web;
pub mod wordpress;
pub mod workflows;

use axum::Json;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "backend": "rust" }))
}
