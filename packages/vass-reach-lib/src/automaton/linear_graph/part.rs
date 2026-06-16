use hashbrown::{HashMap, HashSet};
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
    linear_graph::{LinearGraph, LinearGraphAutomaton},
    path::Path,
};

type GenericPath<NIndex> = Path<NIndex, CFGCounterUpdate>;

// Linear graph regions are deterministic because they are induced from
// deterministic product transitions.
#[derive(Debug, Clone)]
pub struct LinearGraphRegion<NIndex: GIndex> {
    pub graph: DiGraph<NIndex, CFGCounterUpdate>,
    // start index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub start: NodeIndex,
    // end index in the graph, this index refers to the node in the StableDiGraph, not the CFG
    pub end: NodeIndex,
    pub alphabet: Vec<CFGCounterUpdate>,
}

impl<NIndex: GIndex> LinearGraphRegion<NIndex> {
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

        LinearGraphRegion {
            graph,
            start,
            end,
            alphabet,
        }
    }

    /// Creates a LinearGraphRegion from a subset of nodes of a given CFG.
    /// The start and end nodes must be part of the subset.
    /// All indices are in the context of the CFG.
    pub fn from_subset<A>(automaton: &A, nodes: &[NIndex], start: NIndex, end: NIndex) -> Self
    where
        A: TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        let node_set = nodes.iter().cloned().collect::<HashSet<_>>();

        assert!(
            node_set.contains(&start),
            "Start node {:?} must be in the subset of nodes",
            start
        );
        assert!(
            node_set.contains(&end),
            "End node {:?} must be in the subset of nodes",
            end
        );

        let mut graph: Graph<NIndex, CFGCounterUpdate> = DiGraph::new();
        let mut node_map = HashMap::new();

        // Add nodes to the region graph.
        for state in nodes {
            let g_node = graph.add_node(state.clone());
            node_map.insert(state, g_node);
        }

        // Add edges to the region graph.
        for state in nodes {
            let g_node = node_map[state];
            for letter in automaton.alphabet() {
                if let Some(target) = automaton.successor(state, letter)
                    && node_set.contains(&target)
                {
                    let g_target = node_map[&target];
                    graph.add_edge(g_node, g_target, *letter);
                }
            }
        }

        LinearGraphRegion {
            graph,
            start: node_map[&start],
            end: node_map[&end],
            alphabet: automaton.alphabet().to_vec(),
        }
    }

    /// Creates a linear graph region from the union of all states visited by
    /// `paths`.
    ///
    /// The first path supplies the product start and end boundaries. Callers
    /// should only pass paths that share the same boundary states.
    pub fn from_path_union<A>(automaton: &A, paths: &[GenericPath<NIndex>]) -> Self
    where
        A: TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        assert!(
            !paths.is_empty(),
            "Cannot build a linear graph region from an empty path set"
        );

        let nodes = Path::sorted_union_states(paths);

        Self::from_subset(
            automaton,
            &nodes,
            paths[0].start().clone(),
            paths[0].end().clone(),
        )
    }

    pub fn product_start(&self) -> &NIndex {
        self.get_node_unchecked(&self.start)
    }

    pub fn product_end(&self) -> &NIndex {
        self.get_node_unchecked(&self.end)
    }

    /// Maps a path in the LinearGraph back to a path in the CFG.
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

impl<NIndex: GIndex> Alphabet for LinearGraphRegion<NIndex> {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        &self.alphabet
    }
}

impl<NIndex: GIndex> Automaton<Deterministic> for LinearGraphRegion<NIndex> {
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

impl<NIndex: GIndex> ExplicitEdgeAutomaton<Deterministic> for LinearGraphRegion<NIndex> {
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

impl<NIndex: GIndex> ModifiableAutomaton<Deterministic> for LinearGraphRegion<NIndex> {
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

impl<NIndex: GIndex> InitializedAutomaton<Deterministic> for LinearGraphRegion<NIndex> {
    fn get_initial(&self) -> Self::NIndex {
        self.start
    }

    fn is_accepting(&self, node: &Self::NIndex) -> bool {
        node == &self.end
    }
}

impl<NIndex: GIndex> SingleFinalStateAutomaton<Deterministic> for LinearGraphRegion<NIndex> {
    fn get_final(&self) -> Self::NIndex {
        self.end
    }

    fn set_final(&mut self, node: Self::NIndex) {
        self.end = node;
    }
}

impl<NIndex: GIndex> Language for LinearGraphRegion<NIndex> {
    fn accepts<'a>(&self, _input: impl IntoIterator<Item = &'a CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'a,
    {
        todo!()
    }
}

impl<NIndex: GIndex> CFG for LinearGraphRegion<NIndex> {}

#[derive(Debug, Clone)]
pub struct LinearGraphPathSegment<NIndex: GIndex> {
    pub path: GenericPath<NIndex>,
}

impl<NIndex: GIndex> LinearGraphPathSegment<NIndex> {
    pub fn new(path: GenericPath<NIndex>) -> Self {
        LinearGraphPathSegment { path }
    }
}

impl<NIndex: GIndex> From<GenericPath<NIndex>> for LinearGraphPathSegment<NIndex> {
    fn from(path: GenericPath<NIndex>) -> Self {
        LinearGraphPathSegment::new(path)
    }
}

#[derive(Debug, Clone)]
pub struct LinearGraphRepeatPath<NIndex: GIndex> {
    pub path: GenericPath<NIndex>,
}

impl<NIndex: GIndex> LinearGraphRepeatPath<NIndex> {
    pub fn new(path: GenericPath<NIndex>) -> Self {
        assert!(!path.is_empty(), "Repeated paths must be non-empty");
        assert_eq!(
            path.start(),
            path.end(),
            "Repeated paths must start and end in the same state"
        );

        Self { path }
    }
}

impl<NIndex: GIndex> From<GenericPath<NIndex>> for LinearGraphRepeatPath<NIndex> {
    fn from(path: GenericPath<NIndex>) -> Self {
        Self::new(path)
    }
}

#[derive(Debug, Copy, Clone)]
pub enum LinearGraphPart {
    Graph(usize),
    Path(usize),
    RepeatPath(usize),
}

impl LinearGraphPart {
    /// Checks if the part contains the given node.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn contains_node<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &LinearGraph<'a, NIndex, A>,
        node: &NIndex,
    ) -> bool
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => linear_graph
                .graph(*i)
                .graph
                .node_weights()
                .any(|n| n == node),
            LinearGraphPart::Path(i) => linear_graph.path(*i).path.contains_state(node),
            LinearGraphPart::RepeatPath(i) => {
                linear_graph.repeat_path(*i).path.contains_state(node)
            }
        }
    }

    // Checks if the part has the given node as start or end node.
    pub fn has_node_as_extremal<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &LinearGraph<'a, NIndex, A>,
        node: &NIndex,
    ) -> bool
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => {
                linear_graph.graph(*i).product_start() == node
                    || linear_graph.graph(*i).product_end() == node
            }
            LinearGraphPart::Path(i) => {
                linear_graph.path(*i).path.start() == node
                    || linear_graph.path(*i).path.end() == node
            }
            LinearGraphPart::RepeatPath(i) => linear_graph.repeat_path(*i).path.start() == node,
        }
    }

    /// Iters the nodes in this part.
    /// The node indices are in the context of the CFG, not the part itself.
    pub fn iter_nodes<'a, NIndex: GIndex, A>(
        &'a self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> Box<dyn Iterator<Item = &'a NIndex> + 'a>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => Box::new(linear_graph.graph(*i).graph.node_weights()),
            LinearGraphPart::Path(i) => Box::new(linear_graph.path(*i).path.iter_states()),
            LinearGraphPart::RepeatPath(i) => {
                Box::new(linear_graph.repeat_path(*i).path.iter_states())
            }
        }
    }

    /// Returns the start node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn start<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> &'a NIndex
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => linear_graph.graph(*i).product_start(),
            LinearGraphPart::Path(i) => linear_graph.path(*i).path.start(),
            LinearGraphPart::RepeatPath(i) => linear_graph.repeat_path(*i).path.start(),
        }
    }

    /// Returns the end node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn end<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> &'a NIndex
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => linear_graph.graph(*i).product_end(),
            LinearGraphPart::Path(i) => linear_graph.path(*i).path.end(),
            LinearGraphPart::RepeatPath(i) => linear_graph.repeat_path(*i).path.end(),
        }
    }

    pub fn random_node<'a, NIndex: GIndex, A, T: Rng>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
        random: &mut T,
    ) -> &'a NIndex
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => {
                let graph = linear_graph.graph(*i);
                let node_index = random.random_range(0..graph.graph.node_count());
                &graph.graph[NodeIndex::new(node_index)]
            }
            LinearGraphPart::Path(i) => {
                let path = linear_graph.path(*i);
                let node_index = random.random_range(0..path.path.state_len());
                &path.path.states[node_index]
            }
            LinearGraphPart::RepeatPath(i) => {
                let path = linear_graph.repeat_path(*i);
                let node_index = random.random_range(0..path.path.state_len());
                &path.path.states[node_index]
            }
        }
    }

    pub fn size<'a, NIndex: GIndex, A>(&self, linear_graph: &LinearGraph<'a, NIndex, A>) -> usize
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => linear_graph.graph(*i).graph.node_count(),
            LinearGraphPart::Path(i) => linear_graph.path(*i).path.state_len(),
            LinearGraphPart::RepeatPath(i) => linear_graph.repeat_path(*i).path.state_len(),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, LinearGraphPart::Path(_))
    }

    pub fn is_graph(&self) -> bool {
        matches!(self, LinearGraphPart::Graph(_))
    }

    pub fn is_repeat_path(&self) -> bool {
        matches!(self, LinearGraphPart::RepeatPath(_))
    }

    pub fn as_path<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> Option<&'a LinearGraphPathSegment<NIndex>>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Path(i) => Some(linear_graph.path(*i)),
            LinearGraphPart::Graph(_) | LinearGraphPart::RepeatPath(_) => None,
        }
    }

    pub fn as_graph<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> Option<&'a LinearGraphRegion<NIndex>>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => Some(linear_graph.graph(*i)),
            LinearGraphPart::Path(_) | LinearGraphPart::RepeatPath(_) => None,
        }
    }

    pub fn unwrap_path<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> &'a LinearGraphPathSegment<NIndex>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Path(i) => linear_graph.path(*i),
            LinearGraphPart::Graph(_) | LinearGraphPart::RepeatPath(_) => {
                panic!("Called unwrap_path on a non-Path part")
            }
        }
    }

    pub fn unwrap_graph<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &'a LinearGraph<'a, NIndex, A>,
    ) -> &'a LinearGraphRegion<NIndex>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        match self {
            LinearGraphPart::Graph(i) => linear_graph.graph(*i),
            LinearGraphPart::Path(_) | LinearGraphPart::RepeatPath(_) => {
                panic!("Called unwrap_graph on a non-Graph part")
            }
        }
    }
}
