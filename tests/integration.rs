//! End-to-end tests that hit the engine (skip HTTP layer).
//!
//! HINT: use `tempfile::TempDir` for the RocksDB path so tests are hermetic.
//! Each test gets its own dir; no shared state.

// use autocomplete_rs::{Engine, engine::Config};

#[tokio::test]
async fn index_then_query_returns_term() {
    // HINT:
    //   let tmp = tempfile::tempdir().unwrap();
    //   let engine = Engine::open(&Config { data_dir: tmp.path().into() }).unwrap();
    //   engine.index("acme", "cascade", 5).await.unwrap();
    //   let hits = engine.query("acme", "cas", 10);
    //   assert_eq!(hits, vec![("cascade".into(), 5)]);
}

#[tokio::test]
async fn tenants_are_isolated() {
    // HINT: index "foo" under tenant A. Querying tenant B for "f" must return [].
}

#[tokio::test]
async fn frequency_scoring_orders_hits() {
    // HINT: index "cat":1, "car":10, "cab":5 → query "ca" k=3 → [car, cab, cat].
}

#[tokio::test]
async fn typo_within_budget_matches() {
    // HINT: index "cascade":1; query_fuzzy("cscade", 1, 10) → contains cascade.
}

#[tokio::test]
async fn typo_beyond_budget_misses() {
    // HINT: index "cascade":1; query_fuzzy("xxxxxx", 1, 10) → empty.
}

#[tokio::test]
async fn writes_are_visible_within_100ms() {
    // HINT: index, sleep 100ms, query. Should return the term. This asserts
    // the "real-time index updates" deliverable.
}

#[tokio::test]
async fn restart_recovers_state() {
    // HINT: open engine, index, drop engine, reopen with same data_dir,
    // query → term should still be there. This exercises the RocksDB replay path.
}
