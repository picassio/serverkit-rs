//! sk-models — typed access to the ServerKit schema (124-table SQLite baseline
//! ported from Alembic revision `047_agent_footprint_dirs`).
//!
//! JSON serializers here must match the Flask models' `to_dict()` output
//! byte-for-byte semantics — the React frontend depends on those shapes.

pub mod permissions;
pub mod settings;
pub mod user;
