pub mod ai;
pub mod auth;
pub mod db;
pub mod docker;
pub mod files;
pub mod magento;
pub mod monitoring;
pub mod ops;
pub mod servers;
pub mod stubs;
pub mod system;
pub mod templates;
pub mod web;

use axum::Json;
use serde_json::{json, Value};

pub async fn health() -> Json<Value> {
    Json(json!({ "status": "ok", "backend": "rust" }))
}
