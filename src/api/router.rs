//! axum router wiring.

use crate::Engine;
use axum::{
    routing::{delete, get, post},
    Router,
};
use std::sync::Arc;
use std::time::Duration;

/// Requests that outlive this have already missed the latency budget by
/// orders of magnitude; failing fast keeps them from pinning a worker.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(2);

pub fn router(engine: Arc<Engine>) -> Router {
    Router::new()
        .route("/health", get(super::handlers::health))
        .route("/v1/:tenant/index", post(super::handlers::index))
        .route("/v1/:tenant/query", get(super::handlers::query))
        .route("/v1/:tenant/term/:term", delete(super::handlers::delete))
        .with_state(engine)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::timeout::TimeoutLayer::new(REQUEST_TIMEOUT))
}
