use std::sync::Arc;

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
    linear_graph::part::{
        LinearGraphPart, LinearGraphPathSegment, LinearGraphRegion, LinearGraphRepeatPath,
    },
    nfa::NFA,
    path::Path,
};

type CFGPath<NIndex> = Path<NIndex, CFGCounterUpdate>;

pub mod extender;
pub mod part;
pub mod rooted;

pub use rooted::{RootedLinearGraph, RootedLinearGraphError};

#[derive(Debug, Clone, Copy)]
enum NeighborPart {
    Boundary(usize),
    Interior(usize),
}

impl NeighborPart {
    fn first_replaced_part(self) -> usize {
        match self {
            NeighborPart::Boundary(index) => index + 1,
            NeighborPart::Interior(index) => index,
        }
    }

    fn last_replaced_part(self) -> usize {
        match self {
            NeighborPart::Boundary(index) => index - 1,
            NeighborPart::Interior(index) => index,
        }
    }
}

pub trait LinearGraphAutomaton<NIndex: GIndex>:
    InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
    + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
    + SCCAlgorithms
    + Alphabet<Letter = CFGCounterUpdate>
{
}

impl<NIndex: GIndex, A> LinearGraphAutomaton<NIndex> for A where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>
{
}

#[derive(Debug)]
pub struct LinearGraph<'a, NIndex: GIndex = MultiGraphState, A = ImplicitCFGProduct>
where
    A: LinearGraphAutomaton<NIndex>,
{
    /// Invariant: every referenced path or graph is used exactly once, and
    /// every stored path or graph is referenced by exactly one entry in
    /// `parts`.
    pub sequence: Vec<LinearGraphPart>,
    pub graphs: Vec<Arc<LinearGraphRegion<NIndex>>>,
    pub paths: Vec<LinearGraphPathSegment<NIndex>>,
    pub repeat_paths: Vec<LinearGraphRepeatPath<NIndex>>,
    pub automaton: &'a A,
    pub dimension: usize,
}

impl<'a, NIndex: GIndex, A> Clone for LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    fn clone(&self) -> Self {
        Self {
            sequence: self.sequence.clone(),
            graphs: self.graphs.clone(),
            paths: self.paths.clone(),
            repeat_paths: self.repeat_paths.clone(),
            automaton: self.automaton,
            dimension: self.dimension,
        }
    }
}

impl<'a, NIndex: GIndex, A> LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    pub fn from_path(path: CFGPath<NIndex>, automaton: &'a A, dimension: usize) -> Self {
        let mut instance = Self::empty(automaton, dimension);
        instance.add_path(path.into());
        instance.assert_consistent();
        instance
    }

    pub fn from_path_with_repeat_at(
        path: CFGPath<NIndex>,
        repeated: CFGPath<NIndex>,
        repeat_position: usize,
        automaton: &'a A,
        dimension: usize,
    ) -> Self {
        Self::from_path_with_repeats_at(
            path,
            vec![(repeat_position, repeated)],
            automaton,
            dimension,
        )
    }

    pub fn from_path_with_repeats_at(
        path: CFGPath<NIndex>,
        mut repeated: Vec<(usize, CFGPath<NIndex>)>,
        automaton: &'a A,
        dimension: usize,
    ) -> Self {
        repeated.sort_unstable_by_key(|(position, _)| *position);
        for (position, repeated_path) in &repeated {
            assert!(
                *position <= path.len(),
                "Repeat position must be a path-state index"
            );
            assert_eq!(
                &path.states[*position],
                repeated_path.start(),
                "Repeated path must be rooted at the selected path state"
            );
        }
        assert!(
            repeated.windows(2).all(|window| window[0].0 != window[1].0),
            "At most one repeated path may be attached to each path state"
        );

        let mut instance = Self::empty(automaton, dimension);
        let mut repeats = repeated.into_iter().peekable();

        for position in 0..=path.len() {
            if repeats
                .peek()
                .is_some_and(|(repeat_at, _)| *repeat_at == position)
            {
                let (_, repeated_path) = repeats.next().expect("peeked repeated path must exist");
                instance.add_repeat_path(repeated_path.into());
            }

            if position < path.len() {
                instance.add_path(path.slice(position..position + 1).into());
            }
        }

        instance.assert_consistent();
        instance
    }

    /// The idea here is to take the subgraph of nodes that is visited in the
    /// path (including non visited edges between those nodes).
    /// Then we calculate the SCC DAG of that subgraph and build the LinearGraph
    /// from that.
    pub fn from_path_roll_up(path: CFGPath<NIndex>, automaton: &'a A, dimension: usize) -> Self {
        let visited = path.states.iter().cloned().collect::<HashSet<_>>();
        let dag = automaton
            .find_scc_dag_in_subgraph(path.start().clone(), &visited, |node| node == path.end());

        linear_graph_from_scc_dag_guided_path(&dag, &path, automaton, dimension)
    }

    pub fn empty(automaton: &'a A, dimension: usize) -> Self {
        LinearGraph {
            sequence: Vec::new(),
            graphs: Vec::new(),
            paths: Vec::new(),
            repeat_paths: Vec::new(),
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
        let mut used_repeat_paths = vec![0usize; self.repeat_paths.len()];

        for (part_index, part) in self.sequence.iter().enumerate() {
            match part {
                LinearGraphPart::Graph(idx) => {
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
                LinearGraphPart::Path(idx) => {
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
                LinearGraphPart::RepeatPath(idx) => {
                    let Some(used) = used_repeat_paths.get_mut(*idx) else {
                        panic!(
                            "Part {} references missing repeated path {} (have {})",
                            part_index,
                            idx,
                            self.repeat_paths.len()
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

        for (index, uses) in used_repeat_paths.iter().enumerate() {
            assert_eq!(
                *uses, 1,
                "Repeated path {} must be referenced exactly once, found {} uses",
                index, uses
            );
            let path = &self.repeat_paths[index].path;
            assert!(
                !path.is_empty(),
                "Repeated path {} must be non-empty",
                index
            );
            assert_eq!(
                path.start(),
                path.end(),
                "Repeated path {} must be closed",
                index
            );
        }

        for (index, graph) in self.graphs.iter().enumerate() {
            assert!(
                graph.graph.node_weight(graph.start).is_some(),
                "Graph {} start boundary {:?} is not a live node",
                index,
                graph.start
            );
            assert!(
                graph.graph.node_weight(graph.end).is_some(),
                "Graph {} end boundary {:?} is not a live node",
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
                "Adjacent LinearGraph parts must share a boundary"
            );
        }
    }

    #[cfg(not(debug_assertions))]
    pub fn assert_consistent(&self) {}

    fn compact_parts_storage(&mut self) {
        let old_graphs = std::mem::take(&mut self.graphs);
        let old_paths = std::mem::take(&mut self.paths);
        let old_repeat_paths = std::mem::take(&mut self.repeat_paths);

        let mut graph_map = HashMap::new();
        let mut path_map = HashMap::new();
        let mut repeat_path_map = HashMap::new();
        let mut graphs = Vec::with_capacity(old_graphs.len());
        let mut paths = Vec::with_capacity(old_paths.len());
        let mut repeat_paths = Vec::with_capacity(old_repeat_paths.len());

        for part in &mut self.sequence {
            match *part {
                LinearGraphPart::Graph(old_idx) => {
                    let new_idx = *graph_map.entry(old_idx).or_insert_with(|| {
                        let Some(graph) = old_graphs.get(old_idx) else {
                            panic!("Part references missing graph {} while compacting", old_idx);
                        };

                        graphs.push(Arc::clone(graph));
                        graphs.len() - 1
                    });

                    *part = LinearGraphPart::Graph(new_idx);
                }
                LinearGraphPart::Path(old_idx) => {
                    let new_idx = *path_map.entry(old_idx).or_insert_with(|| {
                        let Some(path) = old_paths.get(old_idx) else {
                            panic!("Part references missing path {} while compacting", old_idx);
                        };

                        paths.push(path.clone());
                        paths.len() - 1
                    });

                    *part = LinearGraphPart::Path(new_idx);
                }
                LinearGraphPart::RepeatPath(old_idx) => {
                    let new_idx = *repeat_path_map.entry(old_idx).or_insert_with(|| {
                        let Some(path) = old_repeat_paths.get(old_idx) else {
                            panic!(
                                "Part references missing repeated path {} while compacting",
                                old_idx
                            );
                        };

                        repeat_paths.push(path.clone());
                        repeat_paths.len() - 1
                    });

                    *part = LinearGraphPart::RepeatPath(new_idx);
                }
            }
        }

        self.graphs = graphs;
        self.paths = paths;
        self.repeat_paths = repeat_paths;
    }

    pub fn add_graph(&mut self, graph: impl Into<Arc<LinearGraphRegion<NIndex>>>) {
        let index = self.graphs.len();
        self.graphs.push(graph.into());
        self.sequence.push(LinearGraphPart::Graph(index));
        self.assert_consistent();
    }

    pub fn add_path(&mut self, path: LinearGraphPathSegment<NIndex>) {
        let index = self.paths.len();
        self.paths.push(path);
        self.sequence.push(LinearGraphPart::Path(index));
        self.assert_consistent();
    }

    pub fn add_repeat_path(&mut self, path: LinearGraphRepeatPath<NIndex>) {
        let index = self.repeat_paths.len();
        self.repeat_paths.push(path);
        self.sequence.push(LinearGraphPart::RepeatPath(index));
        self.assert_consistent();
    }

    pub fn graph(&self, index: usize) -> &LinearGraphRegion<NIndex> {
        self.graphs[index].as_ref()
    }

    pub fn path(&self, index: usize) -> &LinearGraphPathSegment<NIndex> {
        &self.paths[index]
    }

    pub fn repeat_path(&self, index: usize) -> &LinearGraphRepeatPath<NIndex> {
        &self.repeat_paths[index]
    }

    /// Adds a node from the CFG to the LinearGraph. The node needs to be
    /// connected to at least one node in the LinearGraph, otherwise the
    /// function will panic. This function will also add all existing
    /// connections between the new node and the existing LinearGraph nodes.
    /// This may quickly lead to large graphs and little path like
    /// structure.
    pub fn add_node(&self, node: NIndex) -> Self {
        let mut result = LinearGraph::empty(self.automaton, self.dimension);
        let neighbors = self.automaton.undirected_neighbors(&node);

        // Split paths first so every neighboring path segment touches the new
        // graph only at a segment boundary.
        for part in &self.sequence {
            match part {
                LinearGraphPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| neighbors.contains(s)) {
                        result.add_path(split.into());
                    }
                }
                LinearGraphPart::Graph(idx) => {
                    result.add_graph(Arc::clone(&self.graphs[*idx]));
                }
                LinearGraphPart::RepeatPath(idx) => {
                    result.add_repeat_path(self.repeat_paths[*idx].clone());
                }
            }
        }

        let mut neighbor_parts = vec![];

        for (i, part) in result.sequence.iter().enumerate() {
            for neighbor in &neighbors {
                match part {
                    LinearGraphPart::Graph(_) => {
                        if part.start(&result) == neighbor || part.end(&result) == neighbor {
                            neighbor_parts.push(NeighborPart::Boundary(i));
                            break;
                        }

                        if part.contains_node(&result, neighbor) {
                            neighbor_parts.push(NeighborPart::Interior(i));
                            break;
                        }
                    }
                    LinearGraphPart::Path(_) => {
                        // Paths have already been split, so a neighbor can only
                        // matter at the segment boundary.
                        if part.start(&result) == neighbor || part.end(&result) == neighbor {
                            neighbor_parts.push(NeighborPart::Boundary(i));
                            break;
                        }
                    }
                    LinearGraphPart::RepeatPath(_) => {
                        if part.start(&result) == neighbor {
                            neighbor_parts.push(NeighborPart::Boundary(i));
                            break;
                        }

                        if part.contains_node(&result, neighbor) {
                            neighbor_parts.push(NeighborPart::Interior(i));
                            break;
                        }
                    }
                }
            }
        }

        if neighbor_parts.is_empty() {
            panic!("Cannot add node that is not connected to any part of the LinearGraph");
        }

        let first_part_index = neighbor_parts
            .first()
            .expect("neighbor parts must be non-empty")
            .first_replaced_part();
        let last_part_index = neighbor_parts
            .last()
            .expect("neighbor parts must be non-empty")
            .last_replaced_part();

        let start_node = result.sequence[first_part_index].start(&result).clone();
        let end_node = result.sequence[last_part_index].end(&result).clone();

        let mut cut_sequence = result
            .sequence
            .drain(first_part_index..=last_part_index)
            .collect_vec();

        if cut_sequence.is_empty() {
            assert_eq!(start_node, end_node);

            cut_sequence.push(LinearGraphPart::Path(result.paths.len()));
            result.paths.push(CFGPath::new(start_node.clone()).into());
        }

        let mut new_graph = DiGraph::<NIndex, CFGCounterUpdate>::new();
        let mut node_map = HashMap::new();

        for part in &cut_sequence {
            for node in part.iter_nodes(&result) {
                if node_map.contains_key(node) {
                    continue;
                }

                let new_node = new_graph.add_node(node.clone());
                node_map.insert(node.clone(), new_node);
            }
        }

        let new_node = new_graph.add_node(node.clone());
        node_map.insert(node, new_node);

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

        let graph = LinearGraphRegion::new(
            new_graph,
            new_start_node,
            new_end_node,
            result.automaton.alphabet().to_vec(),
        );

        let graph_index = result.graphs.len();
        result.graphs.push(Arc::new(graph));
        result
            .sequence
            .insert(first_part_index, LinearGraphPart::Graph(graph_index));

        result.compact_parts_storage();
        result.assert_consistent();

        result
    }

    /// Finds the strongly connected component around the given node in the main
    /// CFG and adds it as a graph part. The node must be contained in
    /// the LinearGraph, otherwise the function will panic.
    pub fn add_scc_around_node(&self, state: NIndex) -> Self {
        assert!(
            self.contains_state(&state),
            "Cannot add SCC around node that is not in the LinearGraph"
        );

        let scc_nodes = self.automaton.find_scc_surrounding(state.clone());
        let mut scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        // make deterministic: sort the SCC nodes before building the region
        scc_nodes_vec.sort_unstable();
        let scc = LinearGraphRegion::from_subset(
            self.automaton,
            &scc_nodes_vec,
            state.clone(),
            state.clone(),
        );

        let mut result = LinearGraph::empty(self.automaton, self.dimension);
        result.graphs = self.graphs.clone();

        // first we split all paths at the given node
        for part in &self.sequence {
            match part {
                LinearGraphPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| s == &state) {
                        result.add_path(split.into());
                    }
                }
                LinearGraphPart::Graph(idx) => {
                    result.sequence.push(LinearGraphPart::Graph(*idx));
                }
                LinearGraphPart::RepeatPath(idx) => {
                    result.add_repeat_path(self.repeat_paths[*idx].clone());
                }
            }
        }

        let scc_idx = result.graphs.len();
        result.graphs.push(Arc::new(scc));

        let parts = std::mem::take(&mut result.sequence);
        result.sequence = parts
            .into_iter()
            .flat_map(|part| {
                if part.end(&result) == &state {
                    vec![part, LinearGraphPart::Graph(scc_idx)]
                } else {
                    vec![part]
                }
            })
            .collect();

        result.assert_consistent();

        result
    }

    pub fn add_scc_around_position(&self, path_index: usize, node_index: usize) -> Self {
        let LinearGraphPart::Path(path_idx) = self.sequence[path_index] else {
            panic!("Part must be a path");
        };
        let state = &self.paths[path_idx].path.states[node_index];

        let scc_nodes = self.automaton.find_scc_surrounding(state.clone());
        let mut scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        // make deterministic: sort the SCC nodes before building the region
        scc_nodes_vec.sort_unstable();
        let scc = Arc::new(LinearGraphRegion::from_subset(
            self.automaton,
            &scc_nodes_vec,
            state.clone(),
            state.clone(),
        ));

        let mut result = LinearGraph::empty(self.automaton, self.dimension);

        for (i, part) in self.sequence.iter().enumerate() {
            if i == path_index {
                if node_index == 0 {
                    result.add_graph(Arc::clone(&scc));
                    result.add_path(self.paths[path_idx].clone());
                } else if node_index == self.paths[path_idx].path.states.len() - 1 {
                    result.add_path(self.paths[path_idx].clone());
                    result.add_graph(Arc::clone(&scc));
                } else {
                    let mut path = self.paths[path_idx].clone();
                    let after = path.path.split_off(node_index);

                    result.add_path(path);
                    result.add_graph(Arc::clone(&scc));
                    result.add_path(after.into());
                }
            } else {
                match part {
                    LinearGraphPart::Path(idx) => result.add_path(self.paths[*idx].clone()),
                    LinearGraphPart::Graph(idx) => result.add_graph(Arc::clone(&self.graphs[*idx])),
                    LinearGraphPart::RepeatPath(idx) => {
                        result.add_repeat_path(self.repeat_paths[*idx].clone())
                    }
                }
            }
        }

        result.assert_consistent();

        result
    }

    /// Removes the given node from the graph. The node must be in the graph,
    /// otherwise the function will panic.
    pub fn remove_node_from_graph(&mut self, graph_index: usize, node_index: NodeIndex) {
        let graph = Arc::make_mut(&mut self.graphs[graph_index]);
        if graph.node_count() <= node_index.index() {
            panic!("Node is not in the graph");
        }

        graph.remove_node(&node_index);
        self.assert_consistent();
    }

    /// Removes the given edge from the graph. The edge must be in the graph,
    /// otherwise the function will panic.
    pub fn remove_edge_from_graph(&mut self, graph_index: usize, edge_index: EdgeIndex) {
        let graph = Arc::make_mut(&mut self.graphs[graph_index]);
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
        let graph = Arc::make_mut(&mut self.graphs[graph_index]);
        graph.retain_nodes(|_, node| nodes_to_keep.contains(&node));
        self.assert_consistent();
    }

    /// Checks if the LinearGraph contains the given state from the product.
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

    /// Converts the LinearGraph into an NFA over CFGCounterUpdate.
    pub fn to_nfa(&self) -> NFA<(), CFGCounterUpdate> {
        self.assert_consistent();

        let mut nfa: NFA<(), CFGCounterUpdate> =
            NFA::new(CFGCounterUpdate::alphabet(self.dimension));
        let start_state = nfa.add_node(DfaNode::non_accepting(()));
        nfa.set_initial(start_state);

        let mut prev_state = start_state;

        for part in &self.sequence {
            match part {
                LinearGraphPart::Path(idx) => {
                    let path = self.path(*idx);
                    for update in &path.path.transitions {
                        let next_state = nfa.add_node(DfaNode::non_accepting(()));

                        nfa.add_edge(&prev_state, &next_state, NFAEdge::Symbol(*update));
                        prev_state = next_state;
                    }
                }
                LinearGraphPart::Graph(idx) => {
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
                LinearGraphPart::RepeatPath(idx) => {
                    let path = self.repeat_path(*idx);
                    let loop_start = prev_state;

                    for update in path
                        .path
                        .transitions
                        .iter()
                        .take(path.path.transitions.len() - 1)
                    {
                        let next_state = nfa.add_node(DfaNode::non_accepting(()));
                        nfa.add_edge(&prev_state, &next_state, NFAEdge::Symbol(*update));
                        prev_state = next_state;
                    }

                    let last = path
                        .path
                        .transitions
                        .last()
                        .expect("repeated path must be non-empty");
                    nfa.add_edge(&prev_state, &loop_start, NFAEdge::Symbol(*last));
                    prev_state = loop_start;
                }
            }
        }

        nfa.set_accepting(prev_state);

        nfa
    }

    pub fn to_cfg(&self) -> VASSCFG<()> {
        tracing::debug!("Converting LinearGraph to NFA");
        let nfa = self.to_nfa();

        tracing::debug!(
            "Converting NFA with {} states and {} edges to CFG",
            nfa.graph.node_count(),
            nfa.graph.edge_count()
        );
        nfa.determinize()
    }

    pub fn iter_parts<'b>(&'b self) -> impl Iterator<Item = &'b LinearGraphPart> + 'b {
        self.sequence.iter()
    }

    pub fn iter_path_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LinearGraphPathSegment<NIndex>> + 'b {
        self.sequence.iter().filter_map(|part| match part {
            LinearGraphPart::Path(idx) => Some(self.path(*idx)),
            LinearGraphPart::Graph(_) | LinearGraphPart::RepeatPath(_) => None,
        })
    }

    pub fn iter_graph_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LinearGraphRegion<NIndex>> + 'b {
        self.sequence.iter().filter_map(|part| match part {
            LinearGraphPart::Graph(idx) => Some(self.graph(*idx)),
            LinearGraphPart::Path(_) | LinearGraphPart::RepeatPath(_) => None,
        })
    }

    pub fn iter_repeat_path_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LinearGraphRepeatPath<NIndex>> + 'b {
        self.sequence.iter().filter_map(|part| match part {
            LinearGraphPart::RepeatPath(idx) => Some(self.repeat_path(*idx)),
            LinearGraphPart::Graph(_) | LinearGraphPart::Path(_) => None,
        })
    }
}

fn linear_graph_from_scc_dag_guided_path<'a, NIndex: GIndex, A>(
    dag: &SCCDag<NIndex, CFGCounterUpdate>,
    path: &CFGPath<NIndex>,
    automaton: &'a A,
    dimension: usize,
) -> LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
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

    let mut linear_graph = LinearGraph::empty(automaton, dimension);
    let mut current_path = CFGPath::new(path.start().clone());
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
                linear_graph.add_path(current_path.clone().into());
            }

            linear_graph.add_graph(region_from_scc(
                scc,
                &path.states[state_index],
                &path.states[run_end],
                automaton,
            ));

            current_path = CFGPath::new(path.states[run_end].clone());
        }

        if run_end < path.transitions.len() {
            current_path.add(path.transitions[run_end], path.states[run_end + 1].clone());
        }

        state_index = run_end + 1;
    }

    if !current_path.is_empty() {
        linear_graph.add_path(current_path.into());
    }

    assert!(
        linear_graph.accepts(path.transitions.iter()),
        "Path-guided SCC roll-up must accept the original path"
    );

    linear_graph
}

fn region_from_scc<NIndex: GIndex, A>(
    scc: &SCC<NIndex>,
    start: &NIndex,
    end: &NIndex,
    automaton: &A,
) -> LinearGraphRegion<NIndex>
where
    A: TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + Alphabet<Letter = CFGCounterUpdate>,
{
    // Keep node order deterministic so repeated runs over the same SCC-DAG
    // produce structurally stable linear graph regions.
    let mut nodes = scc.nodes.clone();
    nodes.sort_unstable();
    LinearGraphRegion::from_subset(automaton, &nodes, start.clone(), end.clone())
}

fn graph_accepting_end_positions<NIndex: GIndex>(
    graph: &LinearGraphRegion<NIndex>,
    input: &[&CFGCounterUpdate],
    start_position: usize,
) -> Vec<usize> {
    let mut current_state = graph.start;
    let mut positions = Vec::new();

    if current_state == graph.end {
        positions.push(start_position);
    }

    for (offset, symbol) in input[start_position..].iter().enumerate() {
        let mut next_state = None;
        for edge_ref in graph
            .graph
            .edges_directed(current_state, petgraph::Direction::Outgoing)
        {
            if *edge_ref.weight() == **symbol {
                next_state = Some(edge_ref.target());
                break;
            }
        }

        let Some(next_state) = next_state else {
            break;
        };
        current_state = next_state;

        if current_state == graph.end {
            positions.push(start_position + offset + 1);
        }
    }

    positions
}

fn repeated_path_accepting_end_positions<NIndex: GIndex>(
    repeated: &LinearGraphRepeatPath<NIndex>,
    input: &[&CFGCounterUpdate],
    start_position: usize,
) -> Vec<usize> {
    let word = &repeated.path.transitions;
    let mut positions = vec![start_position];
    let mut position = start_position;

    while position + word.len() <= input.len()
        && word
            .iter()
            .zip(&input[position..position + word.len()])
            .all(|(expected, actual)| *expected == **actual)
    {
        position += word.len();
        positions.push(position);
    }

    positions
}

impl<'a, NIndex: GIndex, A> Alphabet for LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        self.automaton.alphabet()
    }
}

impl<'a, NIndex: GIndex, A> Language for LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        self.assert_consistent();

        let input = input.into_iter().collect::<Vec<_>>();
        let mut positions = vec![0usize];

        for part in self.sequence.iter() {
            let mut next_positions = Vec::new();

            for position in positions {
                match part {
                    LinearGraphPart::Path(idx) => {
                        let path = self.path(*idx);
                        let end = position.saturating_add(path.path.len());
                        if end <= input.len()
                            && path
                                .path
                                .transitions
                                .iter()
                                .zip(&input[position..end])
                                .all(|(expected, actual)| *expected == **actual)
                        {
                            next_positions.push(end);
                        }
                    }
                    LinearGraphPart::Graph(idx) => {
                        next_positions.extend(graph_accepting_end_positions(
                            self.graph(*idx),
                            &input,
                            position,
                        ));
                    }
                    LinearGraphPart::RepeatPath(idx) => {
                        let repeated = self.repeat_path(*idx);
                        next_positions.extend(repeated_path_accepting_end_positions(
                            repeated, &input, position,
                        ));
                    }
                }
            }

            next_positions.sort_unstable();
            next_positions.dedup();
            if next_positions.is_empty() {
                return false;
            }
            positions = next_positions;
        }

        positions.binary_search(&input.len()).is_ok()
    }
}
