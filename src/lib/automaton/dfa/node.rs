use crate::automaton::AutomatonNode;

/// A node in a DFA.
/// It contains some data of type `T` and a boolean flag indicating whether the
/// node is accepting.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct DfaNode<T: AutomatonNode> {
    pub accepting: bool,
    pub data: T,
}

impl<T: AutomatonNode> DfaNode<T> {
    pub fn new(accepting: bool, data: T) -> Self {
        DfaNode { accepting, data }
    }

    pub fn accepting(&self) -> bool {
        self.accepting
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn invert(&self) -> Self {
        DfaNode::new(!self.accepting, self.data.clone())
    }

    pub fn invert_mut(&mut self) {
        self.accepting = !self.accepting;
    }

    pub fn join<TO: AutomatonNode>(&self, other: &DfaNode<TO>) -> DfaNode<(T, TO)> {
        DfaNode::new(
            self.accepting && other.accepting,
            (self.data.clone(), other.data.clone()),
        )
    }

    pub fn join_left<TO: AutomatonNode>(&self, other: &DfaNode<TO>) -> DfaNode<T> {
        DfaNode::new(self.accepting && other.accepting, self.data.clone())
    }
}
