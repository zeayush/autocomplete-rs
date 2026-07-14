//! Crate-wide error type. One enum keeps callers from juggling
//! `Box<dyn Error>` everywhere.
//!
//! HINT: derive `thiserror::Error`. Wrap rocksdb, io, and serde errors
//! with `#[from]` so `?` composes cleanly.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    // HINT: variants you'll likely need:
    //   #[error("storage: {0}")] Storage(#[from] rocksdb::Error),
    //   #[error("io: {0}")]      Io(#[from] std::io::Error),
    //   #[error("term not found: {0}")] NotFound(String),
    //   #[error("invalid input: {0}")]  Invalid(String),
    //   #[error("tenant not initialized: {0}")] UnknownTenant(String),
    #[error("unimplemented")]
    Todo,
}

pub type Result<T> = std::result::Result<T, Error>;
