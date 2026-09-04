//! Top-K selection primitives.
//!
//! Extracted from the trie so it can be tested in isolation and
//! reused by fuzzy search.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

/// A bounded min-heap that keeps the top-K largest scores it has seen.
///
/// `BinaryHeap` is a max-heap, so items are wrapped in `Reverse` to make it a
/// min-heap — the smallest score sits at the top and is evicted in O(log k)
/// when a bigger score arrives.
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
        if self.k == 0 {
            return false;
        }
        if self.heap.len() < self.k {
            self.heap.push(Reverse((score, item)));
            return true;
        }

        // Compare against the current worst on (score, item) so that equal
        // scores fall back to the item's own ordering — without that, which
        // of several tied candidates survives would depend on arrival order.
        let beats_worst = match self.heap.peek() {
            Some(Reverse((worst_score, worst_item))) => (score, &item) > (*worst_score, worst_item),
            None => true,
        };
        if beats_worst {
            self.heap.pop();
            self.heap.push(Reverse((score, item)));
            true
        } else {
            false
        }
    }

    /// Peek at the *current* cutoff — the lowest score still in the heap.
    ///
    /// A caller may prune any subtree whose best possible score is *strictly*
    /// below this. Equal is not enough: `offer` breaks score ties on the item,
    /// so a candidate scoring exactly the cutoff can still displace the
    /// incumbent, and pruning it would make results depend on traversal order.
    ///
    /// Only meaningful once the heap is full; until then every candidate is
    /// accepted and no pruning is sound.
    pub fn cutoff(&self) -> Option<u64> {
        if self.k > 0 && self.heap.len() >= self.k {
            self.heap.peek().map(|Reverse((score, _))| *score)
        } else {
            None
        }
    }

    pub fn len(&self) -> usize {
        self.heap.len()
    }

    pub fn is_empty(&self) -> bool {
        self.heap.is_empty()
    }

    /// Drain into a Vec ordered by score DESC.
    pub fn into_sorted_vec(self) -> Vec<(u64, T)> {
        // Draining a `Reverse`-wrapped heap yields ascending order, so collect
        // and reverse rather than sorting again.
        let mut out: Vec<(u64, T)> = Vec::with_capacity(self.heap.len());
        let mut heap = self.heap;
        while let Some(Reverse(item)) = heap.pop() {
            out.push(item);
        }
        out.reverse();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_only_the_largest_k() {
        let mut top = TopK::new(3);
        for (score, name) in [(1u64, "a"), (9, "b"), (5, "c"), (7, "d"), (2, "e")] {
            top.offer(score, name);
        }
        let out = top.into_sorted_vec();
        assert_eq!(out, vec![(9, "b"), (7, "d"), (5, "c")]);
    }

    #[test]
    fn cutoff_is_none_until_full() {
        let mut top = TopK::new(2);
        assert_eq!(top.cutoff(), None);
        top.offer(4, "a");
        assert_eq!(top.cutoff(), None);
        top.offer(6, "b");
        assert_eq!(top.cutoff(), Some(4));
        top.offer(9, "c");
        assert_eq!(top.cutoff(), Some(6));
    }

    #[test]
    fn zero_k_accepts_nothing() {
        let mut top: TopK<&str> = TopK::new(0);
        assert!(!top.offer(100, "a"));
        assert!(top.into_sorted_vec().is_empty());
    }

    #[test]
    fn ties_break_on_the_item() {
        let mut top = TopK::new(2);
        top.offer(5, "a");
        top.offer(5, "b");
        // "a" < "b", so an equal-scoring "c" displaces "a".
        assert!(top.offer(5, "c"));
        let out = top.into_sorted_vec();
        assert_eq!(out, vec![(5, "c"), (5, "b")]);
    }
}
