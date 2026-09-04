//! End-to-end tests that hit the engine (skip HTTP layer).
//!
//! Each test gets its own `TempDir` for RocksDB, so there is no shared state
//! and no ordering dependency between them.

use autocomplete_rs::{engine::Config, Engine};
use std::time::Duration;
use tempfile::TempDir;

/// An engine over a fresh directory, with a short flush interval so the
/// durability tests do not have to wait long for the writer to catch up.
fn engine_in(dir: &TempDir) -> Engine {
    Engine::open(&Config {
        data_dir: dir.path().into(),
        flush_interval: Duration::from_millis(10),
        ..Config::default()
    })
    .expect("engine opens over an empty directory")
}

#[tokio::test]
async fn index_then_query_returns_term() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cascade", 5).await.unwrap();

    assert_eq!(engine.query("acme", "cas", 10), vec![("cascade".to_string(), 5)]);
    assert_eq!(engine.query("acme", "", 10), vec![("cascade".to_string(), 5)]);
    assert_eq!(engine.query("acme", "cat", 10), Vec::new());

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn tenants_are_isolated() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("tenant-a", "foo", 1).await.unwrap();

    assert_eq!(engine.query("tenant-a", "f", 10), vec![("foo".to_string(), 1)]);
    assert_eq!(engine.query("tenant-b", "f", 10), Vec::new());
    assert_eq!(engine.query_fuzzy("tenant-b", "foo", 2, 10), Vec::new());

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn frequency_scoring_orders_hits() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cat", 1).await.unwrap();
    engine.index("acme", "car", 10).await.unwrap();
    engine.index("acme", "cab", 5).await.unwrap();

    let hits: Vec<String> = engine.query("acme", "ca", 3).into_iter().map(|(t, _)| t).collect();
    assert_eq!(hits, vec!["car", "cab", "cat"]);

    // k truncates from the bottom.
    let hits: Vec<String> = engine.query("acme", "ca", 2).into_iter().map(|(t, _)| t).collect();
    assert_eq!(hits, vec!["car", "cab"]);

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn repeated_indexing_bumps_frequency() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cat", 1).await.unwrap();
    engine.index("acme", "car", 5).await.unwrap();
    // Three more hits on "cat" push it past "car".
    for _ in 0..5 {
        engine.index("acme", "cat", 1).await.unwrap();
    }

    assert_eq!(
        engine.query("acme", "ca", 2),
        vec![("cat".to_string(), 6), ("car".to_string(), 5)]
    );

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn typo_within_budget_matches() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cascade", 1).await.unwrap();

    let hits = engine.query_fuzzy("acme", "cscade", 1, 10);
    assert_eq!(hits, vec![("cascade".to_string(), 1, 1)]);

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn typo_beyond_budget_misses() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cascade", 1).await.unwrap();

    assert_eq!(engine.query_fuzzy("acme", "xxxxxx", 1, 10), Vec::new());
    // Two edits away is out of reach at budget 1 but not at budget 2.
    assert_eq!(engine.query_fuzzy("acme", "cscad", 1, 10), Vec::new());
    assert_eq!(
        engine.query_fuzzy("acme", "cscad", 2, 10),
        vec![("cascade".to_string(), 1, 2)]
    );

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn deleted_terms_stop_matching() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "cat", 1).await.unwrap();
    engine.index("acme", "car", 1).await.unwrap();

    engine.delete("acme", "cat").await.unwrap();
    assert_eq!(engine.query("acme", "ca", 10), vec![("car".to_string(), 1)]);
    // Deleting again is a 404, not a silent success.
    assert!(engine.delete("acme", "cat").await.is_err());

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn invalid_tenants_and_terms_are_rejected() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    assert!(engine.index("bad/tenant", "cat", 1).await.is_err());
    assert!(engine.index("bad tenant", "cat", 1).await.is_err());
    assert!(engine.index("", "cat", 1).await.is_err());
    assert!(engine.index("acme", "", 1).await.is_err());

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn writes_are_visible_within_100ms() {
    let tmp = TempDir::new().unwrap();
    let engine = engine_in(&tmp);

    engine.index("acme", "realtime", 1).await.unwrap();
    tokio::time::sleep(Duration::from_millis(100)).await;

    assert_eq!(engine.query("acme", "real", 10), vec![("realtime".to_string(), 1)]);

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_recovers_state() {
    let tmp = TempDir::new().unwrap();

    {
        let engine = engine_in(&tmp);
        engine.index("acme", "cascade", 5).await.unwrap();
        engine.index("acme", "cascading", 2).await.unwrap();
        engine.index("other", "zebra", 1).await.unwrap();
        engine.delete("acme", "cascading").await.unwrap();
        // Drains the writer channel and closes the RocksDB directory lock.
        engine.shutdown().await.unwrap();
    }

    let engine = engine_in(&tmp);
    assert_eq!(engine.query("acme", "cas", 10), vec![("cascade".to_string(), 5)]);
    assert_eq!(engine.query("other", "z", 10), vec![("zebra".to_string(), 1)]);
    assert_eq!(engine.tenant_count(), 2);

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn restart_does_not_double_count_frequencies() {
    let tmp = TempDir::new().unwrap();

    {
        let engine = engine_in(&tmp);
        engine.index("acme", "cascade", 5).await.unwrap();
        engine.shutdown().await.unwrap();
    }

    // Replay must *set* the stored total, not add to it.
    let engine = engine_in(&tmp);
    assert_eq!(engine.query("acme", "cas", 10), vec![("cascade".to_string(), 5)]);
    // A post-restart bump still accumulates on top of the replayed value.
    engine.index("acme", "cascade", 2).await.unwrap();
    assert_eq!(engine.query("acme", "cas", 10), vec![("cascade".to_string(), 7)]);

    engine.shutdown().await.unwrap();
}

#[tokio::test]
async fn concurrent_readers_and_writers_agree() {
    use std::sync::Arc;

    let tmp = TempDir::new().unwrap();
    let engine = Arc::new(engine_in(&tmp));

    let writers: Vec<_> = (0..8)
        .map(|w| {
            let engine = Arc::clone(&engine);
            tokio::spawn(async move {
                for i in 0..100 {
                    engine
                        .index("acme", &format!("term-{w}-{i:03}"), 1)
                        .await
                        .unwrap();
                }
            })
        })
        .collect();

    let readers: Vec<_> = (0..4)
        .map(|_| {
            let engine = Arc::clone(&engine);
            tokio::spawn(async move {
                for _ in 0..200 {
                    // Must never panic or deadlock against the writers.
                    let _ = engine.query("acme", "term", 10);
                    tokio::task::yield_now().await;
                }
            })
        })
        .collect();

    for task in writers.into_iter().chain(readers) {
        task.await.unwrap();
    }

    assert_eq!(engine.query("acme", "term", 1000).len(), 800);

    Arc::into_inner(engine)
        .expect("all tasks joined")
        .shutdown()
        .await
        .unwrap();
}
