//! Legacy stub router retired by the Rust route-completion roadmap.
//!
//! This module intentionally exports an empty router so old merge wiring remains
//! harmless while completed families are owned by first-class Rust route modules.

use crate::state::SharedState;
use axum::Router;

pub fn router() -> Router<SharedState> {
    Router::new()
}
