use std::cmp::Ordering;

use crate::automaton::{GIndex, Letter, path::Path};

mod build;
mod dag;
mod graphviz;
mod rolling;

pub use build::SCCAlgorithms;
pub use dag::{SCC, SCCDag, SCCDagEdge};

pub(super) fn sort_and_dedup_component_edges<NIndex: GIndex, L: Letter>(
    edges: &mut Vec<SCCDagEdge<NIndex, L>>,
) {
    // Keep deterministic order and remove structurally identical alternatives.
    edges.sort_by(|left, right| {
        compare_paths(&left.path, &right.path)
            .then(left.target_component.cmp(&right.target_component))
    });
    edges.dedup_by(|left, right| {
        left.target_component == right.target_component && left.path == right.path
    });
}

fn compare_paths<NIndex: GIndex, L: Letter>(
    left: &Path<NIndex, L>,
    right: &Path<NIndex, L>,
) -> Ordering {
    left.states
        .cmp(&right.states)
        .then(left.transitions.cmp(&right.transitions))
}
