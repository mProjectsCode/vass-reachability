use itertools::Itertools;
use petgraph::{
    Direction, Graph,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};
use rand::Rng;

use crate::automaton::{
    Alphabet, Automaton, AutomatonIterators, Deterministic, ExplicitEdgeAutomaton, Frozen,
    InitializedAutomaton, Language, ModifiableAutomaton, SingleFinalStateAutomaton,
    cfg::{CFG, update::CFGCounterUpdate},
    implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath, state::MultiGraphState},
    path::Path,
};

// TODO: die sollten auch nicht determistisch sein können
#[derive(Debug, Clone)]
pub struct LSGGraph {
    pub graph: DiGraph<MultiGraphState, CFGCounterUpdate>,
    // start index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub start: NodeIndex,
    // end index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub end: NodeIndex,
    pub alphabet: Vec<CFGCounterUpdate>,
}

impl LSGGraph {
    pub fn new(
        graph: DiGraph<MultiGraphState, CFGCounterUpdate>,
        start: NodeIndex,
        end: NodeIndex,
        alphabet: Vec<CFGCounterUpdate>,
    ) -> Self {
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

        LSGGraph {
            graph,
            start,
            end,
            alphabet,
        }
    }

    /// Creates an LSGGraph from a subset of nodes of a given CFG.
    /// The start and end nodes must be part of the subset.
    /// All indices are in the context of the CFG.
    pub fn from_subset(
        product: &ImplicitCFGProduct,
        nodes: &[MultiGraphState],
        start: MultiGraphState,
        end: MultiGraphState,
    ) -> Self {
        assert!(
            nodes.contains(&start),
            "Start node {:?} must be in the subset of nodes",
            start
        );
        assert!(
            nodes.contains(&end),
            "End node {:?} must be in the subset of nodes",
            end
        );

        let mut graph: Graph<MultiGraphState, CFGCounterUpdate> = DiGraph::new();
        let mut node_map = std::collections::HashMap::new();

        // Add nodes to the LSG graph
        for state in nodes {
            let lsg_node = graph.add_node(state.clone());
            node_map.insert(state, lsg_node);
        }

        // Add edges to the LSG graph
        for state in nodes {
            let lsg_node = node_map[state];
            for letter in product.alphabet() {
                let target = state.take_letter(&product.cfgs, letter);
                if let Some(target) = target
                    && nodes.contains(&target)
                {
                    let lsg_target = node_map[&target];
                    graph.add_edge(lsg_node, lsg_target, *letter);
                }
            }
        }

        LSGGraph {
            graph,
            start: node_map[&start],
            end: node_map[&end],
            alphabet: product.alphabet().to_vec(),
        }
    }

    pub fn product_start(&self) -> &MultiGraphState {
        self.get_node_unchecked(&self.start)
    }

    pub fn product_end(&self) -> &MultiGraphState {
        self.get_node_unchecked(&self.end)
    }

    /// Maps a path in the LSG back to a path in the CFG.
    pub fn map_path_to_product(&self, path: &Path<NodeIndex, CFGCounterUpdate>) -> MultiGraphPath {
        let mut mapped_path = MultiGraphPath::new(self.map_node_to_product(*path.start()).clone());

        for (update, node) in path.iter() {
            let mapped_node = self.map_node_to_product(*node);
            mapped_path.add(*update, mapped_node.clone());
        }

        mapped_path
    }

    pub fn map_node_to_product(&self, node: NodeIndex) -> &MultiGraphState {
        &self.graph[node]
    }
}

impl Alphabet for LSGGraph {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        &self.alphabet
    }
}

impl Automaton<Deterministic> for LSGGraph {
    type NIndex = NodeIndex;
    type N = MultiGraphState;

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn get_node(&self, index: &Self::NIndex) -> Option<&MultiGraphState> {
        self.graph.node_weight(*index)
    }

    fn get_node_unchecked(&self, index: &Self::NIndex) -> &MultiGraphState {
        &self.graph[*index]
    }
}

impl ExplicitEdgeAutomaton<Deterministic> for LSGGraph {
    type EIndex = EdgeIndex;
    type E = CFGCounterUpdate;

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn get_edge(&self, index: &Self::EIndex) -> Option<&CFGCounterUpdate> {
        self.graph.edge_weight(*index)
    }

    fn get_edge_unchecked(&self, index: &Self::EIndex) -> &CFGCounterUpdate {
        self.graph.edge_weight(*index).unwrap()
    }

    fn edge_endpoints(&self, edge: &Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)> {
        self.graph.edge_endpoints(*edge)
    }

    fn edge_endpoints_unchecked(&self, edge: &Self::EIndex) -> (Self::NIndex, Self::NIndex) {
        self.graph.edge_endpoints(*edge).unwrap()
    }

    fn outgoing_edge_indices(&self, node: &Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(*node, petgraph::Direction::Outgoing)
            .map(|edge| edge.id())
    }

    fn incoming_edge_indices(&self, node: &Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(*node, petgraph::Direction::Incoming)
            .map(|edge| edge.id())
    }

    fn connecting_edge_indices(
        &self,
        from: &Self::NIndex,
        to: &Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_connecting(*from, *to)
            .map(|edge| edge.id())
    }
}

impl ModifiableAutomaton<Deterministic> for LSGGraph {
    fn add_node(&mut self, data: MultiGraphState) -> Self::NIndex {
        self.graph.add_node(data)
    }

    fn add_edge(
        &mut self,
        from: &Self::NIndex,
        to: &Self::NIndex,
        label: CFGCounterUpdate,
    ) -> Self::EIndex {
        let existing_edge = self
            .graph
            .edges_directed(*from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if &target != to {
                panic!(
                    "Transition conflict, adding the new transition causes this automaton to no longer be a VASS, as VASS have to be deterministic. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}",
                    from, label, target, from, label, to
                );
            }
        }

        self.graph.add_edge(*from, *to, label)
    }

    fn remove_node(&mut self, node: &Self::NIndex) {
        self.graph.remove_node(*node);
    }

    fn remove_edge(&mut self, edge: &Self::EIndex) {
        self.graph.remove_edge(*edge);
    }

    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool,
    {
        for index in self.iter_node_indices().rev() {
            if !f(Frozen::from(&mut *self), index) {
                self.remove_node(&index);
            }
        }
    }
}

impl InitializedAutomaton<Deterministic> for LSGGraph {
    fn get_initial(&self) -> Self::NIndex {
        self.start
    }

    fn is_accepting(&self, node: &Self::NIndex) -> bool {
        node == &self.end
    }
}

impl SingleFinalStateAutomaton<Deterministic> for LSGGraph {
    fn get_final(&self) -> Self::NIndex {
        self.end
    }

    fn set_final(&mut self, node: Self::NIndex) {
        self.end = node;
    }
}

impl Language for LSGGraph {
    fn accepts<'a>(&self, _input: impl IntoIterator<Item = &'a CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'a,
    {
        todo!()
    }
}

impl CFG for LSGGraph {}

#[derive(Debug, Clone)]
pub struct LSGPath {
    pub path: MultiGraphPath,
}

impl LSGPath {
    pub fn new(path: MultiGraphPath) -> Self {
        LSGPath { path }
    }
}

impl From<MultiGraphPath> for LSGPath {
    fn from(path: MultiGraphPath) -> Self {
        LSGPath::new(path)
    }
}

#[derive(Debug, Clone)]
pub enum LSGPart {
    SubGraph(LSGGraph),
    Path(LSGPath),
}

impl LSGPart {
    /// Checks if the part contains the given node.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn contains_node(&self, node: &MultiGraphState) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.graph.node_weights().contains(node),
            LSGPart::Path(path) => path.path.contains_state(node),
        }
    }

    // Checks if the part has the given node as start or end node.
    pub fn has_node_as_extremal(&self, node: &MultiGraphState) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => {
                subgraph.product_start() == node || subgraph.product_end() == node
            }
            LSGPart::Path(path) => path.path.start() == node || path.path.end() == node,
        }
    }

    /// Iters the nodes in this part.
    /// The node indices are in the context of the CFG, not the part itself.
    pub fn iter_nodes<'a>(&'a self) -> Box<dyn Iterator<Item = &'a MultiGraphState> + 'a> {
        match self {
            LSGPart::SubGraph(subgraph) => Box::new(subgraph.graph.node_weights()),
            LSGPart::Path(path) => Box::new(path.path.iter_states()),
        }
    }

    /// Returns the start node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn start(&self) -> &MultiGraphState {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.product_start(),
            LSGPart::Path(path) => path.path.start(),
        }
    }

    /// Returns the end node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn end(&self) -> &MultiGraphState {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.product_end(),
            LSGPart::Path(path) => path.path.end(),
        }
    }

    pub fn random_node<T: Rng>(&self, random: &mut T) -> &MultiGraphState {
        match self {
            LSGPart::SubGraph(subgraph) => {
                let node_index = random.gen_range(0..subgraph.graph.node_count());
                &subgraph.graph[NodeIndex::new(node_index)]
            }
            LSGPart::Path(path) => {
                let node_index = random.gen_range(0..path.path.state_len());
                &path.path.states[node_index]
            }
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, LSGPart::Path(_))
    }

    pub fn is_subgraph(&self) -> bool {
        matches!(self, LSGPart::SubGraph(_))
    }

    pub fn unwrap_path(&self) -> &LSGPath {
        match self {
            LSGPart::Path(path) => path,
            LSGPart::SubGraph(_) => panic!("Called unwrap_path on a SubGraph part"),
        }
    }

    pub fn unwrap_subgraph(&self) -> &LSGGraph {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph,
            LSGPart::Path(_) => panic!("Called unwrap_subgraph on a Path part"),
        }
    }
}
