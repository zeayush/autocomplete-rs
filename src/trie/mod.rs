//! Compressed trie (radix tree).
//!
//! Data structure invariants — enforce these in tests:
//! 1. No node (except root) has an empty `edge_label`.
//! 2. No node has exactly one non-terminal child that could be merged.
//! 3. `is_terminal == true` iff a term ends exactly at this node.
//!
//! HINT: split `node.rs` (data + edge-splitting) from `radix_trie.rs`
//! (public API: insert, remove, prefix_search, walk_with_budget).

mod node;
mod radix_trie;

pub use radix_trie::RadixTrie;

// HINT: consider exposing a `TrieStats { nodes, terms, bytes }` type
// so /health can report memory footprint per tenant.
