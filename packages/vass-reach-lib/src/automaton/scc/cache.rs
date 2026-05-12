use std::sync::Arc;

use hashbrown::HashMap;

use super::{
    SCC,
    build::{compute_sccs, reachable_from},
};
use crate::automaton::{Deterministic, GIndex, InitializedAutomaton};

/// A precomputed SCC decomposition of the reachable graph.
#[derive(Debug)]
pub struct PrecomputedSccs<NIndex: GIndex> {
    components: Vec<SCC<NIndex>>,
    component_of_node: HashMap<NIndex, usize>,
}

impl<NIndex: GIndex> PrecomputedSccs<NIndex> {
    pub fn component(&self, component: usize) -> &SCC<NIndex> {
        &self.components[component]
    }

    pub fn component_index(&self, state: &NIndex) -> Option<usize> {
        self.component_of_node.get(state).copied()
    }

    /// Computes the SCCs of the graph reachable from `initial` once up front.
    pub fn from_reachable<A>(automaton: &A, initial: NIndex) -> Self
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex> + ?Sized,
    {
        let reachable = reachable_from(&initial, |current| automaton.successors(current));
        let (components, component_of_node) = compute_sccs(automaton, &reachable, &|_| false);

        Self {
            components,
            component_of_node,
        }
    }
}

/// Lightweight reference to a precomputed SCC.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SccRef {
    pub component: usize,
    pub cyclic: bool,
}

impl SccRef {
    /// Returns whether this component is a singleton without a self-loop.
    pub fn is_trivial(&self) -> bool {
        !self.cyclic
    }
}

/// Caches precomputed SCCs.
pub struct SccCache<NIndex: GIndex> {
    sccs: Arc<PrecomputedSccs<NIndex>>,
}

impl<NIndex: GIndex> SccCache<NIndex> {
    pub fn new(sccs: Arc<PrecomputedSccs<NIndex>>) -> Self {
        Self { sccs }
    }

    pub fn get_scc_for_state(&self, state: &NIndex) -> SccRef {
        let component = self
            .sccs
            .component_index(state)
            .expect("State must belong to the precomputed reachable SCC graph");
        let cyclic = self.sccs.component(component).cyclic;

        SccRef { component, cyclic }
    }

    pub fn get_sccs(&self) -> &Arc<PrecomputedSccs<NIndex>> {
        &self.sccs
    }
}
