# autocomplete-rs

![Rust](https://img.shields.io/badge/Rust-1.75+-orange?logo=rust)
![License](https://img.shields.io/badge/license-MIT-green)

Search autocomplete engine in Rust — compressed radix trie, top-K by
frequency, Levenshtein typo tolerance up to 2 edits, multi-tenant namespaces,
RocksDB persistence behind a write-behind batcher, served over axum.

Part of a distributed systems portfolio implementing every system from **Alex
Xu's System Design Interview (Vol. 1 & 2)**. This covers **Chapter 13 —
Design a Search Autocomplete System**.

---

## What It Provides

- **Compressed radix trie**: edges hold byte slices, split on insert and re-merged on remove
- **Top-K by frequency**: bounded min-heap, `max_subtree_freq` cached per node to prune the walk
- **Typo tolerance**: Levenshtein budget of 1 or 2 edits, one DP row carried down the trie
- **Multi-tenant**: one independent trie per tenant, namespaced in a single RocksDB keyspace
- **Durable but non-blocking**: writes are queryable immediately, persisted by a batching background task
- **Restart recovery**: boot replays the whole keyspace back into per-tenant tries
- **HTTP API**: axum router with index / query / delete / health, request timeout, JSON errors

---

## How It Works

### Radix Trie

Each edge holds a byte slice rather than a single character, so a chain of
single-child nodes collapses into one edge. Every node caches
`max_subtree_freq` — the highest frequency anywhere below it.

```
                     (root)
                       │
                      "ca"                  edges hold byte slices,
                       │                    not single characters
             ┌─────────┴─────────┐
            "r"               "scade"
             │                   │
      ┌──────┴──────┐            ●  cascade    91
     "d"          "go"
      │             │
      ●             ●
    card 1204     cargo 340

  max_subtree_freq :  root 1204 · "ca" 1204 · "r" 1204 · "scade" 91
```

- **Insert:** walk while edges match; on a partial match, split the edge at the
  common prefix and re-parent the tail
- **Remove:** clear the terminal flag, then merge any node left with exactly one
  child — so no non-terminal node ever keeps a single child
- **Search:** walk to the prefix node, then a bounded min-heap over the subtree

### Top-K by Frequency

`scoring::TopK` is a min-heap capped at `k`. The smallest score sits at the
root, so the cutoff is O(1) to read and any subtree whose `max_subtree_freq`
is below it is skipped without being visited at all.

```
  heap (k = 3)                    a child with max_subtree_freq 91
  ┌─────────────────────────┐     91 < 340 (the cutoff)
  │ 340 · 610 · 1204        │     →  that subtree is never entered
  └─────────────────────────┘
    ▲ cutoff = heap min
```

Ties break lexicographically ascending, so results are stable across runs
despite the `HashMap` child iteration order.

### Typo Tolerance

One Levenshtein DP row is carried down the trie and extended one byte per edge
character (Hanov's method). A subtree is abandoned as soon as the row minimum
exceeds the budget, since every deeper row is pointwise no smaller.

```
  query = "carrd", budget = 1

  edge "ca"      row min 0   →  descend
    edge "r"     row min 0   →  descend
      edge "d"   row min 1   →  descend  →  ● "card", distance 1  ✓ hit
    edge "scade" row min 3   →  prune the whole subtree
```

The walk is byte-oriented, so for multi-byte UTF-8 one character substitution
can cost more than one edit.

### Write Path

Writes hit the in-memory trie synchronously and are queued for durability, so
`202 Accepted` comes back before the bytes reach disk.

```
POST /v1/{tenant}/index
        │
        ▼
  Engine::index ──► RadixTrie (in memory)   ← queryable immediately
        │                                     202 Accepted returned here
        ▼
  mpsc channel (capacity 8192)              ← full ⇒ 503, caller retries
        │
        ▼
  writer task — coalesce up to 1000 ops or 50 ms
        │
        ▼
  RocksDB WriteBatch
        key = tenant ‖ 0x00 ‖ term  →  value = frequency (LE u64)
```

Tenant IDs are validated to exclude the `0x00` separator, so the split back
into `(tenant, term)` is unambiguous and terms may contain anything else.
`Engine::open` scans the full keyspace at boot and replays each key into its
tenant's trie. `Engine::shutdown` drains the queue and releases the RocksDB
directory lock before the same path can be reopened.

---

## Quick Start

```sh
git clone https://github.com/zeayush/autocomplete-rs
cd autocomplete-rs
cargo run --release
# listening on 0.0.0.0:8080, data in ./data
```

### Try the API

```sh
# Index a few terms — weight defaults to 1, repeat calls accumulate it
for t in '{"term":"card","weight":1204}' \
         '{"term":"cargo","weight":340}' \
         '{"term":"cascade","weight":91}'; do
  curl -X POST localhost:8080/v1/acme/index \
    -H 'content-type: application/json' -d "$t"
done
# HTTP/1.1 202 Accepted  ×3 — the trie is already updated, disk lags by <=50ms

# Prefix query, top-K by frequency
curl 'localhost:8080/v1/acme/query?prefix=ca&k=3'
# [{"term":"card","score":1204},{"term":"cargo","score":340},{"term":"cascade","score":91}]

# Same query with a 1-edit typo budget — hits carry their distance
curl 'localhost:8080/v1/acme/query?prefix=carrd&k=3&typo=1'
# [{"term":"card","score":1204,"distance":1}]

# Delete a term
curl -X DELETE localhost:8080/v1/acme/term/cascade
# HTTP/1.1 204 No Content

# Health — per-tenant trie shape
curl localhost:8080/health
# {"status":"ok","tenants":[{"tenant":"acme","nodes":5,"terms":3,"bytes":11}]}
```

### Use It In-Process

```rust
use autocomplete_rs::{engine::Config, Engine, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let engine = Engine::open(&Config::default())?;

    engine.index("acme", "cascade", 91).await?;

    let hits = engine.query("acme", "ca", 10);              // Vec<(String, u64)>
    let fuzzy = engine.query_fuzzy("acme", "carrd", 1, 10); // Vec<(String, u64, u32)>
    println!("{hits:?} {fuzzy:?}");

    engine.shutdown().await?; // drain the queue before reopening the same path
    Ok(())
}
```

---

## API

### HTTP

| Method | Path | Body / Query | Success |
|---|---|---|---|
| `POST` | `/v1/{tenant}/index` | `{"term": "...", "weight": 1}` | `202 Accepted` |
| `GET` | `/v1/{tenant}/query` | `?prefix=…&k=10&typo=0` | `200` + `[{term, score, distance?}]` |
| `DELETE` | `/v1/{tenant}/term/{term}` | — | `204 No Content` |
| `GET` | `/health` | — | `200` + per-tenant trie stats |

Query parameters: `k` defaults to `10` and is clamped to `100`; `typo` defaults
to `0` and is rejected above `2`. `distance` is present only when `typo > 0`.
Unknown tenants read as empty rather than as an error — a fresh tenant simply
has nothing to suggest.

Errors are JSON (`{"error": "..."}`) with the status derived from the cause:

| Condition | Status |
|---|---|
| Empty term, bad tenant, `typo > 2` | `400 Bad Request` |
| Delete of a term that is not indexed | `404 Not Found` |
| Write queue saturated | `503 Service Unavailable` |
| RocksDB / IO / serde failure | `500 Internal Server Error` |

Requests are dropped at a 2 s timeout — anything that slow has already missed
the latency budget by orders of magnitude.

### Library

```rust
pub struct Config {
    pub data_dir: PathBuf,        // AUTOCOMPLETE_DATA_DIR
    pub bind_addr: SocketAddr,    // AUTOCOMPLETE_BIND_ADDR
    pub flush_interval: Duration, // AUTOCOMPLETE_FLUSH_INTERVAL_MS
    pub max_batch: usize,         // AUTOCOMPLETE_MAX_BATCH
    pub channel_capacity: usize,  // AUTOCOMPLETE_CHANNEL_CAPACITY
}
impl Config {
    pub fn from_env() -> Result<Self>;
}

impl Engine {
    pub fn open(config: &Config) -> Result<Self>;
    pub async fn index(&self, tenant: &str, term: &str, weight: u64) -> Result<()>;
    pub fn query(&self, tenant: &str, prefix: &str, k: usize) -> Vec<(String, u64)>;
    pub fn query_fuzzy(&self, tenant: &str, query: &str, edit_budget: u32, k: usize)
        -> Vec<(String, u64, u32)>;
    pub async fn delete(&self, tenant: &str, term: &str) -> Result<()>;
    pub fn stats(&self) -> Vec<(String, TrieStats)>;
    pub fn tenant_count(&self) -> usize;
    pub async fn shutdown(self) -> Result<()>;
}

pub mod api {
    pub fn router(engine: Arc<Engine>) -> axum::Router;
}
```

---

## Configuration

Read from the environment by `Config::from_env`; every variable falls back to
the `Config::default()` value.

| Variable | Default | Description |
|---|---|---|
| `AUTOCOMPLETE_DATA_DIR` | `./data` | RocksDB directory |
| `AUTOCOMPLETE_BIND_ADDR` | `0.0.0.0:8080` | HTTP bind address |
| `AUTOCOMPLETE_FLUSH_INTERVAL_MS` | `50` | Max time a write waits before it is batched to disk |
| `AUTOCOMPLETE_MAX_BATCH` | `1000` | Max ops coalesced into one `WriteBatch` |
| `AUTOCOMPLETE_CHANNEL_CAPACITY` | `8192` | Write queue depth before requests get `503` |

---

## Key Design Decisions

**Frequency cached in the trie, not recomputed.** Every node stores
`max_subtree_freq`. Top-K is then a walk that never descends into a subtree
that cannot beat the current heap cutoff, which is what keeps a 1-byte prefix
over a 100K-term corpus at ~16 µs instead of a full subtree scan.

**Write-behind, not write-through.** The in-memory trie is the source of truth
for reads, so a write is queryable the instant it returns `202`. RocksDB sees
it within `flush_interval` (50 ms) or sooner if 1000 ops accumulate first. The
trade: a crash within that window loses the tail of the queue. Suggestions are
regenerable data, so the latency is worth more than the last 50 ms of writes.

**Backpressure surfaces as `503`, never as a stall.** When the writer channel
is full the request fails fast with `Error::Backpressure` rather than blocking
a worker — a slow disk degrades the write path without touching query latency.

**One trie per tenant, one keyspace.** Tenants get isolated `RwLock`-guarded
tries in a `DashMap`, so a write for one tenant never blocks queries for
another, while persistence stays a single RocksDB instance keyed
`tenant ‖ 0x00 ‖ term`. Tenant validation excludes the separator byte, so
decoding is unambiguous and terms stay unrestricted.

**`parking_lot::RwLock` over `tokio::sync::RwLock`.** Trie operations are
CPU-bound and microsecond-scale; an async lock would add a scheduling hop to
every query for a critical section shorter than the hop itself.

**Ties break lexicographically.** `HashMap` child iteration order is not
stable across runs, so equal-frequency results would otherwise shuffle between
identical queries. The tiebreak makes the API deterministic.

**Typo budget capped at 2.** Past 2 edits the DP row stops pruning usefully
and the walk degenerates toward a full scan — see the benchmark below, where
budget 2 already sits at 4.13 ms against a 5 ms target.

---

## Benchmarks

```sh
cargo bench
```

Measured on a 100K-term corpus with a Zipf-ish frequency skew (single core,
`--release`; your numbers will differ):

| Benchmark | Median |
|---|---|
| `insert_10k` | 4.67 ms (2.14 M terms/s) |
| `prefix_query` / 1 byte | 16.1 µs |
| `prefix_query` / 2 bytes | 6.43 µs |
| `prefix_query` / 3 bytes | 731 ns |
| `prefix_query` / 5 bytes | 270 ns |
| `fuzzy_query` / budget 1 | 326 µs |
| `fuzzy_query` / budget 2 | 4.13 ms |

Target: **p99 < 5 ms** at 10K concurrent prefix queries on a warm trie.

Exact prefix search clears that budget by two to four orders of magnitude, and
budget-1 typo tolerance by more than 10×. Budget 2 at 4.13 ms sits just under
the line with nothing to spare — that is the case where the Levenshtein
automaton (Schulz & Mihov) would start to pay for itself, and the reason the
HTTP layer caps `typo` at 2.

---

## Tests

```sh
cargo test
```

**39 unit tests + 12 integration tests.**

Unit tests cover the trie invariants (edge split on insert, merge on remove, no
non-terminal node left with a single child), the bounded top-K heap and its
lexicographic tiebreak, the Levenshtein row transitions, and the RocksDB key
encoding round-trip — including terms that themselves contain the separator
byte.

The integration suite drives the `Engine` end to end: index then query,
frequency ordering, repeated indexing bumping frequency, tenant isolation,
typo hits within budget and misses beyond it, delete, tenant/term validation,
concurrent readers and writers, write visibility within 100 ms, and restart
recovery — including that replay does not double-count frequencies.

---

## Project Structure

```
autocomplete-rs/
├── src/
│   ├── lib.rs                 # Re-exports — Engine, Error, Result
│   ├── main.rs                # Binary entrypoint: tracing, config, server
│   ├── error.rs               # Crate-wide Error/Result + HTTP status mapping
│   ├── engine.rs              # Config + multi-tenant Engine — the object callers use
│   ├── scoring.rs             # Bounded top-K min-heap
│   ├── typo.rs                # Levenshtein DP row transitions
│   ├── trie/
│   │   ├── mod.rs             # Module glue + invariants doc
│   │   ├── node.rs            # RadixNode + edge-split helpers
│   │   └── radix_trie.rs      # insert / remove / prefix_search / fuzzy_search
│   ├── storage/
│   │   ├── mod.rs
│   │   ├── rocks.rs           # RocksDB wrapper + key encoding
│   │   └── writer.rs          # Batched async writer task
│   └── api/
│       ├── mod.rs
│       ├── router.rs          # axum route wiring, trace + timeout layers
│       └── handlers.rs        # HTTP → Engine translation
├── benches/
│   └── trie_bench.rs          # Criterion benchmarks
├── tests/
│   └── integration.rs         # End-to-end tests
├── readinglist.txt            # File-by-file reading list for this chapter
└── Cargo.toml
```

---

## References

- [Radix tree (PATRICIA) — Morrison, 1968](https://en.wikipedia.org/wiki/Radix_tree) — the edge-splitting scheme `node.rs` implements
- [Fast and Easy Levenshtein Distance Using a Trie — Steve Hanov](http://stevehanov.ca/blog/index.php?id=114) — the DP-row-over-trie method used in `typo.rs`
- [Levenshtein automaton — Schulz & Mihov, 2002](https://en.wikipedia.org/wiki/Levenshtein_automaton) — the rigorous alternative, and where budget 3+ would have to go
- [The Log-Structured Merge-Tree — O'Neil et al., 1996](https://www.cs.umb.edu/~poneil/lsmtree.pdf) — why RocksDB behaves the way it does
- [RocksDB wiki](https://github.com/facebook/rocksdb/wiki) — Basic Operations, WriteBatch, Tuning Guide
- [Actors with Tokio — Alice Ryhl](https://ryhl.io/blog/actors-with-tokio/) — the mpsc-task pattern behind `storage/writer.rs`
- [axum](https://github.com/tokio-rs/axum) — HTTP framework used for the API layer
- [rust-rocksdb](https://github.com/rust-rocksdb/rust-rocksdb) — RocksDB bindings used in this project
- [Criterion benchmarking guide](https://bheisler.github.io/criterion.rs/book/) — benchmark harness
- [System Design Interview Vol. 1, Ch. 13 — Design a Search Autocomplete System](https://www.amazon.com/System-Design-Interview-insiders-Second/dp/B08CMF2CQF)

A fuller, file-by-file reading list lives in [`readinglist.txt`](readinglist.txt).

---

## License

MIT
