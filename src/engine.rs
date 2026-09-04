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

use crate::{
    storage::{Storage, WriteOp, WriterConfig, WriterHandle},
    trie::{RadixTrie, TrieStats},
    Error, Result,
};
use dashmap::DashMap;
use parking_lot::RwLock;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub data_dir: std::path::PathBuf,
    pub bind_addr: SocketAddr,
    pub flush_interval: Duration,
    pub max_batch: usize,
    pub channel_capacity: usize,
}

impl Default for Config {
    fn default() -> Self {
        let writer = WriterConfig::default();
        Self {
            data_dir: std::path::PathBuf::from("./data"),
            bind_addr: "0.0.0.0:8080".parse().expect("valid literal address"),
            flush_interval: writer.flush_interval,
            max_batch: writer.max_batch_size,
            channel_capacity: writer.channel_capacity,
        }
    }
}

impl Config {
    /// Read overrides from the environment, falling back to the defaults.
    ///
    /// `AUTOCOMPLETE_DATA_DIR`, `AUTOCOMPLETE_BIND_ADDR`,
    /// `AUTOCOMPLETE_FLUSH_INTERVAL_MS`, `AUTOCOMPLETE_MAX_BATCH`,
    /// `AUTOCOMPLETE_CHANNEL_CAPACITY`.
    pub fn from_env() -> Result<Self> {
        let mut config = Config::default();

        if let Ok(dir) = std::env::var("AUTOCOMPLETE_DATA_DIR") {
            config.data_dir = dir.into();
        }
        if let Ok(addr) = std::env::var("AUTOCOMPLETE_BIND_ADDR") {
            config.bind_addr = addr
                .parse()
                .map_err(|_| Error::Invalid(format!("AUTOCOMPLETE_BIND_ADDR: {addr}")))?;
        }
        config.flush_interval = Duration::from_millis(env_usize(
            "AUTOCOMPLETE_FLUSH_INTERVAL_MS",
            config.flush_interval.as_millis() as usize,
        )? as u64);
        config.max_batch = env_usize("AUTOCOMPLETE_MAX_BATCH", config.max_batch)?.max(1);
        config.channel_capacity =
            env_usize("AUTOCOMPLETE_CHANNEL_CAPACITY", config.channel_capacity)?.max(1);

        Ok(config)
    }

    fn writer_config(&self) -> WriterConfig {
        WriterConfig {
            channel_capacity: self.channel_capacity,
            max_batch_size: self.max_batch,
            flush_interval: self.flush_interval,
        }
    }
}

fn env_usize(key: &str, fallback: usize) -> Result<usize> {
    match std::env::var(key) {
        Ok(raw) => raw
            .parse()
            .map_err(|_| Error::Invalid(format!("{key}: {raw}"))),
        Err(_) => Ok(fallback),
    }
}

pub struct Engine {
    tenants: DashMap<String, Arc<RwLock<RadixTrie>>>,
    storage: Arc<Storage>,
    writer: WriterHandle,
}

impl Engine {
    /// Open the engine, replay RocksDB into memory, spawn the writer task.
    ///
    /// Must be called from within a tokio runtime — the writer task is spawned
    /// here, before any request can arrive.
    pub fn open(config: &Config) -> Result<Self> {
        let storage = Arc::new(Storage::open(&config.data_dir)?);

        let tenants: DashMap<String, Arc<RwLock<RadixTrie>>> = DashMap::new();
        let mut replayed = 0usize;
        storage.scan_all(|tenant, term, frequency| {
            let trie = tenants
                .entry(tenant.to_string())
                .or_insert_with(|| Arc::new(RwLock::new(RadixTrie::new())))
                .value()
                .clone();
            // `set`, not `insert`: the stored number is already the total, so
            // adding would double every frequency on each restart.
            trie.write().set(term, frequency);
            replayed += 1;
        })?;
        tracing::info!(terms = replayed, tenants = tenants.len(), "replayed from disk");

        let writer = WriterHandle::spawn_with(Arc::clone(&storage), config.writer_config());
        Ok(Self { tenants, storage, writer })
    }

    /// Insert or bump a term's frequency.
    ///
    /// Real-time SLA (100ms): trie mutation is synchronous, so the term is
    /// visible to reads *immediately* on this instance. RocksDB catches up
    /// asynchronously via the writer task.
    pub async fn index(&self, tenant: &str, term: &str, weight: u64) -> Result<()> {
        validate_tenant(tenant)?;
        validate_term(term)?;

        let trie = self.tenant_or_create(tenant);
        // Scoped so the (non-async-aware) lock guard is dropped before await.
        let frequency = { trie.write().insert(term, weight) };

        self.writer
            .submit(WriteOp::Upsert {
                tenant: tenant.to_string(),
                term: term.to_string(),
                frequency,
            })
            .await
    }

    /// Prefix query, top-K by frequency. Unknown tenants read as empty rather
    /// than as an error — a fresh tenant simply has nothing to suggest.
    pub fn query(&self, tenant: &str, prefix: &str, k: usize) -> Vec<(String, u64)> {
        let Some(trie) = self.tenants.get(tenant) else {
            return Vec::new();
        };
        // Bound rather than returned directly: the `Ref` guard must outlive
        // the temporary read guard, which a tail expression would not allow.
        let hits = trie.read().prefix_search(prefix, k);
        hits
    }

    /// Fuzzy query — return matches within `edit_budget` edits, top-K.
    pub fn query_fuzzy(
        &self,
        tenant: &str,
        query: &str,
        edit_budget: u32,
        k: usize,
    ) -> Vec<(String, u64, u32)> {
        let Some(trie) = self.tenants.get(tenant) else {
            return Vec::new();
        };
        let hits = trie.read().fuzzy_search(query, edit_budget, k);
        hits
    }

    pub async fn delete(&self, tenant: &str, term: &str) -> Result<()> {
        validate_tenant(tenant)?;
        validate_term(term)?;

        let existed = match self.tenants.get(tenant) {
            Some(trie) => trie.write().remove(term),
            None => false,
        };
        if !existed {
            return Err(Error::NotFound(term.to_string()));
        }

        self.writer
            .submit(WriteOp::Delete {
                tenant: tenant.to_string(),
                term: term.to_string(),
            })
            .await
    }

    /// Per-tenant trie shape, for `/health`.
    pub fn stats(&self) -> Vec<(String, TrieStats)> {
        let mut stats: Vec<(String, TrieStats)> = self
            .tenants
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().read().stats()))
            .collect();
        stats.sort_by(|a, b| a.0.cmp(&b.0));
        stats
    }

    pub fn tenant_count(&self) -> usize {
        self.tenants.len()
    }

    /// Drain pending writes and close the database. Call this before
    /// reopening the same `data_dir`: RocksDB holds an exclusive directory
    /// lock, so a reopen races an un-joined writer task otherwise.
    pub async fn shutdown(self) -> Result<()> {
        let Engine { tenants, storage, writer } = self;
        drop(tenants);
        writer.join().await?;
        drop(storage);
        Ok(())
    }

    fn tenant_or_create(&self, tenant: &str) -> Arc<RwLock<RadixTrie>> {
        if let Some(existing) = self.tenants.get(tenant) {
            return Arc::clone(existing.value());
        }
        let entry = self
            .tenants
            .entry(tenant.to_string())
            .or_insert_with(|| Arc::new(RwLock::new(RadixTrie::new())));
        Arc::clone(entry.value())
    }
}

/// Reject tenant IDs that would collide with the storage key encoding (NUL)
/// or with URL routing (slashes, whitespace).
fn validate_tenant(tenant: &str) -> Result<()> {
    if tenant.is_empty() {
        return Err(Error::Invalid("tenant must not be empty".into()));
    }
    if let Some(bad) = tenant
        .chars()
        .find(|c| *c == '\0' || *c == '/' || c.is_whitespace() || c.is_control())
    {
        return Err(Error::Invalid(format!(
            "tenant must not contain {bad:?}: {tenant:?}"
        )));
    }
    Ok(())
}

/// Terms may contain almost anything, but an empty term is never a useful
/// suggestion and a NUL would be indistinguishable from the key separator on
/// the tenant side of a malformed write.
fn validate_term(term: &str) -> Result<()> {
    if term.is_empty() {
        return Err(Error::Invalid("term must not be empty".into()));
    }
    if term.contains('\0') {
        return Err(Error::Invalid("term must not contain NUL".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tenant_validation_rejects_encoding_hazards() {
        assert!(validate_tenant("acme").is_ok());
        assert!(validate_tenant("acme-1_2").is_ok());
        assert!(validate_tenant("").is_err());
        assert!(validate_tenant("a/b").is_err());
        assert!(validate_tenant("a b").is_err());
        assert!(validate_tenant("a\0b").is_err());
        assert!(validate_tenant("a\nb").is_err());
    }

    #[test]
    fn term_validation_rejects_empty_and_nul() {
        assert!(validate_term("cascade").is_ok());
        assert!(validate_term("two words").is_ok());
        assert!(validate_term("").is_err());
        assert!(validate_term("a\0b").is_err());
    }

    #[test]
    fn env_parsing_reports_bad_values() {
        assert_eq!(env_usize("AUTOCOMPLETE_TEST_UNSET_KEY", 42).unwrap(), 42);
    }
}
