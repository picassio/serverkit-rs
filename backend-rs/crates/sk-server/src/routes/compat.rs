//! Legacy compatibility router retired by the Rust route-completion roadmap.
//!
//! It intentionally contains no fallback/empty-state handlers. Completed route
//! families are owned by first-class Rust modules mounted from `main.rs`.

use crate::state::SharedState;
use axum::Router;

pub fn router() -> Router<SharedState> {
    Router::new()
}
