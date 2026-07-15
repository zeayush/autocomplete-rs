//! Thin wrapper around `rocksdb::DB` with our key encoding baked in.

use crate::Result;
use std::path::Path;

pub struct Storage {
    // HINT: hold `Arc<DB>` if you want to share the handle across the
    // writer task and the boot-time scanner. Plain `DB` also works if
    // your writer owns it and boot happens before the writer starts.
    // db: std::sync::Arc<rocksdb::DB>,
}

impl Storage {
    /// Open (or create) the database at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        // HINT: configure `Options`:
        //   opts.create_if_missing(true);
        //   opts.set_compression_type(DBCompressionType::Lz4);
        //   opts.set_write_buffer_size(64 * 1024 * 1024);
        //   opts.set_max_background_jobs(4);
        let _ = path;
        todo!()
    }

    /// Encode `(tenant, term)` → key bytes.
    pub(crate) fn encode_key(tenant: &str, term: &str) -> Vec<u8> {
        // HINT: `[tenant.as_bytes(), b"\0", term.as_bytes()].concat()`.
        // Validate elsewhere that tenant contains no NUL.
        let _ = (tenant, term);
        todo!()
    }

    /// Split a stored key back into `(tenant, term)`. Returns None if malformed.
    pub(crate) fn decode_key(key: &[u8]) -> Option<(&str, &str)> {
        // HINT: find first b'\0'; std::str::from_utf8 on both halves.
        let _ = key;
        todo!()
    }

    /// Persist a single (tenant, term, frequency) record.
    pub fn put(&self, tenant: &str, term: &str, frequency: u64) -> Result<()> {
        let _ = (tenant, term, frequency);
        todo!()
    }

    /// Batched write for the async writer task.
    pub fn write_batch(&self, ops: &[(&str, &str, Option<u64>)]) -> Result<()> {
        // HINT: build a `rocksdb::WriteBatch`. `None` frequency = delete.
        // Use `db.write_opt` with `WriteOptions::disable_wal(false)`.
        let _ = ops;
        todo!()
    }

    /// Iterate every (tenant, term, frequency) record. Used at boot time
    /// to rebuild in-memory tries.
    pub fn scan_all<F: FnMut(&str, &str, u64)>(&self, mut visit: F) -> Result<()> {
        // HINT: `db.iterator(IteratorMode::Start)`; call `visit` for each row.
        // For u64 decoding: `u64::from_le_bytes(value.try_into().unwrap())`.
        let _ = &mut visit;
        todo!()
    }

    pub fn delete(&self, tenant: &str, term: &str) -> Result<()> {
        let _ = (tenant, term);
        todo!()
    }
}
