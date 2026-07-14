//! Binary entrypoint: boots tokio, initializes tracing, loads config,
//! wires the engine + HTTP router, and blocks on the axum server.
//!
//! HINT: keep this file small. If it grows past ~60 lines, push logic
//! into `api::server::run(config)` and just call it from here.

use autocomplete_rs::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // HINT: initialize tracing_subscriber with EnvFilter here so
    // RUST_LOG=info,autocomplete_rs=debug works.
    // tracing_subscriber::fmt().with_env_filter(...).init();

    // HINT: read config from env (bind addr, rocksdb path, batch size).
    // A small `Config` struct with `from_env()` keeps this tidy.

    // HINT: build the engine (which owns the tenant map + storage handle).
    // let engine = autocomplete_rs::Engine::open(&config)?;

    // HINT: build the axum Router from `api::router(engine)`,
    // then `axum::serve(listener, router).await?`.

    todo!("wire tracing → config → engine → axum server")
}
