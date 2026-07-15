//! axum router wiring.

use crate::Engine;
use axum::{routing::{get, post, delete}, Router};
use std::sync::Arc;

pub fn router(engine: Arc<Engine>) -> Router {
    // HINT sketch:
    //   Router::new()
    //     .route("/health", get(super::handlers::health))
    //     .route("/v1/:tenant/index",       post(super::handlers::index))
    //     .route("/v1/:tenant/query",       get(super::handlers::query))
    //     .route("/v1/:tenant/term/:term",  delete(super::handlers::delete))
    //     .with_state(engine)
    //     .layer(tower_http::trace::TraceLayer::new_for_http())
    //     .layer(tower::timeout::TimeoutLayer::new(Duration::from_secs(2)))
    let _ = engine;
    todo!()
}
