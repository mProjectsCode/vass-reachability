use std::ops::Index;

use petgraph::graph::NodeIndex;

use crate::automaton::{
    TransitionSystem,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
};

/// A state in the product of multiple graphs, storing the individual states in
/// each graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct MultiGraphState {
    pub states: Box<[NodeIndex]>,
}

impl MultiGraphState {
    pub fn cfg_state(&self, cfg_index: usize) -> NodeIndex {
        self.states[cfg_index]
    }

    /// Takes a letter in the product of multiple graphs, returning the target
    /// MultiGraphState if it exists. If any graph does not have a
    /// transition for the letter, returns None.
    ///
    /// Assumes that all graphs are in the same order as the node indices in the
    /// MultiGraphState.
    pub fn take_letter(
        &self,
        graphs: &[VASSCFG<()>],
        letter: &CFGCounterUpdate,
    ) -> Option<MultiGraphState> {
        let mut new_states = vec![];

        for (i, cfg) in graphs.iter().enumerate() {
            let current_state = self.states[i];
            if let Some(target) = cfg.successor(&current_state, letter) {
                new_states.push(target);
            } else {
                return None;
            }
        }

        Some(MultiGraphState {
            states: new_states.into_boxed_slice(),
        })
    }

    /// Clears the indices in the given range, setting them to
    /// `NodeIndex::end()`.
    ///
    /// This is useful for tracking only a subset of the graphs in the product,
    /// for example when we want to ignore the bounded counting separators.
    pub fn clear_indices(&mut self, range: std::ops::Range<usize>) {
        for i in range {
            self.states[i] = NodeIndex::end();
        }
    }

    /// Clears the index at the given position, setting it to
    /// `NodeIndex::end()`.
    pub fn clear_index(&mut self, index: usize) {
        self.states[index] = NodeIndex::end();
    }
}

impl Index<usize> for MultiGraphState {
    type Output = NodeIndex;

    fn index(&self, index: usize) -> &Self::Output {
        &self.states[index]
    }
}

impl From<Vec<NodeIndex>> for MultiGraphState {
    fn from(states: Vec<NodeIndex>) -> Self {
        MultiGraphState {
            states: states.into_boxed_slice(),
        }
    }
}

impl From<Box<[NodeIndex]>> for MultiGraphState {
    fn from(states: Box<[NodeIndex]>) -> Self {
        MultiGraphState { states }
    }
}

impl From<NodeIndex> for MultiGraphState {
    fn from(state: NodeIndex) -> Self {
        MultiGraphState {
            states: vec![state].into_boxed_slice(),
        }
    }
}
