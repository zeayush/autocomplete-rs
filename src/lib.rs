//! autocomplete-rs — a fast prefix-search engine.
//!
//! Public surface is intentionally thin: callers usually go through
//! [`engine::Engine`] (in-process) or the HTTP API in [`api`].
//!
//! This file stays re-exports only; the logic lives in the submodules.

pub mod api;
pub mod engine;
pub mod error;
pub mod scoring;
pub mod storage;
pub mod trie;
pub mod typo;

pub use engine::Engine;
pub use error::{Error, Result};
