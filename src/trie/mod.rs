//! Compressed trie (radix tree).
//!
//! Data structure invariants — enforce these in tests:
//! 1. No node (except root) has an empty `edge_label`.
//! 2. No node has exactly one non-terminal child that could be merged.
//! 3. `is_terminal == true` iff a term ends exactly at this node — so no
//!    non-terminal leaves survive a `remove`.
//!
//! `node.rs` holds the data and the edge-splitting mechanics; `radix_trie.rs`
//! holds the public API (insert, remove, prefix_search, fuzzy_search).

mod node;
mod radix_trie;

pub use radix_trie::{RadixTrie, TrieStats};
