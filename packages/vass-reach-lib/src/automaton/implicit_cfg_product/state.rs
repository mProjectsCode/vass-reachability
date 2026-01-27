use std::ops::Index;

use petgraph::graph::NodeIndex;

use crate::automaton::{
    TransitionSystem,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
};

/// A state in the product of multiple graphs, storing the individual states in
/// each graph.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MultiGraphState {
    pub states: Box<[NodeIndex]>,
}

impl MultiGraphState {
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
            if let Some(target) = cfg.successor(current_state, letter) {
                new_states.push(target);
            } else {
                return None;
            }
        }

        Some(MultiGraphState {
            states: new_states.into_boxed_slice(),
        })
    }
}

impl Index<usize> for MultiGraphState {
    type Output = NodeIndex;

    fn index(&self, index: usize) -> &Self::Output {
        &self.states[index]
    }
}
