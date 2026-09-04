# autocomplete-rs

Fast search autocomplete engine in Rust. Compressed trie, top-K by frequency,
Levenshtein typo tolerance up to 2 edits, multi-tenant, persistent via RocksDB,
exposed over HTTP with axum.

## Status

Implemented end to end: trie, scoring, typo tolerance, persistence, the
background writer, and the HTTP layer. `cargo test` covers the data-structure
invariants, the storage key encoding, and the engine end to end (including
restart recovery).

## Layout

```
src/
  lib.rs            re-exports
  main.rs           binary entrypoint
  error.rs          crate-wide Error/Result
  engine.rs         multi-tenant Engine — the object callers use
  scoring.rs        bounded top-K min-heap
  typo.rs           Levenshtein DP row transitions
  trie/
    mod.rs          module glue + invariants doc
    node.rs         RadixNode + edge-split helpers
    radix_trie.rs   insert / remove / prefix_search / fuzzy_search
  storage/
    mod.rs
    rocks.rs        RocksDB wrapper + key encoding
    writer.rs       batched async writer task
  api/
    mod.rs
    router.rs       axum route wiring
    handlers.rs     HTTP → Engine translation
benches/
  trie_bench.rs     Criterion benchmarks
tests/
  integration.rs    end-to-end tests
```

## API

| Method | Path                         | Body / Query                  |
| ------ | ---------------------------- | ----------------------------- |
| POST   | `/v1/{tenant}/index`         | `{ "term": "...", "weight": 1 }` |
| GET    | `/v1/{tenant}/query`         | `?prefix=...&k=10&typo=0`     |
| DELETE | `/v1/{tenant}/term/{term}`   | —                             |
| GET    | `/health`                    | —                             |

## Design notes

**Trie.** Radix tree over bytes; edges are split on insert and re-merged on
remove, so no non-terminal node is ever left with a single child. Each node
caches `max_subtree_freq`, which lets top-K search skip any subtree that
cannot beat the current heap cutoff.

**Top-K.** A bounded min-heap of size `k` (`scoring::TopK`). Ties break
lexicographically ascending, so results are stable across runs despite the
`HashMap` child iteration order.

**Typo tolerance.** One Levenshtein DP row carried down the trie (Hanov's
method). A subtree is abandoned as soon as its row minimum exceeds the budget,
since every deeper row is pointwise no smaller. The walk is byte-oriented, so
for multi-byte UTF-8 one character substitution can cost more than one edit.

**Durability.** Writes hit the in-memory trie synchronously — they are
queryable immediately — and are queued to a background task that coalesces up
to 1000 ops or 50ms into one RocksDB `WriteBatch`. Boot replays the whole
keyspace back into per-tenant tries. Call `Engine::shutdown` to drain the queue
and release the RocksDB directory lock before reopening the same path.

## Configuration

Read from the environment by `Config::from_env`:

| Variable                          | Default        |
| --------------------------------- | -------------- |
| `AUTOCOMPLETE_DATA_DIR`           | `./data`       |
| `AUTOCOMPLETE_BIND_ADDR`          | `0.0.0.0:8080` |
| `AUTOCOMPLETE_FLUSH_INTERVAL_MS`  | `50`           |
| `AUTOCOMPLETE_MAX_BATCH`          | `1000`         |
| `AUTOCOMPLETE_CHANNEL_CAPACITY`   | `8192`         |

## Performance target

p99 < 5ms at 10K concurrent prefix queries on a warm trie. `cargo bench`
measures insert throughput, prefix search at prefix lengths 1–5, and fuzzy
search at edit budgets 1 and 2.

Measured on a 100K-term corpus with a Zipf-ish frequency skew (single core,
`--release`; your numbers will differ):

| Benchmark                  | Median   |
| -------------------------- | -------- |
| `insert_10k`               | 4.67 ms (2.14 M terms/s) |
| `prefix_query` / 1 byte    | 16.1 µs  |
| `prefix_query` / 2 bytes   | 6.43 µs  |
| `prefix_query` / 3 bytes   | 731 ns   |
| `prefix_query` / 5 bytes   | 270 ns   |
| `fuzzy_query` / budget 1   | 326 µs   |
| `fuzzy_query` / budget 2   | 4.13 ms  |

Exact prefix search clears the budget by two to four orders of magnitude, and
budget-1 typo tolerance by more than 10×. Budget 2 at 4.13 ms sits just under
the 5 ms line with nothing to spare — that is the case where the Levenshtein
automaton (Schulz & Mihov) would start to pay for itself, and the reason the
HTTP layer caps `typo` at 2.

## Build

```
cargo build --release
cargo test
cargo bench
```
