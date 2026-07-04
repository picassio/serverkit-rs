//! Compatibility routes for full-ServerKit UI pages whose backends were not
//! ported. Each returns a shape the page's loader accepts so the page renders
//! cleanly (no 404s). Where a page maps onto capabilities we DO have, the
//! route serves real data (Domains -> nginx vhosts, Firewall -> ufw). The rest
//! return valid empty state until the subsystem is implemented.

use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;

pub fn router() -> Router<SharedState> {
    Router::new()
        // ── Marketplace registry (remote packages) ──────────────────────
        .route(
            "/marketplace/registry",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "items": [] }))),
        )
        // ── Plugins / extensions ────────────────────────────────────────
        .route(
            "/plugins",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))),
        )
        .route(
            "/plugins/",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))),
        )
        .route(
            "/plugins/builtin",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "plugins": [] }))),
        )
        .route(
            "/plugins/updates",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "updates": [] }))),
        )
    // (servers/* live in the nested servers router to avoid a nest conflict)
}
