//! A single radix-tree node.
//!
//! HINT: start dead simple — `HashMap<u8, Box<Node>>` for children.
//! Measure before optimizing. Only reach for `[Option<Box<Node>>; 256]`
//! or sorted vecs if profiling says child lookup is hot.

use std::collections::HashMap;

pub(crate) struct Node {
    /// Bytes on the edge leading INTO this node from its parent.
    /// Root has an empty label.
    pub(crate) edge_label: Vec<u8>,

    /// Keyed by the first byte of the child's edge_label.
    pub(crate) children: HashMap<u8, Box<Node>>,

    /// True iff a complete term ends at this node.
    pub(crate) is_terminal: bool,

    /// Frequency / weight of the term ending here (0 if !is_terminal).
    pub(crate) frequency: u64,

    // HINT: for fast top-K pruning, cache the max frequency
    // in this subtree. Update on insert/remove.
    // pub(crate) max_subtree_freq: u64,
}

impl Node {
    pub(crate) fn new(edge_label: Vec<u8>) -> Self {
        // HINT: constructor should not mark terminal; callers decide.
        let _ = edge_label;
        todo!("construct an empty non-terminal node with the given edge label")
    }

    /// Find the length of the longest common prefix between two byte slices.
    /// Used for edge splitting during `insert`.
    pub(crate) fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
        // HINT: 3-line implementation with .zip() + .take_while().
        let _ = (a, b);
        todo!()
    }

    // HINT: helper for edge splitting.
    // When inserting "team" into a node with edge "test", you need to:
    //   1. create a new intermediate node with edge "te"
    //   2. shorten the existing node's edge to "st"
    //   3. add the new "am" node as a sibling under the intermediate
    // Consider a method: `fn split_edge_at(&mut self, at: usize) -> Node`
    // that returns the *new* child and mutates self to be the parent-side.
}
