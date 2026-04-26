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
