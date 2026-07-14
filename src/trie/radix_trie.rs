//! Public radix-trie API.
//!
//! HINT: keep this file *thin*. It's the state machine over Node.
//! Recursion is fine at this scale, but if you ever hit stack limits
//! on huge tries, rewrite the hot paths iteratively.

use super::node::Node;

pub struct RadixTrie {
    root: Box<Node>,
    // HINT: cache term count for O(1) len(). Update on insert/remove.
    // len: usize,
}

impl RadixTrie {
    pub fn new() -> Self {
        // HINT: root has empty edge_label, is not terminal.
        todo!("build a trie with an empty root")
    }

    /// Insert (or increment frequency of) `term`.
    /// Returns the *new* frequency for the term.
    pub fn insert(&mut self, term: &str, weight: u64) -> u64 {
        // HINT: walk from root, byte by byte. At each step:
        //   - if no child matches the next byte → create leaf, done
        //   - if a child matches but its edge is a prefix of remaining → descend
        //   - if remaining is a prefix of the edge → split edge, mark terminal
        //   - if they diverge mid-edge → split edge, add two children
        // Merge frequencies (add `weight`) if term already exists.
        let _ = (term, weight);
        todo!()
    }

    /// Remove `term`. Returns true if it existed.
    pub fn remove(&mut self, term: &str) -> bool {
        // HINT: mark node non-terminal, then compact:
        //   - if node has 0 children → delete it, then check if parent
        //     can now be merged with its sole remaining child
        //   - if node has 1 child → merge edges: parent.edge += child.edge
        // Compaction preserves invariant #2 (see mod.rs).
        let _ = term;
        todo!()
    }

    /// Collect up to `k` terms that begin with `prefix`, ordered by frequency desc.
    /// Empty prefix returns the top-K terms across the whole trie.
    pub fn prefix_search(&self, prefix: &str, k: usize) -> Vec<(String, u64)> {
        // HINT: two phases.
        //   1. Descend to the node representing `prefix` (may end mid-edge).
        //      If the prefix doesn't match any path, return [].
        //   2. DFS from there, maintaining a bounded min-heap of size k.
        //      See scoring::TopK — factor it out.
        // Optimization: if you cache max_subtree_freq per node, prune
        // any subtree whose max < heap.peek(). Skip this until it matters.
        let _ = (prefix, k);
        todo!()
    }

    /// Walk the trie collecting terms within `edit_budget` Levenshtein distance
    /// of `query`, capped at `k` by frequency.
    pub fn fuzzy_search(
        &self,
        query: &str,
        edit_budget: u32,
        k: usize,
    ) -> Vec<(String, u64, u32)> {
        // HINT: classic "trie + row-DP" walk.
        //   State per recursion frame: the DP row for the *current* node's
        //   accumulated label vs the full query. Children reuse the row.
        //   Prune when `row.iter().min() > edit_budget`.
        // See typo::walk for the reusable helper.
        let _ = (query, edit_budget, k);
        todo!()
    }

    /// Number of distinct terms stored.
    pub fn len(&self) -> usize {
        todo!()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Default for RadixTrie {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    // HINT: unit tests to write first (TDD-friendly order):
    //   1. empty trie: prefix_search returns []
    //   2. insert("cat") then prefix_search("c") returns [("cat", 1)]
    //   3. insert("cat"), insert("car") → edge split at "ca"
    //   4. insert("cat", 5), insert("cat", 3) → frequency merges to 8
    //   5. remove("cat") from {"cat","car"} → "car" still findable
    //   6. prefix_search top-k respects k and orders by freq desc
    //   7. proptest: insert N random strings, prefix_search("") returns them all
}
