use crate::automaton::AutomatonNode;

/// A node in a DFA.
/// It contains some data of type `T`, a boolean flag indicating whether the
/// node is accepting, and a boolean flag indicating whether the node is a trap
/// node.
///
/// Invariant: A node cannot be both accepting and a trap node.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DfaNode<T: AutomatonNode> {
    pub accepting: bool,
    /// Whether the node is a trap node. Meaning from it there is no way to
    /// reach an accepting state. When it's unknown whether it's a trap
    /// node, this is set to false.
    pub trap: bool,
    pub data: T,
}

impl<T: AutomatonNode> DfaNode<T> {
    pub fn new(accepting: bool, trap: bool, data: T) -> Self {
        assert!(
            !(accepting && trap),
            "A node cannot be both accepting and a trap node"
        );
        DfaNode {
            accepting,
            trap,
            data,
        }
    }

    pub fn accepting(data: T) -> Self {
        DfaNode::new(true, false, data)
    }

    pub fn non_accepting(data: T) -> Self {
        DfaNode::new(false, false, data)
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn invert(&self) -> Self {
        DfaNode::new(!self.accepting, false, self.data.clone())
    }

    pub fn invert_mut(&mut self) {
        self.accepting = !self.accepting;
        self.trap = false;
    }

    pub fn join<TO: AutomatonNode>(&self, other: &DfaNode<TO>) -> DfaNode<(T, TO)> {
        DfaNode::new(
            self.accepting && other.accepting,
            self.trap || other.trap,
            (self.data.clone(), other.data.clone()),
        )
    }

    pub fn join_left<TO: AutomatonNode>(&self, other: &DfaNode<TO>) -> DfaNode<T> {
        DfaNode::new(
            self.accepting && other.accepting,
            self.trap || other.trap,
            self.data.clone(),
        )
    }
}

impl<T: Default + AutomatonNode> DfaNode<T> {
    pub fn default_accepting() -> Self {
        DfaNode::new(true, false, T::default())
    }
}

impl<T: Default + AutomatonNode> Default for DfaNode<T> {
    fn default() -> Self {
        DfaNode::new(false, false, T::default())
    }
}
