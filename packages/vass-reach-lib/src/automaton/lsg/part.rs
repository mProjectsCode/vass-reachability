use itertools::Itertools;
use petgraph::{
    Direction,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::automaton::{
    Alphabet, Automaton, ExplicitEdgeAutomaton, Frozen, GIndex, InitializedAutomaton, Language,
    ModifiableAutomaton, SingleFinalStateAutomaton,
    cfg::{CFG, update::CFGCounterUpdate},
    path::Path,
};

#[derive(Debug, Clone)]
pub struct LSGGraph<NIndex: GIndex> {
    pub graph: DiGraph<NIndex, CFGCounterUpdate>,
    // start index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub start: NodeIndex,
    // end index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub end: NodeIndex,
}

impl<NIndex: GIndex> LSGGraph<NIndex> {
    pub fn new(graph: DiGraph<NIndex, CFGCounterUpdate>, start: NodeIndex, end: NodeIndex) -> Self {
        assert!(
            start.index() < graph.node_count(),
            "Start node {:?} must be in the graph",
            start
        );
        assert!(
            end.index() < graph.node_count(),
            "End node {:?} must be in the graph",
            end
        );

        LSGGraph { graph, start, end }
    }

    pub fn cfg_start(&self) -> NIndex {
        self.get_node_unchecked(self.start).clone()
    }

    pub fn cfg_end(&self) -> NIndex {
        self.get_node_unchecked(self.end).clone()
    }

    /// Maps a path in the LSG back to a path in the CFG.
    pub fn map_path_to_cfg(
        &self,
        path: &Path<NodeIndex, CFGCounterUpdate>,
    ) -> Path<NIndex, CFGCounterUpdate> {
        let mut mapped_path = Path::new(self.map_node_to_cfg(path.start()));

        for (update, node) in path.iter() {
            let mapped_node = self.map_node_to_cfg(*node);
            mapped_path.add(*update, mapped_node);
        }

        mapped_path
    }

    pub fn map_node_to_cfg(&self, node: NodeIndex) -> NIndex {
        self.graph[node]
    }

    pub fn map_edge_to_cfg<C: CFG<NIndex = NIndex>>(&self, edge: EdgeIndex, cfg: &C) -> C::EIndex {
        let (src, dst) = self
            .graph
            .edge_endpoints(edge)
            .expect("subgraph does not contain edge");
        let edge_update = self
            .graph
            .edge_weight(edge)
            .expect("subgraph does not contain edge");

        let mapped_src = self.map_node_to_cfg(src);
        let mapped_dst = self.map_node_to_cfg(dst);

        for edge in cfg.connecting_edge_indices(mapped_src, mapped_dst) {
            if cfg.get_edge_unchecked(edge) == edge_update {
                return edge;
            }
        }

        panic!(
            "Could not find corresponding edge in CFG for edge {:?}",
            edge
        );
    }
}

impl<NIndex: GIndex> Alphabet for LSGGraph<NIndex> {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        todo!()
    }
}

impl<NIndex: GIndex> Automaton for LSGGraph<NIndex> {
    type NIndex = NodeIndex;
    type N = NIndex;

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn get_node(&self, index: Self::NIndex) -> Option<&NIndex> {
        self.graph.node_weight(index)
    }

    fn get_node_unchecked(&self, index: Self::NIndex) -> &NIndex {
        &self.graph[index]
    }
}

impl<NIndex: GIndex> ExplicitEdgeAutomaton for LSGGraph<NIndex> {
    type EIndex = EdgeIndex;
    type E = CFGCounterUpdate;

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn get_edge(&self, index: Self::EIndex) -> Option<&CFGCounterUpdate> {
        self.graph.edge_weight(index)
    }

    fn get_edge_unchecked(&self, index: Self::EIndex) -> &CFGCounterUpdate {
        self.graph.edge_weight(index).unwrap()
    }

    fn edge_endpoints(&self, edge: Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)> {
        self.graph.edge_endpoints(edge)
    }

    fn edge_endpoints_unchecked(&self, edge: Self::EIndex) -> (Self::NIndex, Self::NIndex) {
        self.graph.edge_endpoints(edge).unwrap()
    }

    fn outgoing_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, petgraph::Direction::Outgoing)
            .map(|edge| edge.id())
    }

    fn incoming_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, petgraph::Direction::Incoming)
            .map(|edge| edge.id())
    }

    fn connecting_edge_indices(
        &self,
        from: Self::NIndex,
        to: Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex> {
        self.graph.edges_connecting(from, to).map(|edge| edge.id())
    }
}

impl<NIndex: GIndex> ModifiableAutomaton for LSGGraph<NIndex> {
    fn add_node(&mut self, data: NIndex) -> Self::NIndex {
        self.graph.add_node(data)
    }

    fn add_edge(
        &mut self,
        from: Self::NIndex,
        to: Self::NIndex,
        label: CFGCounterUpdate,
    ) -> Self::EIndex {
        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!(
                    "Transition conflict, adding the new transition causes this automaton to no longer be a VASS, as VASS have to be deterministic. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}",
                    from, label, target, from, label, to
                );
            }
        }

        self.graph.add_edge(from, to, label)
    }

    fn remove_node(&mut self, node: Self::NIndex) {
        self.graph.remove_node(node);
    }

    fn remove_edge(&mut self, edge: Self::EIndex) {
        self.graph.remove_edge(edge);
    }

    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool,
    {
        for index in self.iter_node_indices().rev() {
            if !f(Frozen::from(&mut *self), index) {
                self.remove_node(index);
            }
        }
    }
}

impl<NIndex: GIndex> InitializedAutomaton for LSGGraph<NIndex> {
    fn get_initial(&self) -> Self::NIndex {
        self.start
    }

    fn set_initial(&mut self, node: Self::NIndex) {
        self.start = node;
    }

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        node == self.end
    }
}

impl<NIndex: GIndex> SingleFinalStateAutomaton for LSGGraph<NIndex> {
    fn get_final(&self) -> Self::NIndex {
        self.end
    }

    fn set_final(&mut self, node: Self::NIndex) {
        self.end = node;
    }
}

impl<NIndex: GIndex> Language for LSGGraph<NIndex> {
    fn accepts<'a>(&self, _input: impl IntoIterator<Item = &'a CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'a,
    {
        todo!()
    }
}

impl<NIndex: GIndex> CFG for LSGGraph<NIndex> {}

#[derive(Debug, Clone)]
pub struct LSGPath<NIndex: GIndex> {
    pub path: Path<NIndex, CFGCounterUpdate>,
}

impl<NIndex: GIndex> LSGPath<NIndex> {
    pub fn new(path: Path<NIndex, CFGCounterUpdate>) -> Self {
        LSGPath { path }
    }
}

impl<NIndex: GIndex> From<Path<NIndex, CFGCounterUpdate>> for LSGPath<NIndex> {
    fn from(path: Path<NIndex, CFGCounterUpdate>) -> Self {
        LSGPath::new(path)
    }
}

#[derive(Debug, Clone)]
pub enum LSGPart<NIndex: GIndex> {
    SubGraph(LSGGraph<NIndex>),
    Path(LSGPath<NIndex>),
}

impl<NIndex: GIndex> LSGPart<NIndex> {
    /// Checks if the part contains the given node.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn contains_node(&self, node: NIndex) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.graph.node_weights().contains(&node),
            LSGPart::Path(path) => path.path.contains_node(node),
        }
    }

    // Checks if the part has the given node as start or end node.
    pub fn has_node_as_extremal(&self, node: NIndex) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => {
                subgraph.cfg_start() == node || subgraph.cfg_end() == node
            }
            LSGPart::Path(path) => path.path.start() == node || path.path.end() == node,
        }
    }

    /// Iters the nodes in this part.
    /// The node indices are in the context of the CFG, not the part itself.
    pub fn iter_nodes<'a>(&'a self) -> Box<dyn Iterator<Item = NIndex> + 'a> {
        match self {
            LSGPart::SubGraph(subgraph) => Box::new(subgraph.graph.node_weights().cloned()),
            LSGPart::Path(path) => Box::new(path.path.iter_nodes()),
        }
    }

    /// Returns the start node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn start(&self) -> NIndex {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.cfg_start(),
            LSGPart::Path(path) => path.path.start(),
        }
    }

    /// Returns the end node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn end(&self) -> NIndex {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.cfg_end(),
            LSGPart::Path(path) => path.path.end(),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, LSGPart::Path(_))
    }

    pub fn is_subgraph(&self) -> bool {
        matches!(self, LSGPart::SubGraph(_))
    }

    pub fn unwrap_path(&self) -> &LSGPath<NIndex> {
        match self {
            LSGPart::Path(path) => path,
            LSGPart::SubGraph(_) => panic!("Called unwrap_path on a SubGraph part"),
        }
    }

    pub fn unwrap_subgraph(&self) -> &LSGGraph<NIndex> {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph,
            LSGPart::Path(_) => panic!("Called unwrap_subgraph on a Path part"),
        }
    }
}
