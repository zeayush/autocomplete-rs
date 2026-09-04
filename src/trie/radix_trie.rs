//! Public radix-trie API — the state machine over `Node`.
//!
//! The walks are recursive, which is fine because depth is bounded by the
//! longest indexed term, not by the number of terms. If terms ever get long
//! enough to threaten the stack, the hot paths convert to explicit stacks
//! without changing this interface.

use super::node::Node;
use crate::scoring::TopK;
use crate::typo::{initial_row, next_row, row_min};
use std::cmp::Reverse;

/// Memory/shape summary, cheap enough to expose on `/health`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct TrieStats {
    /// Node count, excluding the root.
    pub nodes: usize,
    /// Distinct terms stored.
    pub terms: usize,
    /// Bytes of edge label held across all nodes. A lower bound on real
    /// footprint (it ignores per-node and HashMap overhead), but it is the
    /// part that scales with the corpus.
    pub bytes: usize,
}

pub struct RadixTrie {
    root: Box<Node>,
    /// Cached so `len()` stays O(1); updated by insert/remove.
    len: usize,
}

impl RadixTrie {
    pub fn new() -> Self {
        Self { root: Box::new(Node::new(Vec::new())), len: 0 }
    }

    /// Insert (or increment frequency of) `term`.
    /// Returns the *new* frequency for the term.
    ///
    /// Inserting an existing term adds `weight` to its frequency rather than
    /// replacing it, which is what makes "index this query again" a bump.
    pub fn insert(&mut self, term: &str, weight: u64) -> u64 {
        let (frequency, is_new) = insert_at(&mut self.root, term.as_bytes(), weight);
        if is_new {
            self.len += 1;
        }
        frequency
    }

    /// Set `term`'s frequency to exactly `frequency`, replacing any existing
    /// value. Used when replaying persisted state at boot, where the stored
    /// number is already the total.
    pub fn set(&mut self, term: &str, frequency: u64) -> bool {
        let (_, is_new) = set_at(&mut self.root, term.as_bytes(), frequency);
        if is_new {
            self.len += 1;
        }
        is_new
    }

    /// Remove `term`. Returns true if it existed.
    pub fn remove(&mut self, term: &str) -> bool {
        let removed = remove_at(&mut self.root, term.as_bytes());
        if removed {
            self.len -= 1;
        }
        removed
    }

    /// Frequency of an exact term, if present.
    pub fn frequency(&self, term: &str) -> Option<u64> {
        let (node, path) = self.descend(term.as_bytes())?;
        // `descend` may stop mid-edge; an exact hit lands on a node boundary.
        if path.len() == term.len() && node.is_terminal {
            Some(node.frequency)
        } else {
            None
        }
    }

    pub fn contains(&self, term: &str) -> bool {
        self.frequency(term).is_some()
    }

    /// Collect up to `k` terms that begin with `prefix`, ordered by frequency
    /// desc (ties broken lexicographically ascending, so results are stable).
    /// An empty prefix returns the top-K terms across the whole trie.
    pub fn prefix_search(&self, prefix: &str, k: usize) -> Vec<(String, u64)> {
        if k == 0 {
            return Vec::new();
        }
        let Some((node, mut path)) = self.descend(prefix.as_bytes()) else {
            return Vec::new();
        };

        let mut top: TopK<Reverse<String>> = TopK::new(k);
        collect_top_k(node, &mut path, &mut top);

        top.into_sorted_vec()
            .into_iter()
            .map(|(score, Reverse(term))| (term, score))
            .collect()
    }

    /// Walk the trie collecting terms within `edit_budget` Levenshtein
    /// distance of `query`, capped at `k` by frequency. Each hit carries the
    /// distance that matched it.
    pub fn fuzzy_search(
        &self,
        query: &str,
        edit_budget: u32,
        k: usize,
    ) -> Vec<(String, u64, u32)> {
        if k == 0 {
            return Vec::new();
        }
        let query = query.as_bytes();
        let row = initial_row(query.len());

        let mut top: TopK<(Reverse<String>, u32)> = TopK::new(k);
        let mut path = Vec::new();
        fuzzy_walk(&self.root, query, &row, edit_budget, &mut path, &mut top);

        top.into_sorted_vec()
            .into_iter()
            .map(|(score, (Reverse(term), distance))| (term, score, distance))
            .collect()
    }

    /// Number of distinct terms stored.
    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn stats(&self) -> TrieStats {
        let mut nodes = 0;
        let mut bytes = 0;
        walk_nodes(&self.root, &mut |node| {
            nodes += 1;
            bytes += node.edge_label.len();
        });
        // `walk_nodes` counts the root, which holds no label and no term.
        TrieStats { nodes: nodes - 1, terms: self.len, bytes }
    }

    /// Descend to the node covering `key`, returning it alongside the full
    /// byte path from the root. The path may be *longer* than `key` when the
    /// key ends part-way along an edge — that node still represents every
    /// term with `key` as a prefix, which is exactly what prefix search wants.
    fn descend(&self, key: &[u8]) -> Option<(&Node, Vec<u8>)> {
        let mut node: &Node = &self.root;
        let mut rest = key;
        let mut path = Vec::with_capacity(key.len());

        while !rest.is_empty() {
            let child = node.children.get(&rest[0])?;
            let cpl = Node::common_prefix_len(&child.edge_label, rest);

            if cpl == child.edge_label.len() {
                // Consumed the whole edge; keep walking.
                path.extend_from_slice(&child.edge_label);
                rest = &rest[cpl..];
                node = child;
            } else if cpl == rest.len() {
                // Key ran out mid-edge — this child covers the prefix.
                path.extend_from_slice(&child.edge_label);
                return Some((child, path));
            } else {
                // Diverged mid-edge: no term has this prefix.
                return None;
            }
        }
        Some((node, path))
    }
}

impl Default for RadixTrie {
    fn default() -> Self {
        Self::new()
    }
}

/// Insert `key` below `node`, adding `weight` to any existing frequency.
/// Returns `(new_frequency, term_was_new)`.
fn insert_at(node: &mut Node, key: &[u8], weight: u64) -> (u64, bool) {
    if key.is_empty() {
        let is_new = !node.is_terminal;
        node.is_terminal = true;
        node.frequency = node.frequency.saturating_add(weight);
        node.recompute_max();
        return (node.frequency, is_new);
    }

    let result = if let Some(child) = node.children.get_mut(&key[0]) {
        let cpl = Node::common_prefix_len(&child.edge_label, key);
        if cpl < child.edge_label.len() {
            // Key diverges part-way along this edge: split it, then let the
            // recursion below either mark the split point terminal or hang a
            // second child off it.
            let tail = child.split_edge_at(cpl);
            child.attach(tail);
        }
        insert_at(child, &key[cpl..], weight)
    } else {
        let mut leaf = Box::new(Node::new(key.to_vec()));
        leaf.is_terminal = true;
        leaf.frequency = weight;
        leaf.recompute_max();
        node.attach(leaf);
        (weight, true)
    };

    node.recompute_max();
    result
}

/// Like `insert_at` but assigns the frequency outright instead of adding.
fn set_at(node: &mut Node, key: &[u8], frequency: u64) -> (u64, bool) {
    if key.is_empty() {
        let is_new = !node.is_terminal;
        node.is_terminal = true;
        node.frequency = frequency;
        node.recompute_max();
        return (frequency, is_new);
    }

    let result = if let Some(child) = node.children.get_mut(&key[0]) {
        let cpl = Node::common_prefix_len(&child.edge_label, key);
        if cpl < child.edge_label.len() {
            let tail = child.split_edge_at(cpl);
            child.attach(tail);
        }
        set_at(child, &key[cpl..], frequency)
    } else {
        let mut leaf = Box::new(Node::new(key.to_vec()));
        leaf.is_terminal = true;
        leaf.frequency = frequency;
        leaf.recompute_max();
        node.attach(leaf);
        (frequency, true)
    };

    node.recompute_max();
    result
}

/// Remove `key` below `node`, compacting on the way back up so that no
/// non-terminal node is left with a single child (invariant #2).
fn remove_at(node: &mut Node, key: &[u8]) -> bool {
    if key.is_empty() {
        if !node.is_terminal {
            return false;
        }
        node.is_terminal = false;
        node.frequency = 0;
        node.recompute_max();
        return true;
    }

    let first = key[0];
    let child_is_now_dead = {
        let Some(child) = node.children.get_mut(&first) else {
            return false;
        };
        let cpl = Node::common_prefix_len(&child.edge_label, key);
        if cpl < child.edge_label.len() {
            // The key diverges mid-edge, so no such term is stored.
            return false;
        }
        if !remove_at(child, &key[cpl..]) {
            return false;
        }

        if !child.is_terminal && child.children.len() == 1 {
            child.merge_with_only_child();
        }
        !child.is_terminal && child.children.is_empty()
    };

    if child_is_now_dead {
        node.children.remove(&first);
    }
    node.recompute_max();
    true
}

/// True when no term in a subtree with this maximum frequency could enter the
/// heap. The comparison is strict: `TopK::offer` breaks score ties on the term
/// itself, so a subtree that merely *matches* the cutoff still has to be
/// visited or the winner among tied terms would depend on `HashMap` order.
fn prunable(max_subtree_freq: u64, cutoff: Option<u64>) -> bool {
    matches!(cutoff, Some(cutoff) if max_subtree_freq < cutoff)
}

/// DFS collecting terminals into a bounded heap. Subtrees whose best possible
/// frequency cannot beat the current cutoff are skipped outright — that is
/// what `max_subtree_freq` buys.
fn collect_top_k(node: &Node, path: &mut Vec<u8>, top: &mut TopK<Reverse<String>>) {
    if node.is_terminal {
        if let Ok(term) = std::str::from_utf8(path) {
            top.offer(node.frequency, Reverse(term.to_string()));
        }
    }

    for child in node.children.values() {
        if prunable(child.max_subtree_freq, top.cutoff()) {
            continue;
        }
        let len = child.edge_label.len();
        path.extend_from_slice(&child.edge_label);
        collect_top_k(child, path, top);
        path.truncate(path.len() - len);
    }
}

/// Trie walk carrying one Levenshtein DP row per node (Hanov's method).
///
/// `row` is the DP row for `path` against the full query. Each child's edge is
/// consumed byte by byte; the moment a row's minimum exceeds the budget the
/// whole subtree is unreachable — every deeper row is pointwise no smaller —
/// so it is abandoned without descending.
fn fuzzy_walk(
    node: &Node,
    query: &[u8],
    row: &[u32],
    budget: u32,
    path: &mut Vec<u8>,
    top: &mut TopK<(Reverse<String>, u32)>,
) {
    if node.is_terminal {
        let distance = row[query.len()];
        if distance <= budget {
            if let Ok(term) = std::str::from_utf8(path) {
                top.offer(node.frequency, (Reverse(term.to_string()), distance));
            }
        }
    }

    for child in node.children.values() {
        if prunable(child.max_subtree_freq, top.cutoff()) {
            continue;
        }

        let mut current = row.to_vec();
        let mut reachable = true;
        for &byte in &child.edge_label {
            current = next_row(&current, query, byte);
            if row_min(&current) > budget {
                reachable = false;
                break;
            }
        }
        if !reachable {
            continue;
        }

        let len = child.edge_label.len();
        path.extend_from_slice(&child.edge_label);
        fuzzy_walk(child, query, &current, budget, path, top);
        path.truncate(path.len() - len);
    }
}

fn walk_nodes(node: &Node, visit: &mut impl FnMut(&Node)) {
    visit(node);
    for child in node.children.values() {
        walk_nodes(child, visit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Assert the structural invariants documented in `trie/mod.rs`.
    fn assert_invariants(trie: &RadixTrie) {
        fn check(node: &Node, is_root: bool) {
            if !is_root {
                assert!(!node.edge_label.is_empty(), "invariant 1: empty non-root edge");
                assert!(
                    node.is_terminal || node.children.len() != 1,
                    "invariant 2: unmerged single-child node"
                );
                assert!(
                    node.is_terminal || !node.children.is_empty(),
                    "invariant 3: non-terminal leaf"
                );
            }
            for (key, child) in &node.children {
                assert_eq!(*key, child.edge_label[0], "child keyed by wrong byte");
                check(child, false);
            }
            let own = if node.is_terminal { node.frequency } else { 0 };
            let expected = node
                .children
                .values()
                .map(|c| c.max_subtree_freq)
                .fold(own, u64::max);
            assert_eq!(node.max_subtree_freq, expected, "stale max_subtree_freq");
        }
        check(&trie.root, true);
    }

    /// Every term in the trie, sorted. The bound is generous relative to any
    /// corpus these tests build, so it never truncates.
    fn terms(trie: &RadixTrie) -> Vec<String> {
        let mut all: Vec<String> = trie
            .prefix_search("", 10_000)
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        all.sort();
        all
    }

    #[test]
    fn empty_trie_finds_nothing() {
        let trie = RadixTrie::new();
        assert!(trie.is_empty());
        assert_eq!(trie.len(), 0);
        assert_eq!(trie.prefix_search("c", 10), Vec::new());
        assert_eq!(trie.prefix_search("", 10), Vec::new());
        assert_invariants(&trie);
    }

    #[test]
    fn single_insert_is_findable_by_every_prefix() {
        let mut trie = RadixTrie::new();
        assert_eq!(trie.insert("cat", 1), 1);
        for prefix in ["", "c", "ca", "cat"] {
            assert_eq!(trie.prefix_search(prefix, 10), vec![("cat".to_string(), 1)]);
        }
        assert_eq!(trie.prefix_search("cats", 10), Vec::new());
        assert_eq!(trie.prefix_search("d", 10), Vec::new());
        assert_eq!(trie.len(), 1);
        assert_invariants(&trie);
    }

    #[test]
    fn diverging_terms_split_the_shared_edge() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        trie.insert("car", 1);

        // Root -> "ca" -> {"t", "r"}
        let ca = trie.root.children.get(&b'c').expect("shared edge");
        assert_eq!(ca.edge_label, b"ca");
        assert!(!ca.is_terminal);
        assert_eq!(ca.children.len(), 2);
        assert_eq!(terms(&trie), vec!["car", "cat"]);
        assert_invariants(&trie);
    }

    #[test]
    fn inserting_a_prefix_of_an_existing_term_splits_and_marks_terminal() {
        let mut trie = RadixTrie::new();
        trie.insert("testing", 1);
        trie.insert("test", 2);

        assert_eq!(trie.len(), 2);
        assert_eq!(trie.frequency("test"), Some(2));
        assert_eq!(trie.frequency("testing"), Some(1));
        assert_eq!(terms(&trie), vec!["test", "testing"]);
        assert_invariants(&trie);
    }

    #[test]
    fn inserting_an_extension_descends_past_a_terminal() {
        let mut trie = RadixTrie::new();
        trie.insert("test", 2);
        trie.insert("testing", 1);

        assert_eq!(trie.len(), 2);
        assert_eq!(trie.frequency("test"), Some(2));
        assert_eq!(trie.frequency("testing"), Some(1));
        assert_invariants(&trie);
    }

    #[test]
    fn repeat_inserts_accumulate_frequency() {
        let mut trie = RadixTrie::new();
        assert_eq!(trie.insert("cat", 5), 5);
        assert_eq!(trie.insert("cat", 3), 8);
        assert_eq!(trie.len(), 1);
        assert_eq!(trie.prefix_search("ca", 10), vec![("cat".to_string(), 8)]);
        assert_invariants(&trie);
    }

    #[test]
    fn set_replaces_rather_than_accumulates() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 5);
        assert!(!trie.set("cat", 2));
        assert_eq!(trie.frequency("cat"), Some(2));
        assert!(trie.set("cab", 9));
        assert_eq!(trie.len(), 2);
        assert_invariants(&trie);
    }

    #[test]
    fn remove_leaves_siblings_findable_and_compacts() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        trie.insert("car", 1);

        assert!(trie.remove("cat"));
        assert_eq!(trie.len(), 1);
        assert_eq!(terms(&trie), vec!["car"]);
        // "ca" + "r" must have collapsed back into a single "car" edge.
        assert_eq!(trie.root.children.get(&b'c').unwrap().edge_label, b"car");
        assert_invariants(&trie);
    }

    #[test]
    fn remove_of_an_absent_term_is_a_no_op() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        assert!(!trie.remove("car"));
        assert!(!trie.remove("ca"));
        assert!(!trie.remove("cats"));
        assert!(!trie.remove(""));
        assert_eq!(trie.len(), 1);
        assert_invariants(&trie);
    }

    #[test]
    fn remove_an_interior_terminal_keeps_the_extension() {
        let mut trie = RadixTrie::new();
        trie.insert("test", 1);
        trie.insert("testing", 1);

        assert!(trie.remove("test"));
        assert_eq!(terms(&trie), vec!["testing"]);
        assert_eq!(trie.root.children.get(&b't').unwrap().edge_label, b"testing");
        assert_invariants(&trie);
    }

    #[test]
    fn removing_everything_empties_the_trie() {
        let mut trie = RadixTrie::new();
        for term in ["cat", "car", "cab", "dog", "do"] {
            trie.insert(term, 1);
        }
        for term in ["cat", "car", "cab", "dog", "do"] {
            assert!(trie.remove(term), "removing {term}");
            assert_invariants(&trie);
        }
        assert!(trie.is_empty());
        assert!(trie.root.children.is_empty());
    }

    #[test]
    fn empty_string_is_a_storable_term() {
        let mut trie = RadixTrie::new();
        assert_eq!(trie.insert("", 4), 4);
        assert_eq!(trie.len(), 1);
        assert_eq!(trie.prefix_search("", 10), vec![(String::new(), 4)]);
        assert!(trie.remove(""));
        assert!(trie.is_empty());
        assert_invariants(&trie);
    }

    #[test]
    fn prefix_search_honours_k_and_orders_by_frequency() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        trie.insert("car", 10);
        trie.insert("cab", 5);
        trie.insert("dog", 100);

        assert_eq!(
            trie.prefix_search("ca", 3),
            vec![("car".into(), 10), ("cab".into(), 5), ("cat".into(), 1)]
        );
        assert_eq!(trie.prefix_search("ca", 2), vec![("car".into(), 10), ("cab".into(), 5)]);
        assert_eq!(trie.prefix_search("ca", 0), Vec::new());
        // Empty prefix ranks across the whole trie.
        assert_eq!(trie.prefix_search("", 1), vec![("dog".into(), 100)]);
    }

    #[test]
    fn equal_frequencies_break_ties_lexicographically() {
        let mut trie = RadixTrie::new();
        for term in ["cd", "ca", "cc", "cb"] {
            trie.insert(term, 7);
        }
        let hits: Vec<String> = trie.prefix_search("c", 4).into_iter().map(|(t, _)| t).collect();
        assert_eq!(hits, vec!["ca", "cb", "cc", "cd"]);
        // The bounded heap must pick the same winners when k truncates.
        let hits: Vec<String> = trie.prefix_search("c", 2).into_iter().map(|(t, _)| t).collect();
        assert_eq!(hits, vec!["ca", "cb"]);
    }

    /// With every frequency equal, the heap cutoff equals every subtree's
    /// maximum. Pruning on `<=` would skip whichever branches happened to come
    /// after the heap filled, making the winners depend on `HashMap` order —
    /// so this is the test that pins the pruning comparison to `<`.
    #[test]
    fn tie_breaks_survive_subtree_pruning() {
        let mut trie = RadixTrie::new();
        let corpus: Vec<String> = (b'a'..=b'z').map(|c| format!("c{}", c as char)).collect();
        for term in &corpus {
            trie.insert(term, 7);
        }

        let hits: Vec<String> = trie.prefix_search("c", 5).into_iter().map(|(t, _)| t).collect();
        assert_eq!(hits, vec!["ca", "cb", "cc", "cd", "ce"]);

        // Same requirement on the fuzzy path, which shares the pruning rule.
        let hits: Vec<String> = trie
            .fuzzy_search("ca", 1, 5)
            .into_iter()
            .map(|(t, _, _)| t)
            .collect();
        assert_eq!(hits, vec!["ca", "cb", "cc", "cd", "ce"]);
    }

    #[test]
    fn subtree_pruning_does_not_drop_winners() {
        // 200 low-frequency terms plus one high-frequency term buried deep;
        // the pruning path must still surface the high one.
        let mut trie = RadixTrie::new();
        for i in 0..200 {
            trie.insert(&format!("term{i:03}"), 1);
        }
        trie.insert("term199deep", 999);
        assert_eq!(trie.prefix_search("term", 1), vec![("term199deep".into(), 999)]);
    }

    #[test]
    fn fuzzy_search_finds_terms_within_budget() {
        let mut trie = RadixTrie::new();
        trie.insert("cascade", 3);
        trie.insert("cascading", 1);
        trie.insert("zebra", 1);

        let hits = trie.fuzzy_search("cscade", 1, 10);
        assert_eq!(hits, vec![("cascade".into(), 3, 1)]);

        let exact = trie.fuzzy_search("cascade", 0, 10);
        assert_eq!(exact, vec![("cascade".into(), 3, 0)]);

        assert_eq!(trie.fuzzy_search("xxxxxx", 1, 10), Vec::new());
    }

    #[test]
    fn fuzzy_search_agrees_with_the_reference_distance() {
        let corpus = ["cat", "cart", "chart", "car", "dog", "cats", "scat", ""];
        let mut trie = RadixTrie::new();
        for term in corpus {
            trie.insert(term, 1);
        }

        for budget in 0..=2u32 {
            let mut expected: Vec<String> = corpus
                .iter()
                .filter(|t| crate::typo::distance(t.as_bytes(), b"cat") <= budget)
                .map(|t| t.to_string())
                .collect();
            expected.sort();

            let mut got: Vec<String> = trie
                .fuzzy_search("cat", budget, 100)
                .into_iter()
                .inspect(|(term, _, distance)| {
                    assert_eq!(*distance, crate::typo::distance(term.as_bytes(), b"cat"));
                })
                .map(|(term, _, _)| term)
                .collect();
            got.sort();

            assert_eq!(got, expected, "budget {budget}");
        }
    }

    #[test]
    fn fuzzy_search_honours_k_by_frequency() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        trie.insert("cot", 50);
        trie.insert("cut", 20);

        let hits = trie.fuzzy_search("cat", 1, 2);
        assert_eq!(hits, vec![("cot".into(), 50, 1), ("cut".into(), 20, 1)]);
    }

    #[test]
    fn stats_track_terms_and_labels() {
        let mut trie = RadixTrie::new();
        trie.insert("cat", 1);
        trie.insert("car", 1);
        let stats = trie.stats();
        assert_eq!(stats.terms, 2);
        // "ca" + "t" + "r"
        assert_eq!(stats.nodes, 3);
        assert_eq!(stats.bytes, 4);
    }

    /// A deterministic pseudo-random corpus: insert, verify, remove half,
    /// verify again. Cheaper than pulling proptest into the unit tests while
    /// still exercising the split/merge paths in bulk.
    #[test]
    fn bulk_insert_and_remove_round_trips() {
        let mut seed = 0x2545_F491_4F6C_DD1Du64;
        let mut next = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        let mut corpus: Vec<String> = Vec::new();
        for _ in 0..500 {
            let len = (next() % 12) as usize + 1;
            let word: String = (0..len)
                .map(|_| (b'a' + (next() % 6) as u8) as char)
                .collect();
            corpus.push(word);
        }
        corpus.sort();
        corpus.dedup();

        let mut trie = RadixTrie::new();
        for (i, term) in corpus.iter().enumerate() {
            trie.insert(term, i as u64 + 1);
        }
        assert_eq!(trie.len(), corpus.len());
        assert_invariants(&trie);
        assert_eq!(terms(&trie), corpus);

        // Every stored term must be reachable from each of its own prefixes.
        for term in &corpus {
            for cut in 0..=term.len() {
                let prefix = &term[..cut];
                let hits = trie.prefix_search(prefix, corpus.len());
                assert!(
                    hits.iter().any(|(t, _)| t == term),
                    "{term} missing under prefix {prefix:?}"
                );
            }
        }

        let (dropped, kept): (Vec<_>, Vec<_>) =
            corpus.iter().enumerate().partition(|(i, _)| i % 2 == 0);
        for (_, term) in &dropped {
            assert!(trie.remove(term), "removing {term}");
        }
        assert_invariants(&trie);
        assert_eq!(trie.len(), kept.len());

        let mut expected: Vec<String> = kept.iter().map(|(_, t)| (*t).clone()).collect();
        expected.sort();
        assert_eq!(terms(&trie), expected);
    }
}
