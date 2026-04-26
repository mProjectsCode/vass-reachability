use std::iter::Peekable;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use petgraph::{
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use super::nfa::NFAEdge;
use crate::automaton::{
    Alphabet, Automaton, Deterministic, ExplicitEdgeAutomaton, GIndex, InitializedAutomaton,
    Language, ModifiableAutomaton, TransitionSystem,
    algorithms::{SCC, SCCAlgorithms, SCCDag},
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
    mgts::part::{MGTSPart, MarkedGraph, MarkedPath},
    nfa::NFA,
    path::Path,
};

type GenericPath<NIndex> = Path<NIndex, CFGCounterUpdate>;

pub mod extender;
pub mod part;

#[derive(Debug)]
pub struct MGTS<'a, NIndex: GIndex = MultiGraphState, A = ImplicitCFGProduct>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    /// Invariant: every referenced path or graph is used exactly once, and
    /// every stored path or graph is referenced by exactly one entry in
    /// `parts`.
    pub sequence: Vec<MGTSPart>,
    pub graphs: Vec<MarkedGraph<NIndex>>,
    pub paths: Vec<MarkedPath<NIndex>>,
    pub automaton: &'a A,
    pub dimension: usize,
}

impl<'a, NIndex: GIndex, A> Clone for MGTS<'a, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    fn clone(&self) -> Self {
        Self {
            sequence: self.sequence.clone(),
            graphs: self.graphs.clone(),
            paths: self.paths.clone(),
            automaton: self.automaton,
            dimension: self.dimension,
        }
    }
}

impl<'a, NIndex: GIndex, A> MGTS<'a, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    pub fn from_path(path: GenericPath<NIndex>, automaton: &'a A, dimension: usize) -> Self {
        let mut instance = Self::empty(automaton, dimension);
        instance.add_path(path.into());
        instance.assert_consistent();
        instance
    }

    /// The idea here is to take the subgraph of nodes that is visited in the
    /// path (including non visited edges between those nodes).
    /// Then we calculate the SCC DAG of that subgraph and build the MGTS from
    /// that.
    pub fn from_path_roll_up(
        path: GenericPath<NIndex>,
        automaton: &'a A,
        dimension: usize,
    ) -> Self {
        let visited = path.states.iter().cloned().collect::<HashSet<_>>();
        let dag = automaton
            .find_scc_dag_in_subgraph(path.start().clone(), &visited, |node| node == path.end());

        mgts_from_scc_dag_guided_path(&dag, &path, automaton, dimension)
    }

    pub fn empty(automaton: &'a A, dimension: usize) -> Self {
        MGTS {
            sequence: Vec::new(),
            graphs: Vec::new(),
            paths: Vec::new(),
            automaton,
            dimension,
        }
    }

    /// Debug-only invariant check for the internal part storage.
    ///
    /// `parts` is the source of truth. Every path or graph must be referenced
    /// by exactly one part, and every part index must be in bounds.
    #[cfg(debug_assertions)]
    pub fn assert_consistent(&self) {
        let mut used_graphs = vec![0usize; self.graphs.len()];
        let mut used_paths = vec![0usize; self.paths.len()];

        for (part_index, part) in self.sequence.iter().enumerate() {
            match part {
                MGTSPart::Graph(idx) => {
                    let Some(used) = used_graphs.get_mut(*idx) else {
                        panic!(
                            "Part {} references missing graph {} (have {})",
                            part_index,
                            idx,
                            self.graphs.len()
                        );
                    };
                    *used += 1;
                }
                MGTSPart::Path(idx) => {
                    let Some(used) = used_paths.get_mut(*idx) else {
                        panic!(
                            "Part {} references missing path {} (have {})",
                            part_index,
                            idx,
                            self.paths.len()
                        );
                    };
                    *used += 1;
                }
            }
        }

        for (index, uses) in used_graphs.iter().enumerate() {
            assert_eq!(
                *uses, 1,
                "Graph {} must be referenced exactly once, found {} uses",
                index, uses
            );
        }

        for (index, uses) in used_paths.iter().enumerate() {
            assert_eq!(
                *uses, 1,
                "Path {} must be referenced exactly once, found {} uses",
                index, uses
            );
        }

        for (index, graph) in self.graphs.iter().enumerate() {
            assert!(
                graph.graph.node_weight(graph.start).is_some(),
                "Graph {} start marker {:?} is not a live node",
                index,
                graph.start
            );
            assert!(
                graph.graph.node_weight(graph.end).is_some(),
                "Graph {} end marker {:?} is not a live node",
                index,
                graph.end
            );
        }

        for window in self.sequence.windows(2) {
            let left = &window[0];
            let right = &window[1];
            assert_eq!(
                left.end(self),
                right.start(self),
                "Adjacent MGTS parts must share a boundary"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn assert_consistent(&self) {}

    fn compact_parts_storage(&mut self) {
        let old_graphs = std::mem::take(&mut self.graphs);
        let old_paths = std::mem::take(&mut self.paths);

        let mut graph_map = HashMap::new();
        let mut path_map = HashMap::new();
        let mut graphs = Vec::with_capacity(old_graphs.len());
        let mut paths = Vec::with_capacity(old_paths.len());

        for part in &mut self.sequence {
            match *part {
                MGTSPart::Graph(old_idx) => {
                    let new_idx = *graph_map.entry(old_idx).or_insert_with(|| {
                        let Some(graph) = old_graphs.get(old_idx) else {
                            panic!("Part references missing graph {} while compacting", old_idx);
                        };

                        graphs.push(graph.clone());
                        graphs.len() - 1
                    });

                    *part = MGTSPart::Graph(new_idx);
                }
                MGTSPart::Path(old_idx) => {
                    let new_idx = *path_map.entry(old_idx).or_insert_with(|| {
                        let Some(path) = old_paths.get(old_idx) else {
                            panic!("Part references missing path {} while compacting", old_idx);
                        };

                        paths.push(path.clone());
                        paths.len() - 1
                    });

                    *part = MGTSPart::Path(new_idx);
                }
            }
        }

        self.graphs = graphs;
        self.paths = paths;
    }

    pub fn add_graph(&mut self, graph: MarkedGraph<NIndex>) {
        let index = self.graphs.len();
        self.graphs.push(graph);
        self.sequence.push(MGTSPart::Graph(index));
        self.assert_consistent();
    }

    pub fn add_path(&mut self, path: MarkedPath<NIndex>) {
        let index = self.paths.len();
        self.paths.push(path);
        self.sequence.push(MGTSPart::Path(index));
        self.assert_consistent();
    }

    pub fn graph(&self, index: usize) -> &MarkedGraph<NIndex> {
        &self.graphs[index]
    }

    pub fn path(&self, index: usize) -> &MarkedPath<NIndex> {
        &self.paths[index]
    }

    /// Adds a node from the CFG to the MGTS. The node needs to be connected to
    /// at least one node in the MGTS, otherwise the function will panic.
    /// This function will also add all existing connections between the new
    /// node and the existing MGTS nodes. This may quickly lead to large
    /// graphs and little path like structure.
    pub fn add_node(&self, node: NIndex) -> Self {
        // first we need to find all parts that contain a neighbor of the node
        // then we build a new graph containing everything between the first and last
        // neighbor then we replace all those parts with the new graph.
        // For this to work correctly, we would need to ensure that paths get split,
        // otherwise we would end up with just a single giant graph part.
        // As a simple solution, we split the paths beforehand, so that we don't have to
        // deal with the complexity of splitting paths later in this function.

        // dbg!(&self.parts);
        // dbg!(node);

        let mut result = MGTS::empty(self.automaton, self.dimension);
        let neighbors = self.automaton.undirected_neighbors(&node);

        // first we split all paths at the given node
        for part in &self.sequence {
            match part {
                MGTSPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| neighbors.contains(s)) {
                        result.add_path(split.into());
                    }
                }
                MGTSPart::Graph(idx) => {
                    result.add_graph(self.graph(*idx).clone());
                }
            }
        }

        // then we find all parts that contain a neighbor of the node
        // the second boolean in the tuple indicates whether the neighbor is at the
        // start or end of the part (true) or inside the part (false)
        let mut neighbor_parts_indices = vec![];

        for (i, part) in result.sequence.iter().enumerate() {
            for neighbor in &neighbors {
                match part {
                    MGTSPart::Graph(_) => {
                        if part.start(&result) == neighbor || part.end(&result) == neighbor {
                            neighbor_parts_indices.push((i, true));
                            break;
                        }

                        if part.contains_node(&result, neighbor) {
                            neighbor_parts_indices.push((i, false));
                            break;
                        }
                    }
                    MGTSPart::Path(_) => {
                        // since we split the paths beforehand, we only need to check the start and
                        // end nodes
                        if part.start(&result) == neighbor || part.end(&result) == neighbor {
                            neighbor_parts_indices.push((i, true));
                            break;
                        }
                    }
                }
            }
        }

        // if the list is empty, we can't add the node
        if neighbor_parts_indices.is_empty() {
            panic!("Cannot add node that is not connected to any part of the MGTS");
        }

        // thanks to the way we search for neighbors, the indices should be sorted
        let first_part = *neighbor_parts_indices.first().unwrap();
        let last_part = *neighbor_parts_indices.last().unwrap();

        // dbg!(&neighbor_parts_indices);

        let first_part_index = first_part.0 + usize::from(first_part.1);
        let last_part_index = last_part.0 - usize::from(last_part.1);

        let start_node = result.sequence[first_part_index].start(&result).clone();
        let end_node = result.sequence[last_part_index].end(&result).clone();

        let mut cut_sequence = result
            .sequence
            .drain(first_part_index..=last_part_index)
            .collect_vec();

        if cut_sequence.is_empty() {
            assert_eq!(start_node, end_node);

            cut_sequence.push(MGTSPart::Path(result.paths.len()));
            result
                .paths
                .push(GenericPath::new(start_node.clone()).into());
        }

        let mut new_graph = DiGraph::<NIndex, CFGCounterUpdate>::new();
        let mut node_map = HashMap::new();

        // add all nodes from the cut sequence to the new graph
        for part in &cut_sequence {
            for node in part.iter_nodes(&result) {
                // we may have already added this node, because start and end nodes overlap
                if node_map.contains_key(node) {
                    continue;
                }

                let new_node = new_graph.add_node(node.clone());
                node_map.insert(node.clone(), new_node);
            }
        }

        // add the new node
        let new_node = new_graph.add_node(node.clone());
        node_map.insert(node, new_node);

        // now we add all edges between the nodes in the new graph
        for (product_state, new_node) in &node_map {
            for letter in result.automaton.alphabet() {
                let Some(successor) = result.automaton.successor(product_state, letter) else {
                    continue;
                };

                if let Some(&new_target) = node_map.get(&successor) {
                    new_graph.add_edge(*new_node, new_target, *letter);
                }
            }
        }

        let new_start_node = *node_map
            .get(&start_node)
            .expect("Start node must be in the new graph");
        let new_end_node = *node_map
            .get(&end_node)
            .expect("End node must be in the new graph");

        // lastly we create the new MarkedGraph and insert it into the parts
        let graph = MarkedGraph::new(
            new_graph,
            new_start_node,
            new_end_node,
            result.automaton.alphabet().to_vec(),
        );

        let graph_index = result.graphs.len();
        result.graphs.push(graph);
        result
            .sequence
            .insert(first_part_index, MGTSPart::Graph(graph_index));

        result.compact_parts_storage();
        result.assert_consistent();

        result
    }

    /// Finds the strongly connected component around the given node in the main
    /// CFG and adds it as a graph part. The node must be contained in
    /// the MGTS, otherwise the function will panic.
    pub fn add_scc_around_node(&self, state: NIndex) -> Self {
        assert!(
            self.contains_state(&state),
            "Cannot add SCC around node that is not in the MGTS"
        );

        let scc_nodes = self.automaton.find_scc_surrounding(state.clone());
        let mut scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        // make deterministic: sort the SCC nodes before building the MGTS graph
        scc_nodes_vec.sort_unstable();
        let scc =
            MarkedGraph::from_subset(self.automaton, &scc_nodes_vec, state.clone(), state.clone());

        let mut result = MGTS::empty(self.automaton, self.dimension);
        result.graphs = self.graphs.clone();

        // first we split all paths at the given node
        for part in &self.sequence {
            match part {
                MGTSPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| s == &state) {
                        result.add_path(split.into());
                    }
                }
                MGTSPart::Graph(idx) => {
                    result.sequence.push(MGTSPart::Graph(*idx));
                }
            }
        }

        let scc_idx = result.graphs.len();
        result.graphs.push(scc);

        let parts = std::mem::take(&mut result.sequence);
        result.sequence = parts
            .into_iter()
            .flat_map(|part| {
                if part.end(&result) == &state {
                    vec![part, MGTSPart::Graph(scc_idx)]
                } else {
                    vec![part]
                }
            })
            .collect();

        result.assert_consistent();

        result
    }

    pub fn add_scc_around_position(&self, path_index: usize, node_index: usize) -> Self {
        let MGTSPart::Path(path_idx) = self.sequence[path_index] else {
            panic!("Part must be a path");
        };
        let state = &self.paths[path_idx].path.states[node_index];

        let scc_nodes = self.automaton.find_scc_surrounding(state.clone());
        let mut scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        // make deterministic: sort the SCC nodes before building the MGTS graph
        scc_nodes_vec.sort_unstable();
        let scc =
            MarkedGraph::from_subset(self.automaton, &scc_nodes_vec, state.clone(), state.clone());

        let mut result = MGTS::empty(self.automaton, self.dimension);

        for (i, part) in self.sequence.iter().enumerate() {
            if i == path_index {
                if node_index == 0 {
                    result.add_graph(scc.clone());
                    result.add_path(self.paths[path_idx].clone());
                } else if node_index == self.paths[path_idx].path.states.len() - 1 {
                    result.add_path(self.paths[path_idx].clone());
                    result.add_graph(scc.clone());
                } else {
                    let mut path = self.paths[path_idx].clone();
                    let after = path.path.split_off(node_index);

                    result.add_path(path);
                    result.add_graph(scc.clone());
                    result.add_path(after.into());
                }
            } else {
                match part {
                    MGTSPart::Path(idx) => result.add_path(self.paths[*idx].clone()),
                    MGTSPart::Graph(idx) => result.add_graph(self.graphs[*idx].clone()),
                }
            }
        }

        result.assert_consistent();

        result
    }

    /// Removes the given node from the graph. The node must be in the graph,
    /// otherwise the function will panic.
    pub fn remove_node_from_graph(&mut self, graph_index: usize, node_index: NodeIndex) {
        let graph = &mut self.graphs[graph_index];
        if graph.node_count() <= node_index.index() {
            panic!("Node is not in the graph");
        }

        graph.remove_node(&node_index);
        self.assert_consistent();
    }

    /// Removes the given edge from the graph. The edge must be in the graph,
    /// otherwise the function will panic.
    pub fn remove_edge_from_graph(&mut self, graph_index: usize, edge_index: EdgeIndex) {
        let graph = &mut self.graphs[graph_index];
        if graph.edge_count() <= edge_index.index() {
            panic!("Edge is not in the graph");
        }

        graph.remove_edge(&edge_index);
        self.assert_consistent();
    }

    /// Removes all nodes from the graph except the given ones.
    pub fn restrict_graph_to_subset(
        &mut self,
        graph_index: usize,
        nodes_to_keep: HashSet<NodeIndex>,
    ) {
        let graph = &mut self.graphs[graph_index];
        graph.retain_nodes(|_, node| nodes_to_keep.contains(&node));
        self.assert_consistent();
    }

    /// Checks if the MGTS contains the given state from the product.
    pub fn contains_state(&self, state: &NIndex) -> bool {
        for part in &self.sequence {
            if part.contains_node(self, state) {
                return true;
            }
        }

        false
    }

    pub fn size(&self) -> usize {
        self.sequence.iter().map(|part| part.size(self)).sum()
    }

    /// Converts the MGTS into an NFA over CFGCounterUpdate.
    pub fn to_nfa(&self) -> NFA<(), CFGCounterUpdate> {
        self.assert_consistent();

        let mut nfa: NFA<(), CFGCounterUpdate> =
            NFA::new(CFGCounterUpdate::alphabet(self.dimension));
        let start_state = nfa.add_node(DfaNode::non_accepting(()));
        nfa.set_initial(start_state);

        let mut prev_state = start_state;

        for part in &self.sequence {
            match part {
                MGTSPart::Path(idx) => {
                    let path = self.path(*idx);
                    for update in &path.path.transitions {
                        let next_state = nfa.add_node(DfaNode::non_accepting(()));

                        nfa.add_edge(&prev_state, &next_state, NFAEdge::Symbol(*update));
                        prev_state = next_state;
                    }
                }
                MGTSPart::Graph(idx) => {
                    let graph = self.graph(*idx);
                    // compute base index in the NFA for the first node of this graph
                    let base = nfa.graph.node_count() as u32;

                    // first add all states (they will get indices base..)
                    for _ in graph.graph.node_indices() {
                        nfa.add_node(DfaNode::non_accepting(()));
                    }

                    // then connect the previous part to the start of the graph
                    for i in graph.graph.node_indices() {
                        if i == graph.start {
                            let end_index = base + i.index() as u32;
                            nfa.add_edge(
                                &prev_state,
                                &NodeIndex::from(end_index),
                                NFAEdge::Epsilon,
                            );
                        }
                    }

                    // then set the prev_state to the end of the graph
                    for i in graph.graph.node_indices() {
                        if i == graph.end {
                            let end_index = base + i.index() as u32;
                            prev_state = end_index.into();
                        }
                    }

                    // add all edges (map graph node indices -> NFA indices using base)
                    for edge_ref in graph.graph.edge_references() {
                        let src = NodeIndex::from(base + edge_ref.source().index() as u32);
                        let dst = NodeIndex::from(base + edge_ref.target().index() as u32);
                        let weight = *edge_ref.weight();

                        nfa.add_edge(&src, &dst, NFAEdge::Symbol(weight));
                    }
                }
            }
        }

        nfa.set_accepting(prev_state);

        nfa
    }

    pub fn to_cfg(&self) -> VASSCFG<()> {
        tracing::debug!("Converting MGTS to NFA");
        let nfa = self.to_nfa();

        tracing::debug!(
            "Converting NFA with {} states and {} edges to CFG",
            nfa.graph.node_count(),
            nfa.graph.edge_count()
        );
        nfa.determinize()
    }

    pub fn iter_parts<'b>(&'b self) -> impl Iterator<Item = &'b MGTSPart> + 'b {
        self.sequence.iter()
    }

    pub fn iter_path_parts<'b>(&'b self) -> impl Iterator<Item = &'b MarkedPath<NIndex>> + 'b {
        self.sequence.iter().filter_map(|part| match part {
            MGTSPart::Path(idx) => Some(self.path(*idx)),
            MGTSPart::Graph(_) => None,
        })
    }

    pub fn iter_graph_parts<'b>(&'b self) -> impl Iterator<Item = &'b MarkedGraph<NIndex>> + 'b {
        self.sequence.iter().filter_map(|part| match part {
            MGTSPart::Graph(idx) => Some(self.graph(*idx)),
            MGTSPart::Path(_) => None,
        })
    }
}

fn mgts_from_scc_dag_guided_path<'a, NIndex: GIndex, A>(
    dag: &SCCDag<NIndex, CFGCounterUpdate>,
    path: &GenericPath<NIndex>,
    automaton: &'a A,
    dimension: usize,
) -> MGTS<'a, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    let component_of_state = dag
        .components
        .iter()
        .enumerate()
        .flat_map(|(component, scc)| scc.nodes.iter().cloned().map(move |node| (node, component)))
        .collect::<HashMap<_, _>>();

    for state in &path.states {
        assert!(
            component_of_state.contains_key(state),
            "Path state {:?} is not part of the SCC DAG subgraph",
            state
        );
    }

    let mut mgts = MGTS::empty(automaton, dimension);
    let mut current_path = GenericPath::new(path.start().clone());
    let mut state_index = 0usize;

    while state_index + 1 < path.states.len() {
        let component = component_of_state[&path.states[state_index]];
        let scc = &dag.components[component];

        let mut run_end = state_index;
        while run_end + 1 < path.states.len()
            && component_of_state[&path.states[run_end + 1]] == component
        {
            run_end += 1;
        }

        if scc.is_trivial() {
            for edge_index in state_index..run_end {
                current_path.add(
                    path.transitions[edge_index],
                    path.states[edge_index + 1].clone(),
                );
            }
        } else {
            if !current_path.is_empty() {
                mgts.add_path(current_path.clone().into());
            }

            mgts.add_graph(marked_graph_from_scc(
                scc,
                &path.states[state_index],
                &path.states[run_end],
                automaton,
            ));

            current_path = GenericPath::new(path.states[run_end].clone());
        }

        if run_end < path.transitions.len() {
            current_path.add(path.transitions[run_end], path.states[run_end + 1].clone());
        }

        state_index = run_end + 1;
    }

    if !current_path.is_empty() {
        mgts.add_path(current_path.into());
    }

    assert!(
        mgts.accepts(path.transitions.iter()),
        "Path-guided SCC roll-up must accept the original path"
    );

    mgts
}

fn marked_graph_from_scc<NIndex: GIndex, A>(
    scc: &SCC<NIndex>,
    start: &NIndex,
    end: &NIndex,
    automaton: &A,
) -> MarkedGraph<NIndex>
where
    A: TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + Alphabet<Letter = CFGCounterUpdate>,
{
    // Keep node order deterministic so repeated runs over the same SCC-DAG
    // produce structurally stable MGTS graph parts.
    let mut nodes = scc.nodes.clone();
    nodes.sort_unstable();
    MarkedGraph::from_subset(automaton, &nodes, start.clone(), end.clone())
}

fn partial_accept_path<'a, NIndex: GIndex>(
    path: &MarkedPath<NIndex>,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut index = 0;

    if path.path.is_empty() {
        return true;
    }

    while let Some(symbol) = input.peek() {
        let update = path.path.transitions[index];

        if update == **symbol {
            index += 1;
            input.next();
        } else {
            return false;
        }

        if index == path.path.len() {
            return true;
        }
    }

    index == path.path.len()
}

fn partial_accept_graph<'a, NIndex: GIndex>(
    graph: &MarkedGraph<NIndex>,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut current_state = graph.start;

    while let Some(symbol) = input.peek() {
        let mut found_next_state = false;
        for edge_ref in graph
            .graph
            .edges_directed(current_state, petgraph::Direction::Outgoing)
        {
            if edge_ref.weight() == *symbol {
                current_state = edge_ref.target();
                found_next_state = true;
                input.next();
                break;
            }
        }

        if !found_next_state {
            break;
        }
    }

    current_state == graph.end
}

impl<'a, NIndex: GIndex, A> Alphabet for MGTS<'a, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        self.automaton.alphabet()
    }
}

impl<'a, NIndex: GIndex, A> Language for MGTS<'a, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>,
{
    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        self.assert_consistent();

        let mut input = input.into_iter().peekable();
        for part in self.sequence.iter() {
            let success = match part {
                MGTSPart::Path(idx) => partial_accept_path(self.path(*idx), &mut input),
                MGTSPart::Graph(idx) => partial_accept_graph(self.graph(*idx), &mut input),
            };

            if !success {
                return false;
            }
        }

        // lastly we need to check that we are at the end of the input
        input.next().is_none()
    }
}
