use super::rolling::{compact_removed_trivial_components_in_place, roll_trivial_paths_in_place};
use crate::automaton::{GIndex, Letter, path::Path};

/// Metadata for a single strongly connected component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCC<NIndex: GIndex> {
    /// Nodes in this component, kept in sorted order for deterministic
    /// behavior.
    pub nodes: Vec<NIndex>,
    /// Subset of `nodes` that satisfy the caller-provided acceptance predicate.
    pub accepting_nodes: Vec<NIndex>,
    /// `true` for multi-node SCCs and for singleton SCCs with a self-loop.
    pub cyclic: bool,
}

impl<NIndex: GIndex> SCC<NIndex> {
    /// Returns whether this component is a singleton without a self-loop.
    pub fn is_trivial(&self) -> bool {
        self.nodes.len() == 1 && !self.cyclic
    }
}

/// Condensation DAG where each vertex is an SCC from the relevant automaton
/// subgraph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCCDag<NIndex: GIndex, L: Letter> {
    /// Index of the SCC containing the chosen root/initial node.
    pub root_component: usize,
    /// All SCC vertices in topological-like discovery order.
    pub components: Vec<SCC<NIndex>>,
    /// For each component, outgoing inter-component edges.
    pub edges: Vec<Vec<SCCDagEdge<NIndex, L>>>,
    /// True iff non-accepting trivial SCCs have been bypassed so cross-
    /// component edges may carry longer connector paths.
    pub trivial_paths_rolled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SCCDagRouteSummary {
    pub components: usize,
    pub edges: usize,
    pub accepting_components: usize,
    pub accepting_states: usize,
    pub accepting_component_routes: u128,
    pub accepting_state_routes: u128,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCCDagEdge<NIndex: GIndex, L: Letter> {
    /// Witness path from source SCC to target SCC.
    ///
    /// The first state is in the source component (exit point), the last state
    /// is in the target component (entry point), and any intermediate states
    /// are outside both endpoint components.
    pub path: Path<NIndex, L>,
    /// Index into `SCCDag::components`.
    pub target_component: usize,
}

impl<NIndex: GIndex, L: Letter> SCCDag<NIndex, L> {
    /// Returns the root SCC (the component containing the original initial
    /// node).
    pub fn root(&self) -> &SCC<NIndex> {
        &self.components[self.root_component]
    }

    /// Returns all outgoing cross-component edges from `component`.
    pub fn outgoing_edges(&self, component: usize) -> &[SCCDagEdge<NIndex, L>] {
        &self.edges[component]
    }

    pub fn accepting_route_summary(&self) -> SCCDagRouteSummary {
        SCCDagRouteSummary {
            components: self.components.len(),
            edges: self.edge_count(),
            accepting_components: self.accepting_component_count(),
            accepting_states: self.accepting_state_count(),
            accepting_component_routes: self.count_root_to_accepting_routes(false),
            accepting_state_routes: self.count_root_to_accepting_routes(true),
        }
    }

    pub fn edge_count(&self) -> usize {
        self.edges.iter().map(Vec::len).sum()
    }

    pub fn accepting_component_count(&self) -> usize {
        self.components
            .iter()
            .filter(|component| !component.accepting_nodes.is_empty())
            .count()
    }

    pub fn accepting_state_count(&self) -> usize {
        self.components
            .iter()
            .map(|component| component.accepting_nodes.len())
            .sum()
    }

    pub fn count_root_to_accepting_component_routes(&self) -> u128 {
        self.count_root_to_accepting_routes(false)
    }

    pub fn count_root_to_accepting_state_routes(&self) -> u128 {
        self.count_root_to_accepting_routes(true)
    }

    fn count_root_to_accepting_routes(&self, count_accepting_states: bool) -> u128 {
        if self.components.is_empty() {
            return 0;
        }

        let mut memo = vec![None; self.components.len()];
        self.count_root_to_accepting_routes_from(
            self.root_component,
            count_accepting_states,
            &mut memo,
        )
    }

    fn count_root_to_accepting_routes_from(
        &self,
        component: usize,
        count_accepting_states: bool,
        memo: &mut [Option<u128>],
    ) -> u128 {
        if let Some(count) = memo[component] {
            return count;
        }

        let accepting_count = self.components[component].accepting_nodes.len();
        let own_routes = if count_accepting_states {
            accepting_count as u128
        } else if accepting_count > 0 {
            1
        } else {
            0
        };

        let outgoing_routes = self
            .outgoing_edges(component)
            .iter()
            .fold(0u128, |sum, edge| {
                sum.saturating_add(self.count_root_to_accepting_routes_from(
                    edge.target_component,
                    count_accepting_states,
                    memo,
                ))
            });

        let total = own_routes.saturating_add(outgoing_routes);
        memo[component] = Some(total);
        total
    }

    /// Returns a copy where non-accepting trivial SCCs are bypassed by
    /// concatenating incoming and outgoing edge paths.
    ///
    /// Accepting trivial SCCs are intentionally kept, because they represent
    /// valid terminal stopping points.
    pub fn with_rolled_trivial_paths(&self) -> Self {
        if self.trivial_paths_rolled {
            return self.clone();
        }

        let mut simplified = self.clone();

        roll_trivial_paths_in_place(&mut simplified);
        compact_removed_trivial_components_in_place(&mut simplified);

        simplified.trivial_paths_rolled = true;
        simplified
    }
}
