//! HTTP surface (axum).
//!
//! Routes (see `router` for the axum tree):
//!   POST   /v1/{tenant}/index         → index a term
//!   GET    /v1/{tenant}/query         → prefix search
//!   DELETE /v1/{tenant}/term/{term}   → remove a term
//!   GET    /health                    → liveness

mod handlers;
mod router;

pub use router::router;
