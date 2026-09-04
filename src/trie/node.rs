//! A single radix-tree node.
//!
//! Children live in a `HashMap<u8, Box<Node>>` keyed by the first byte of the
//! child's edge label. That is deliberately unsophisticated: swap it for a
//! sorted vec or a 256-slot array only if profiling says child lookup is hot.

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

    /// Largest frequency anywhere in this node's subtree, including itself.
    /// Maintained on insert/remove so top-K search can prune whole subtrees
    /// that cannot beat the current heap cutoff.
    pub(crate) max_subtree_freq: u64,
}

impl Node {
    /// A fresh, non-terminal node. Callers decide terminality afterwards.
    pub(crate) fn new(edge_label: Vec<u8>) -> Self {
        Self {
            edge_label,
            children: HashMap::new(),
            is_terminal: false,
            frequency: 0,
            max_subtree_freq: 0,
        }
    }

    /// Find the length of the longest common prefix between two byte slices.
    /// Used for edge splitting during `insert`.
    pub(crate) fn common_prefix_len(a: &[u8], b: &[u8]) -> usize {
        a.iter().zip(b.iter()).take_while(|(x, y)| x == y).count()
    }

    /// Split this node's edge at `at`, keeping the first `at` bytes here and
    /// moving everything else — the suffix, the children, the terminal marker
    /// — into a new child, which is returned for the caller to attach.
    ///
    /// Inserting "team" alongside a node with edge "test" calls
    /// `split_edge_at(2)`: this node's edge becomes "te", the returned child
    /// carries "st", and the caller then adds "am" as its sibling.
    pub(crate) fn split_edge_at(&mut self, at: usize) -> Box<Node> {
        debug_assert!(at > 0 && at < self.edge_label.len());

        let suffix = self.edge_label.split_off(at);
        let mut child = Box::new(Node::new(suffix));
        child.children = std::mem::take(&mut self.children);
        child.is_terminal = self.is_terminal;
        child.frequency = self.frequency;
        child.max_subtree_freq = self.max_subtree_freq;

        self.is_terminal = false;
        self.frequency = 0;
        // `self.max_subtree_freq` is restored by `recompute_max` once the
        // caller has attached both children.
        child
    }

    /// Absorb this node's only child, concatenating the edge labels. Used
    /// during `remove` compaction to restore invariant #2 (see `mod.rs`).
    ///
    /// The map key in the *parent* is the first byte of `edge_label`, which
    /// this does not change, so the parent needs no fix-up.
    pub(crate) fn merge_with_only_child(&mut self) {
        debug_assert!(!self.is_terminal && self.children.len() == 1);

        let key = *self.children.keys().next().expect("exactly one child");
        let child = self.children.remove(&key).expect("key came from the map");
        let Node { edge_label, children, is_terminal, frequency, max_subtree_freq } = *child;

        self.edge_label.extend_from_slice(&edge_label);
        self.children = children;
        self.is_terminal = is_terminal;
        self.frequency = frequency;
        self.max_subtree_freq = max_subtree_freq;
    }

    /// Recompute `max_subtree_freq` from this node and its (already correct)
    /// children. Call on the way back up any mutating walk.
    pub(crate) fn recompute_max(&mut self) {
        let own = if self.is_terminal { self.frequency } else { 0 };
        self.max_subtree_freq = self
            .children
            .values()
            .map(|c| c.max_subtree_freq)
            .fold(own, u64::max);
    }

    /// Attach `child`, keyed by the first byte of its edge label.
    pub(crate) fn attach(&mut self, child: Box<Node>) {
        let key = *child.edge_label.first().expect("non-root edges are non-empty");
        self.children.insert(key, child);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn common_prefix_len_handles_the_edges() {
        assert_eq!(Node::common_prefix_len(b"", b"abc"), 0);
        assert_eq!(Node::common_prefix_len(b"abc", b""), 0);
        assert_eq!(Node::common_prefix_len(b"test", b"team"), 2);
        assert_eq!(Node::common_prefix_len(b"test", b"test"), 4);
        assert_eq!(Node::common_prefix_len(b"test", b"testing"), 4);
        assert_eq!(Node::common_prefix_len(b"xyz", b"abc"), 0);
    }

    #[test]
    fn split_moves_the_payload_to_the_child() {
        let mut parent = Node::new(b"test".to_vec());
        parent.is_terminal = true;
        parent.frequency = 7;
        parent.recompute_max();

        let child = parent.split_edge_at(2);

        assert_eq!(parent.edge_label, b"te");
        assert!(!parent.is_terminal);
        assert_eq!(parent.frequency, 0);
        assert_eq!(child.edge_label, b"st");
        assert!(child.is_terminal);
        assert_eq!(child.frequency, 7);
        assert_eq!(child.max_subtree_freq, 7);
    }

    #[test]
    fn merge_concatenates_edges_and_takes_the_payload() {
        let mut parent = Node::new(b"te".to_vec());
        let mut child = Box::new(Node::new(b"st".to_vec()));
        child.is_terminal = true;
        child.frequency = 3;
        child.recompute_max();
        parent.attach(child);

        parent.merge_with_only_child();

        assert_eq!(parent.edge_label, b"test");
        assert!(parent.is_terminal);
        assert_eq!(parent.frequency, 3);
        assert!(parent.children.is_empty());
    }
}
