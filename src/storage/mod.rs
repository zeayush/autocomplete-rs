//! Durable storage layer built on RocksDB.
//!
//! Key scheme:
//!   `<tenant_id>\0<term>` → `frequency` (u64 little-endian)
//!
//! We use a single column family and a NUL byte separator because tenant
//! IDs are validated to not contain NUL (see [`crate::engine`]).
//!
//! For very high write volume this would split into per-tenant column
//! families — worth doing only once benchmarks show compaction stalls.

mod rocks;
mod writer;

pub use rocks::Storage;
pub use writer::{WriteOp, WriterConfig, WriterHandle};
