//! Crate-wide error type. One enum keeps callers from juggling
//! `Box<dyn Error>` everywhere.
//!
//! The `#[from]` conversions are what let `?` compose across the storage,
//! io and serde boundaries without hand-written `map_err` at every call.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("storage: {0}")]
    Storage(#[from] rocksdb::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("serde: {0}")]
    Serde(#[from] serde_json::Error),

    #[error("term not found: {0}")]
    NotFound(String),

    #[error("invalid input: {0}")]
    Invalid(String),

    #[error("tenant not initialized: {0}")]
    UnknownTenant(String),

    /// The writer channel is saturated — the caller should retry later.
    #[error("write queue full")]
    Backpressure,
}

pub type Result<T> = std::result::Result<T, Error>;

impl Error {
    pub fn status(&self) -> StatusCode {
        match self {
            Error::Invalid(_) => StatusCode::BAD_REQUEST,
            Error::NotFound(_) | Error::UnknownTenant(_) => StatusCode::NOT_FOUND,
            Error::Backpressure => StatusCode::SERVICE_UNAVAILABLE,
            Error::Storage(_) | Error::Io(_) | Error::Serde(_) => {
                StatusCode::INTERNAL_SERVER_ERROR
            }
        }
    }
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = serde_json::json!({ "error": self.to_string() });
        (status, axum::Json(body)).into_response()
    }
}
