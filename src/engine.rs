//! Multi-tenant engine — the main object callers interact with.
//!
//! Owns:
//!   - a DashMap of tenant_id → per-tenant state (trie behind RwLock)
//!   - the Storage handle
//!   - the WriterHandle (background flusher)
//!
//! Threading model:
//!   - reads take `RwLock::read()` on the tenant's trie; contention is low
//!     because writes are brief (in-memory mutation + channel send).
//!   - writes take `RwLock::write()`, mutate, drop, then send to writer.
//!   - one background task drains the writer channel and batches to RocksDB.

use crate::{Result, storage::{Storage, WriterHandle}, trie::RadixTrie};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct Config {
    pub data_dir: std::path::PathBuf,
    // HINT: add bind_addr, flush_interval_ms, max_batch, channel_capacity here.
}

pub struct Engine {
    tenants: DashMap<String, Arc<RwLock<RadixTrie>>>,
    storage: Arc<Storage>,
    writer: WriterHandle,
}

impl Engine {
    /// Open the engine, replay RocksDB into memory, spawn the writer task.
    pub fn open(config: &Config) -> Result<Self> {
        // HINT sketch:
        //   let storage = Arc::new(Storage::open(&config.data_dir)?);
        //   let tenants = DashMap::new();
        //   storage.scan_all(|tenant, term, freq| {
        //       tenants.entry(tenant.to_string())
        //              .or_insert_with(|| Arc::new(RwLock::new(RadixTrie::new())))
        //              .write().insert(term, freq);
        //   })?;
        //   let writer = WriterHandle::spawn(storage.clone());
        //   Ok(Self { tenants, storage, writer })
        let _ = config;
        todo!()
    }

    /// Insert or bump a term's frequency.
    ///
    /// Real-time SLA (100ms): trie mutation is synchronous, so the term is
    /// visible to reads *immediately* on this instance. RocksDB catches up
    /// asynchronously via the writer task.
    pub async fn index(&self, tenant: &str, term: &str, weight: u64) -> Result<()> {
        // HINT:
        //   validate_tenant(tenant)?; // no NUL, non-empty
        //   let trie = self.tenants
        //       .entry(tenant.to_string())
        //       .or_insert_with(|| Arc::new(RwLock::new(RadixTrie::new())))
        //       .clone();
        //   let new_freq = { trie.write().insert(term, weight) };
        //   self.writer.submit(WriteOp::Upsert { tenant, term, frequency: new_freq }).await
        let _ = (tenant, term, weight);
        todo!()
    }

    /// Prefix query, top-K by frequency.
    pub fn query(&self, tenant: &str, prefix: &str, k: usize) -> Vec<(String, u64)> {
        // HINT:
        //   let Some(trie) = self.tenants.get(tenant) else { return vec![]; };
        //   trie.read().prefix_search(prefix, k)
        let _ = (tenant, prefix, k);
        todo!()
    }

    /// Fuzzy query — return matches within `edit_budget` edits, top-K.
    pub fn query_fuzzy(
        &self,
        tenant: &str,
        query: &str,
        edit_budget: u32,
        k: usize,
    ) -> Vec<(String, u64, u32)> {
        // HINT: same shape as `query`, delegate to `RadixTrie::fuzzy_search`.
        let _ = (tenant, query, edit_budget, k);
        todo!()
    }

    pub async fn delete(&self, tenant: &str, term: &str) -> Result<()> {
        // HINT: trie.write().remove(term); then submit WriteOp::Delete.
        let _ = (tenant, term);
        todo!()
    }
}

// HINT: helper — reject tenant IDs containing NUL, whitespace, or slashes,
// since they collide with the key encoding and URL routing respectively.
fn validate_tenant(_tenant: &str) -> Result<()> {
    todo!()
}
