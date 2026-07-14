//! Levenshtein-bounded search over the radix trie.
//!
//! Two approaches, in order of complexity:
//!
//! 1. **Row-DP walk (recommended first pass)** — carry a DP row of length
//!    `query.len() + 1` down through the trie. For each byte of an edge
//!    label, compute the next row; abort if `row.iter().min() > budget`.
//!    O(query_len × trie_nodes_visited).
//!
//! 2. **Levenshtein automaton** — precompute a DFA over `query` at
//!    distance ≤ k, then run the DFA in lockstep with the trie DFS.
//!    Faster in practice for k ≥ 2 (Schulz & Mihov 2002). More code.
//!
//! Ship #1 first, benchmark, then decide if #2 pays off.

/// Compute the next Levenshtein DP row given the previous row and the
/// character we just consumed (from the trie edge).
///
/// HINT: classic Wagner–Fischer transition, one row at a time.
///   next[0] = prev[0] + 1
///   next[i] = min(
///       prev[i]   + 1,               // deletion
///       next[i-1] + 1,               // insertion
///       prev[i-1] + (query[i-1] != ch) as u32   // substitution
///   )
pub fn next_row(prev: &[u32], query: &[u8], ch: u8) -> Vec<u32> {
    let _ = (prev, query, ch);
    todo!()
}

/// The initial row: [0, 1, 2, ..., query.len()].
pub fn initial_row(query_len: usize) -> Vec<u32> {
    let _ = query_len;
    todo!()
}

// HINT: the actual trie walk lives in RadixTrie::fuzzy_search. That method
// keeps `Vec<u8>` state for the accumulated path and calls `next_row` for
// each byte of each edge. Prune when `row.iter().min() > budget`.
