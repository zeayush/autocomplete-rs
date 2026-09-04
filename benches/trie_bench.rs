//! Criterion benchmarks.
//!
//! Run with: `cargo bench`
//!
//! The corpus is built once, outside the measured closures — building the trie
//! inside the timed region would bury the query cost in setup noise. Queries
//! run against a pre-warmed trie, which is the state the p99 target
//! (< 5ms at 10K concurrent prefix queries) describes.

use autocomplete_rs::trie::RadixTrie;
use criterion::{
    black_box, criterion_group, criterion_main, BatchSize, BenchmarkId, Criterion, Throughput,
};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

const CORPUS_SIZE: usize = 100_000;
const SEED: u64 = 0xA10C_0DE5;

/// Pseudo-word corpus with a Zipf-ish frequency skew, so top-K pruning is
/// exercised the way a real query log would exercise it. Deterministic, so
/// runs are comparable across commits.
fn corpus(size: usize) -> Vec<(String, u64)> {
    let mut rng = StdRng::seed_from_u64(SEED);
    let mut words = Vec::with_capacity(size);
    let mut seen = std::collections::HashSet::with_capacity(size);

    while words.len() < size {
        let len = rng.random_range(3..=12);
        let word: String = (0..len)
            .map(|_| (b'a' + rng.random_range(0..26u8)) as char)
            .collect();
        if seen.insert(word.clone()) {
            // Frequencies span four orders of magnitude; most terms are rare.
            let rank = words.len() as u64 + 1;
            let frequency = (CORPUS_SIZE as u64 * 10) / rank;
            words.push((word, frequency));
        }
    }
    words
}

fn warm_trie(words: &[(String, u64)]) -> RadixTrie {
    let mut trie = RadixTrie::new();
    for (word, frequency) in words {
        trie.insert(word, *frequency);
    }
    trie
}

fn bench_insert_10k(c: &mut Criterion) {
    let words = corpus(10_000);

    let mut group = c.benchmark_group("insert");
    group.throughput(Throughput::Elements(words.len() as u64));
    group.bench_function("insert_10k", |b| {
        b.iter_batched(
            || words.clone(),
            |words| {
                let mut trie = RadixTrie::new();
                for (word, frequency) in &words {
                    trie.insert(word, *frequency);
                }
                black_box(trie.len())
            },
            BatchSize::SmallInput,
        );
    });
    group.finish();
}

fn bench_prefix_query(c: &mut Criterion) {
    let words = corpus(CORPUS_SIZE);
    let trie = warm_trie(&words);

    let mut group = c.benchmark_group("prefix_query");
    // Prefix length drives fan-out: a 1-byte prefix reaches ~1/26th of the
    // corpus, a 5-byte prefix reaches a handful of terms.
    for prefix_len in [1usize, 2, 3, 5] {
        let prefixes: Vec<String> = words
            .iter()
            .take(64)
            .filter(|(w, _)| w.len() >= prefix_len)
            .map(|(w, _)| w[..prefix_len].to_string())
            .collect();

        group.bench_with_input(
            BenchmarkId::from_parameter(prefix_len),
            &prefixes,
            |b, prefixes| {
                let mut i = 0usize;
                b.iter(|| {
                    let prefix = &prefixes[i % prefixes.len()];
                    i += 1;
                    black_box(trie.prefix_search(black_box(prefix), 10))
                });
            },
        );
    }
    group.finish();
}

fn bench_fuzzy_query(c: &mut Criterion) {
    let words = corpus(CORPUS_SIZE);
    let trie = warm_trie(&words);

    // Queries drawn from the corpus with one byte corrupted, so every query
    // has at least one real match to find.
    let mut rng = StdRng::seed_from_u64(SEED ^ 0xFFFF);
    let queries: Vec<String> = words
        .iter()
        .take(64)
        .map(|(w, _)| {
            let mut bytes = w.clone().into_bytes();
            let at = rng.random_range(0..bytes.len());
            bytes[at] = b'a' + rng.random_range(0..26u8);
            String::from_utf8(bytes).expect("ascii stays ascii")
        })
        .collect();

    let mut group = c.benchmark_group("fuzzy_query");
    // Fuzzy is the expensive case — this is where the decision to invest in a
    // Levenshtein automaton (Schulz & Mihov) would be made.
    for budget in [1u32, 2] {
        group.bench_with_input(BenchmarkId::from_parameter(budget), &budget, |b, &budget| {
            let mut i = 0usize;
            b.iter(|| {
                let query = &queries[i % queries.len()];
                i += 1;
                black_box(trie.fuzzy_search(black_box(query), budget, 10))
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_insert_10k, bench_prefix_query, bench_fuzzy_query);
criterion_main!(benches);
