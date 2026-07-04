//! Shape-correct stubs for endpoints the dashboard calls but whose modules
//! are not yet ported. Each returns the Flask handler's *empty state* so the
//! UI renders cleanly instead of erroring on 404s.
//!
//! Every stub cites its Flask oracle. Replace with real ports per the phase
//! plan (wiki: serverkit-exploration-magento-fork).

use crate::extract::AuthUser;
use crate::state::SharedState;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{json, Value};

pub fn router() -> Router<SharedState> {
    Router::new()
        // app/api/notifications.py get_inbox
        .route(
            "/notifications/inbox",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "items": [], "unread_count": 0 }))),
        )
        // app/api/plugins.py get_contributions
        .route(
            "/plugins/contributions",
            get(async |AuthUser(_u): AuthUser| {
                Json(json!({
                    "nav": [], "routes": [], "page_titles": {},
                    "command_palette": [], "widgets": [], "layouts": [], "tabs": [],
                    "ai": { "suggested_prompts": [], "tool_renderers": [] }
                }))
            }),
        )
        // (ai/* is now served by routes::ai, backed by the pi-SDK sidecar)
        // app/api/apps.py list (empty until sk-apps lands)
        .route("/apps", get(async |AuthUser(_u): AuthUser| Json(json!([]))))
        // app/api/modules.py list_modules
        .route(
            "/modules",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "modules": [] }))),
        )
        // app/api/workspaces.py list_workspaces
        .route(
            "/workspaces/",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "workspaces": [] }))),
        )
        // app/api/gpu.py GpuService.info()
        .route(
            "/gpu/",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "available": false, "gpus": [] }))),
        )
        // serverkit-wordpress standalone status (extension not ported — we
        // target Magento instead; nav entry stays dormant)
        .route(
            "/wordpress/standalone/status",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "installed": false }))),
        )
        // app/api/admin.py activity feed
        .route(
            "/admin/activity/feed",
            get(async |AuthUser(_u): AuthUser| Json(json!({ "items": [], "total": 0 }))),
        )
}

// Silence unused warning pattern for stub extractors
#[allow(dead_code)]
fn _shape_reference() -> Value {
    json!(null)
}
