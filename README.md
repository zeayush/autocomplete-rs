# autocomplete-rs

Fast search autocomplete engine in Rust. Compressed trie, top-K by frequency,
Levenshtein typo tolerance up to 2 edits, multi-tenant, persistent via RocksDB,
exposed over HTTP with axum.

## Status

Scaffolding only — implementations are stubbed with `todo!()` and hints.

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

## Performance target

p99 < 5ms at 10K concurrent prefix queries on a warm trie.

## Build

```
cargo build --release
cargo test
cargo bench
```
