//! Thin wrapper around `rocksdb::DB` with our key encoding baked in.

use crate::{Error, Result};
use rocksdb::{DBCompressionType, IteratorMode, Options, WriteBatch, WriteOptions, DB};
use std::path::Path;
use std::sync::Arc;

/// Separator between tenant and term in a key. Tenant IDs are validated to
/// exclude it (see [`crate::engine`]), so the split is unambiguous.
const SEP: u8 = 0;

pub struct Storage {
    /// `Arc` so the boot-time scanner and the background writer task can share
    /// one handle without reopening the database (RocksDB takes an exclusive
    /// directory lock, so a second open would fail).
    db: Arc<DB>,
}

impl Storage {
    /// Open (or create) the database at `path`.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let mut opts = Options::default();
        opts.create_if_missing(true);
        opts.set_compression_type(DBCompressionType::Lz4);
        opts.set_write_buffer_size(64 * 1024 * 1024);
        opts.set_max_background_jobs(4);

        let db = DB::open(&opts, path)?;
        Ok(Self { db: Arc::new(db) })
    }

    /// Encode `(tenant, term)` → key bytes.
    pub(crate) fn encode_key(tenant: &str, term: &str) -> Vec<u8> {
        let mut key = Vec::with_capacity(tenant.len() + 1 + term.len());
        key.extend_from_slice(tenant.as_bytes());
        key.push(SEP);
        key.extend_from_slice(term.as_bytes());
        key
    }

    /// Split a stored key back into `(tenant, term)`. Returns None if malformed.
    pub(crate) fn decode_key(key: &[u8]) -> Option<(&str, &str)> {
        let split = key.iter().position(|&b| b == SEP)?;
        let tenant = std::str::from_utf8(&key[..split]).ok()?;
        let term = std::str::from_utf8(&key[split + 1..]).ok()?;
        Some((tenant, term))
    }

    /// Persist a single (tenant, term, frequency) record.
    pub fn put(&self, tenant: &str, term: &str, frequency: u64) -> Result<()> {
        self.db
            .put(Self::encode_key(tenant, term), frequency.to_le_bytes())?;
        Ok(())
    }

    /// Batched write for the async writer task. A `None` frequency is a delete.
    pub fn write_batch(&self, ops: &[(&str, &str, Option<u64>)]) -> Result<()> {
        if ops.is_empty() {
            return Ok(());
        }

        let mut batch = WriteBatch::default();
        for (tenant, term, frequency) in ops {
            let key = Self::encode_key(tenant, term);
            match frequency {
                Some(f) => batch.put(key, f.to_le_bytes()),
                None => batch.delete(key),
            }
        }

        // Keep the WAL on: the whole point of the batch is to preserve
        // durability while moving the fsync off the request path.
        let mut write_opts = WriteOptions::default();
        write_opts.disable_wal(false);
        self.db.write_opt(batch, &write_opts)?;
        Ok(())
    }

    /// Iterate every (tenant, term, frequency) record. Used at boot time
    /// to rebuild in-memory tries.
    pub fn scan_all<F: FnMut(&str, &str, u64)>(&self, mut visit: F) -> Result<()> {
        for row in self.db.iterator(IteratorMode::Start) {
            let (key, value) = row?;
            let Some((tenant, term)) = Self::decode_key(&key) else {
                // A key we did not write. Skip rather than abort boot.
                continue;
            };
            let Some(frequency) = decode_frequency(&value) else {
                continue;
            };
            visit(tenant, term, frequency);
        }
        Ok(())
    }

    /// Read back one record's frequency.
    pub fn get(&self, tenant: &str, term: &str) -> Result<Option<u64>> {
        let raw = self.db.get(Self::encode_key(tenant, term))?;
        Ok(raw.as_deref().and_then(decode_frequency))
    }

    pub fn delete(&self, tenant: &str, term: &str) -> Result<()> {
        self.db.delete(Self::encode_key(tenant, term))?;
        Ok(())
    }

    /// Flush memtables to SST files. Called on shutdown so a restart does not
    /// have to replay the whole WAL.
    pub fn flush(&self) -> Result<()> {
        self.db.flush().map_err(Error::from)
    }
}

fn decode_frequency(value: &[u8]) -> Option<u64> {
    // Written by us as 8 little-endian bytes; anything else is corruption or
    // a foreign key, and is skipped rather than panicked on.
    value.try_into().ok().map(u64::from_le_bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip() {
        let key = Storage::encode_key("acme", "cascade");
        assert_eq!(Storage::decode_key(&key), Some(("acme", "cascade")));
    }

    #[test]
    fn keys_round_trip_with_empty_parts() {
        let key = Storage::encode_key("acme", "");
        assert_eq!(Storage::decode_key(&key), Some(("acme", "")));
    }

    #[test]
    fn terms_may_contain_the_separator_byte() {
        // Only the *first* NUL splits, so a NUL inside a term is preserved.
        let key = Storage::encode_key("acme", "a\0b");
        assert_eq!(Storage::decode_key(&key), Some(("acme", "a\0b")));
    }

    #[test]
    fn decode_rejects_a_key_without_a_separator() {
        assert_eq!(Storage::decode_key(b"nosep"), None);
    }

    #[test]
    fn frequency_decoding_rejects_wrong_widths() {
        assert_eq!(decode_frequency(&7u64.to_le_bytes()), Some(7));
        assert_eq!(decode_frequency(&[1, 2, 3]), None);
    }
}
