//! Top-K selection primitives.
//!
//! Extracted from the trie so it can be tested in isolation and
//! reused by fuzzy search.

use std::collections::BinaryHeap;
use std::cmp::Reverse;

/// A bounded min-heap that keeps the top-K largest scores it has seen.
///
/// HINT: `BinaryHeap` is a max-heap. Wrap items in `Reverse` to make it a
/// min-heap — then the smallest score is at the top and can be evicted
/// in O(log k) when a bigger score arrives.
pub struct TopK<T> {
    k: usize,
    heap: BinaryHeap<Reverse<(u64, T)>>,
}

impl<T: Ord> TopK<T> {
    pub fn new(k: usize) -> Self {
        Self { k, heap: BinaryHeap::with_capacity(k + 1) }
    }

    /// Offer a candidate. Returns true if it made the cut.
    pub fn offer(&mut self, score: u64, item: T) -> bool {
        // HINT:
        //   if heap.len() < k → push, return true
        //   else if score > heap.peek().0 → pop, push, return true
        //   else return false
        let _ = (score, item);
        todo!()
    }

    /// Peek at the *current* cutoff — subtrees with max_freq below this
    /// can be pruned in prefix_search.
    pub fn cutoff(&self) -> Option<u64> {
        // HINT: only meaningful once the heap is full.
        todo!()
    }

    /// Drain into a Vec ordered by score DESC.
    pub fn into_sorted_vec(self) -> Vec<(u64, T)> {
        // HINT: pop everything, then reverse — heap gives ascending on drain
        // because we wrapped in Reverse.
        todo!()
    }
}
