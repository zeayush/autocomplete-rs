//! Durable storage layer built on RocksDB.
//!
//! Key scheme:
//!   `<tenant_id>\0<term>` → `frequency` (u64 little-endian)
//!
//! We use a single column family and a NUL byte separator because tenant
//! IDs are validated to not contain NUL (see [`crate::engine`]).
//!
//! HINT: for very high write volume, split into per-tenant column
//! families. Only bother if you see compaction stalls in benchmarks.

mod rocks;
mod writer;

pub use rocks::Storage;
pub use writer::{WriteOp, WriterHandle};
