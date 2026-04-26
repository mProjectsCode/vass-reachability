use hashbrown::HashMap;
use petgraph::{
    Direction, Graph,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};
use rand::{Rng, RngExt};

use crate::automaton::{
    Alphabet, Automaton, AutomatonIterators, Deterministic, ExplicitEdgeAutomaton, Frozen, GIndex,
    InitializedAutomaton, Language, ModifiableAutomaton, SingleFinalStateAutomaton,
    TransitionSystem,
    cfg::{CFG, update::CFGCounterUpdate},
    mgts::MGTS,
    path::Path,
};

type GenericPath<NIndex> = Path<NIndex, CFGCounterUpdate>;

// TODO: die sollten auch nicht determistisch sein können
#[derive(Debug, Clone)]
pub struct MarkedGraph<NIndex: GIndex> {
    pub graph: DiGraph<NIndex, CFGCounterUpdate>,
    // start index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub start: NodeIndex,
    // end index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub end: NodeIndex,
    pub alphabet: Vec<CFGCounterUpdate>,
}

impl<NIndex: GIndex> MarkedGraph<NIndex> {
    pub fn new(
        graph: DiGraph<NIndex, CFGCounterUpdate>,
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

        MarkedGraph {
            graph,
            start,
            end,
            alphabet,
        }
    }

    /// Creates an MarkedGraph from a subset of nodes of a given CFG.
    /// The start and end nodes must be part of the subset.
    /// All indices are in the context of the CFG.
    pub fn from_subset<A>(automaton: &A, nodes: &[NIndex], start: NIndex, end: NIndex) -> Self
    where
        A: TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + Alphabet<Letter = CFGCounterUpdate>,
    {
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

        let mut graph: Graph<NIndex, CFGCounterUpdate> = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add nodes to the MGTS graph
        for state in nodes {
            let g_node = graph.add_node(state.clone());
            node_map.insert(state, g_node);
        }

        // Add edges to the MGTS graph
        for state in nodes {
            let g_node = node_map[state];
            for letter in automaton.alphabet() {
                if let Some(target) = automaton.successor(state, letter)
                    && nodes.contains(&target)
                {
                    let g_target = node_map[&target];
                    graph.add_edge(g_node, g_target, *letter);
                }
            }
        }

        MarkedGraph {
            graph,
            start: node_map[&start],
            end: node_map[&end],
            alphabet: automaton.alphabet().to_vec(),
        }
    }

    pub fn product_start(&self) -> &NIndex {
        self.get_node_unchecked(&self.start)
    }

    pub fn product_end(&self) -> &NIndex {
        self.get_node_unchecked(&self.end)
    }

    /// Maps a path in the MGTS back to a path in the CFG.
    pub fn map_path_to_product(
        &self,
        path: &Path<NodeIndex, CFGCounterUpdate>,
    ) -> GenericPath<NIndex> {
        let mut mapped_path = GenericPath::new(self.map_node_to_product(*path.start()).clone());

        for (update, node) in path.iter() {
            let mapped_node = self.map_node_to_product(*node);
            mapped_path.add(*update, mapped_node.clone());
        }

        mapped_path
    }

    pub fn map_node_to_product(&self, node: NodeIndex) -> &NIndex {
        &self.graph[node]
    }
}

impl<NIndex: GIndex> Alphabet for MarkedGraph<NIndex> {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        &self.alphabet
    }
}

impl<NIndex: GIndex> Automaton<Deterministic> for MarkedGraph<NIndex> {
    type NIndex = NodeIndex;
    type N = NIndex;

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn get_node(&self, index: &Self::NIndex) -> Option<&NIndex> {
        self.graph.node_weight(*index)
    }

    fn get_node_unchecked(&self, index: &Self::NIndex) -> &NIndex {
        &self.graph[*index]
    }
}

impl<NIndex: GIndex> ExplicitEdgeAutomaton<Deterministic> for MarkedGraph<NIndex> {
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

    fn outgoing_edge_indices<'a>(
        &'a self,
        node: &Self::NIndex,
    ) -> Box<dyn Iterator<Item = Self::EIndex> + 'a> {
        let node = *node;

        Box::new(
            self.graph
                .edges_directed(node, petgraph::Direction::Outgoing)
                .map(|edge| edge.id()),
        )
    }

    fn incoming_edge_indices<'a>(
        &'a self,
        node: &Self::NIndex,
    ) -> Box<dyn Iterator<Item = Self::EIndex> + 'a> {
        let node = *node;

        Box::new(
            self.graph
                .edges_directed(node, petgraph::Direction::Incoming)
                .map(|edge| edge.id()),
        )
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

impl<NIndex: GIndex> ModifiableAutomaton<Deterministic> for MarkedGraph<NIndex> {
    fn add_node(&mut self, data: NIndex) -> Self::NIndex {
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

impl<NIndex: GIndex> InitializedAutomaton<Deterministic> for MarkedGraph<NIndex> {
    fn get_initial(&self) -> Self::NIndex {
        self.start
    }

    fn is_accepting(&self, node: &Self::NIndex) -> bool {
        node == &self.end
    }
}

impl<NIndex: GIndex> SingleFinalStateAutomaton<Deterministic> for MarkedGraph<NIndex> {
    fn get_final(&self) -> Self::NIndex {
        self.end
    }

    fn set_final(&mut self, node: Self::NIndex) {
        self.end = node;
    }
}

impl<NIndex: GIndex> Language for MarkedGraph<NIndex> {
    fn accepts<'a>(&self, _input: impl IntoIterator<Item = &'a CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'a,
    {
        todo!()
    }
}

impl<NIndex: GIndex> CFG for MarkedGraph<NIndex> {}

#[derive(Debug, Clone)]
pub struct MarkedPath<NIndex: GIndex> {
    pub path: GenericPath<NIndex>,
}

impl<NIndex: GIndex> MarkedPath<NIndex> {
    pub fn new(path: GenericPath<NIndex>) -> Self {
        MarkedPath { path }
    }
}

impl<NIndex: GIndex> From<GenericPath<NIndex>> for MarkedPath<NIndex> {
    fn from(path: GenericPath<NIndex>) -> Self {
        MarkedPath::new(path)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum MGTSPart {
    Graph(usize),
    Path(usize),
}

impl MGTSPart {
    /// Checks if the part contains the given node.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn contains_node<'a, NIndex: GIndex, A>(
        &self,
        mgts: &MGTS<'a, NIndex, A>,
        node: &NIndex,
    ) -> bool
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => mgts.graph(*i).graph.node_weights().any(|n| n == node),
            MGTSPart::Path(i) => mgts.path(*i).path.contains_state(node),
        }
    }

    // Checks if the part has the given node as start or end node.
    pub fn has_node_as_extremal<'a, NIndex: GIndex, A>(
        &self,
        mgts: &MGTS<'a, NIndex, A>,
        node: &NIndex,
    ) -> bool
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => {
                mgts.graph(*i).product_start() == node || mgts.graph(*i).product_end() == node
            }
            MGTSPart::Path(i) => {
                mgts.path(*i).path.start() == node || mgts.path(*i).path.end() == node
            }
        }
    }

    /// Iters the nodes in this part.
    /// The node indices are in the context of the CFG, not the part itself.
    pub fn iter_nodes<'a, NIndex: GIndex, A>(
        &'a self,
        mgts: &'a MGTS<'a, NIndex, A>,
    ) -> Box<dyn Iterator<Item = &'a NIndex> + 'a>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => Box::new(mgts.graph(*i).graph.node_weights()),
            MGTSPart::Path(i) => Box::new(mgts.path(*i).path.iter_states()),
        }
    }

    /// Returns the start node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn start<'a, NIndex: GIndex, A>(&self, mgts: &'a MGTS<'a, NIndex, A>) -> &'a NIndex
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => mgts.graph(*i).product_start(),
            MGTSPart::Path(i) => mgts.path(*i).path.start(),
        }
    }

    /// Returns the end node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn end<'a, NIndex: GIndex, A>(&self, mgts: &'a MGTS<'a, NIndex, A>) -> &'a NIndex
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => mgts.graph(*i).product_end(),
            MGTSPart::Path(i) => mgts.path(*i).path.end(),
        }
    }

    pub fn random_node<'a, NIndex: GIndex, A, T: Rng>(
        &self,
        mgts: &'a MGTS<'a, NIndex, A>,
        random: &mut T,
    ) -> &'a NIndex
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => {
                let graph = mgts.graph(*i);
                let node_index = random.random_range(0..graph.graph.node_count());
                &graph.graph[NodeIndex::new(node_index)]
            }
            MGTSPart::Path(i) => {
                let path = mgts.path(*i);
                let node_index = random.random_range(0..path.path.state_len());
                &path.path.states[node_index]
            }
        }
    }

    pub fn size<'a, NIndex: GIndex, A>(&self, mgts: &MGTS<'a, NIndex, A>) -> usize
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => mgts.graph(*i).graph.node_count(),
            MGTSPart::Path(i) => mgts.path(*i).path.state_len(),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, MGTSPart::Path(_))
    }

    pub fn is_graph(&self) -> bool {
        matches!(self, MGTSPart::Graph(_))
    }

    pub fn as_path<'a, NIndex: GIndex, A>(
        &self,
        mgts: &'a MGTS<'a, NIndex, A>,
    ) -> Option<&'a MarkedPath<NIndex>>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Path(i) => Some(mgts.path(*i)),
            MGTSPart::Graph(_) => None,
        }
    }

    pub fn as_graph<'a, NIndex: GIndex, A>(
        &self,
        mgts: &'a MGTS<'a, NIndex, A>,
    ) -> Option<&'a MarkedGraph<NIndex>>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => Some(mgts.graph(*i)),
            MGTSPart::Path(_) => None,
        }
    }

    pub fn unwrap_path<'a, NIndex: GIndex, A>(
        &self,
        mgts: &'a MGTS<'a, NIndex, A>,
    ) -> &'a MarkedPath<NIndex>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Path(i) => mgts.path(*i),
            MGTSPart::Graph(_) => panic!("Called unwrap_path on a Graph part"),
        }
    }

    pub fn unwrap_graph<'a, NIndex: GIndex, A>(
        &self,
        mgts: &'a MGTS<'a, NIndex, A>,
    ) -> &'a MarkedGraph<NIndex>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + crate::automaton::scc::SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        match self {
            MGTSPart::Graph(i) => mgts.graph(*i),
            MGTSPart::Path(_) => panic!("Called unwrap_graph on a Path part"),
        }
    }
}
