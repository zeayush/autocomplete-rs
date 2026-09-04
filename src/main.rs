//! Binary entrypoint: boots tokio, initializes tracing, loads config,
//! wires the engine + HTTP router, and blocks on the axum server.

use autocomplete_rs::{api, engine::Config, Engine, Result};
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,autocomplete_rs=debug")),
        )
        .init();

    let config = Config::from_env()?;
    tracing::info!(?config, "starting autocomplete-rs");

    let engine = Arc::new(Engine::open(&config)?);
    let app = api::router(engine);

    let listener = tokio::net::TcpListener::bind(config.bind_addr).await?;
    tracing::info!(addr = %listener.local_addr()?, "listening");
    axum::serve(listener, app).await?;

    Ok(())
}
