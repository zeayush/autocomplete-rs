//! Criterion benchmarks.
//!
//! Run with: `cargo bench`
//!
//! HINT: seed the trie with a realistic corpus once, then benchmark queries.
//! Building the trie inside the measured closure hides the query cost in
//! setup noise. Use `bench_with_input` or `iter_with_setup`.

use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn bench_insert_10k(_c: &mut Criterion) {
    // HINT: measure `RadixTrie::insert` over 10k unique random strings.
    // Report: throughput (elements per second) and mean latency.
    // c.bench_function("insert_10k", |b| {
    //     b.iter_batched(
    //         || (RadixTrie::new(), corpus.clone()),
    //         |(mut trie, words)| { for w in words { trie.insert(&w, 1); } },
    //         BatchSize::SmallInput,
    //     );
    // });
    let _ = black_box(0);
}

fn bench_prefix_query(_c: &mut Criterion) {
    // HINT: pre-fill trie with 100k common english words, then measure
    // prefix_search for prefixes of length 1..5. Distinguish cache-warm
    // and cache-cold runs.
}

fn bench_fuzzy_query(_c: &mut Criterion) {
    // HINT: same corpus, edit_budget=1 and =2. Report p50/p99.
    // Fuzzy is the expensive case — this is where you'd decide whether
    // to invest in the Levenshtein automaton.
}

criterion_group!(benches, bench_insert_10k, bench_prefix_query, bench_fuzzy_query);
criterion_main!(benches);
