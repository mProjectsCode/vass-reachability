use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};

use crate::automaton::cfg::update::CFGCounterUpdate;

pub mod update;
pub mod vasscfg;

pub trait CFG {
    type N;
    type E;

    fn get_graph(&self) -> &DiGraph<Self::N, Self::E>;

    fn edge_update(&self, edge: EdgeIndex) -> CFGCounterUpdate;

    fn get_start(&self) -> NodeIndex;

    fn is_accepting(&self, node: NodeIndex) -> bool;

    fn state_count(&self) -> usize {
        self.get_graph().node_count()
    }

    fn edge_count(&self) -> usize {
        self.get_graph().edge_count()
    }
}
