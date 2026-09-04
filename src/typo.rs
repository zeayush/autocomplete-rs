//! Levenshtein-bounded search over the radix trie.
//!
//! Two approaches, in order of complexity:
//!
//! 1. **Row-DP walk (what we ship)** — carry a DP row of length
//!    `query.len() + 1` down through the trie. For each byte of an edge
//!    label, compute the next row; abort if `row.iter().min() > budget`.
//!    O(query_len × trie_nodes_visited).
//!
//! 2. **Levenshtein automaton** — precompute a DFA over `query` at
//!    distance ≤ k, then run the DFA in lockstep with the trie DFS.
//!    Faster in practice for k ≥ 2 (Schulz & Mihov 2002). More code.
//!
//! The walk is byte-oriented, matching the trie's byte-oriented edges. For
//! ASCII that is the usual character-level Levenshtein distance; for
//! multi-byte UTF-8 a single character substitution can cost more than one
//! edit. Callers indexing non-ASCII corpora should budget accordingly.

/// Compute the next Levenshtein DP row given the previous row and the
/// byte we just consumed from a trie edge.
///
/// Classic Wagner–Fischer transition, one row at a time:
///   next[0] = prev[0] + 1
///   next[i] = min(deletion, insertion, substitution)
pub fn next_row(prev: &[u32], query: &[u8], ch: u8) -> Vec<u32> {
    debug_assert_eq!(prev.len(), query.len() + 1);

    let mut next = Vec::with_capacity(prev.len());
    next.push(prev[0] + 1);
    for i in 1..prev.len() {
        let sub_cost = u32::from(query[i - 1] != ch);
        let best = (prev[i] + 1)
            .min(next[i - 1] + 1)
            .min(prev[i - 1] + sub_cost);
        next.push(best);
    }
    next
}

/// The initial row: `[0, 1, 2, ..., query_len]`.
pub fn initial_row(query_len: usize) -> Vec<u32> {
    (0..=query_len as u32).collect()
}

/// Smallest value in a DP row — the walk prunes a subtree when this exceeds
/// the edit budget, since every descendant row is monotonically ≥ this one.
pub fn row_min(row: &[u32]) -> u32 {
    row.iter().copied().min().unwrap_or(0)
}

/// Full Levenshtein distance between two byte strings. Only used by tests and
/// as a reference implementation; the trie walk never materializes a matrix.
pub fn distance(a: &[u8], b: &[u8]) -> u32 {
    let mut row = initial_row(b.len());
    for &ch in a {
        row = next_row(&row, b, ch);
    }
    row[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_row_is_the_identity_ladder() {
        assert_eq!(initial_row(0), vec![0]);
        assert_eq!(initial_row(3), vec![0, 1, 2, 3]);
    }

    #[test]
    fn distance_matches_known_pairs() {
        assert_eq!(distance(b"", b""), 0);
        assert_eq!(distance(b"cat", b"cat"), 0);
        assert_eq!(distance(b"cat", b"cut"), 1);
        assert_eq!(distance(b"cat", b"cats"), 1);
        assert_eq!(distance(b"cascade", b"cscade"), 1);
        assert_eq!(distance(b"kitten", b"sitting"), 3);
        assert_eq!(distance(b"", b"abc"), 3);
    }

    #[test]
    fn row_min_bounds_the_reachable_distance() {
        // After consuming "ca" against query "cat", a completion is still
        // reachable at distance 0 (namely "t"), so the row minimum is 0.
        let mut row = initial_row(3);
        for &ch in b"ca" {
            row = next_row(&row, b"cat", ch);
        }
        assert_eq!(row_min(&row), 0);

        // "zzz" cannot be rescued by any suffix within 1 edit.
        let mut row = initial_row(3);
        for &ch in b"zzz" {
            row = next_row(&row, b"cat", ch);
        }
        assert!(row_min(&row) > 1);
    }
}
