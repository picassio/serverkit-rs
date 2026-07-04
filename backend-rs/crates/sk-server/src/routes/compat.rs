//! Compatibility routes for full-ServerKit UI pages whose backends were not
//! ported. Each returns a shape the page's loader accepts so the page renders
//! cleanly (no 404s). Where a page maps onto capabilities we DO have, the
//! route serves real data (Domains -> nginx vhosts, Firewall -> ufw). The rest
//! return valid empty state until the subsystem is implemented.

use crate::state::SharedState;
use axum::Router;

pub fn router() -> Router<SharedState> {
    Router::new()
    // (servers/* live in the nested servers router to avoid a nest conflict)
}
