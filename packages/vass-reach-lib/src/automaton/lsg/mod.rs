use std::iter::Peekable;

use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
};

use super::nfa::NFAEdge;
use crate::automaton::{
    Alphabet, Language, ModifiableAutomaton, TransitionSystem,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath, state::MultiGraphState},
    lsg::part::{LSGGraph, LSGPart, LSGPath},
    nfa::NFA,
};

pub mod extender;
pub mod part;

#[derive(Debug, Clone)]
pub struct LinearSubGraph<'a> {
    pub parts: Vec<LSGPart>,
    pub subgraphs: Vec<LSGGraph>,
    pub paths: Vec<LSGPath>,
    pub product: &'a ImplicitCFGProduct,
    pub dimension: usize,
}

impl<'a> LinearSubGraph<'a> {
    pub fn from_path(
        path: MultiGraphPath,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
    ) -> Self {
        let mut instance = Self::empty(product, dimension);
        instance.add_path(path.into());
        instance
    }

    pub fn empty(product: &'a ImplicitCFGProduct, dimension: usize) -> Self {
        LinearSubGraph {
            parts: Vec::new(),
            subgraphs: Vec::new(),
            paths: Vec::new(),
            product,
            dimension,
        }
    }

    pub fn add_subgraph(&mut self, subgraph: LSGGraph) {
        let index = self.subgraphs.len();
        self.subgraphs.push(subgraph);
        self.parts.push(LSGPart::SubGraph(index));
    }

    pub fn add_path(&mut self, path: LSGPath) {
        let index = self.paths.len();
        self.paths.push(path);
        self.parts.push(LSGPart::Path(index));
    }

    pub fn subgraph(&self, index: usize) -> &LSGGraph {
        &self.subgraphs[index]
    }

    pub fn path(&self, index: usize) -> &LSGPath {
        &self.paths[index]
    }

    /// Adds a node from the CFG to the LSG. The node needs to be connected to
    /// at least one node in the LSG, otherwise the function will panic.
    /// This function will also add all existing connections between the new
    /// node and the existing LSG nodes. This may quickly lead to large
    /// subgraphs and little path like structure.
    pub fn add_node(&self, node: MultiGraphState) -> Self {
        // first we need to find all parts that contain a neighbor of the node
        // then we build a new subgraph containing everything between the first and last
        // neighbor then we replace all those parts with the new subgraph.
        // For this to work correctly, we would need to ensure that paths get split,
        // otherwise we would end up with just a single giant SubGraph part.
        // As a simple solution, we split the paths beforehand, so that we don't have to
        // deal with the complexity of splitting paths later in this function.

        // dbg!(&self.parts);
        // dbg!(node);

        let mut result = LinearSubGraph::empty(self.product, self.dimension);
        let neighbors = self.product.undirected_neighbors(&node);

        // first we split all paths at the given node
        for part in &self.parts {
            match part {
                LSGPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| neighbors.contains(s)) {
                        result.add_path(split.into());
                    }
                }
                LSGPart::SubGraph(idx) => {
                    result.add_subgraph(self.subgraph(*idx).clone());
                }
            }
        }

        // then we find all parts that contain a neighbor of the node
        // the second boolean in the tuple indicates whether the neighbor is at the
        // start or end of the part (true) or inside the part (false)
        let mut neighbor_parts_indices = vec![];

        for (i, part) in result.parts.iter().enumerate() {
            for neighbor in &neighbors {
                match part {
                    LSGPart::SubGraph(_) => {
                        if part.start(&result) == neighbor || part.end(&result) == neighbor {
                            neighbor_parts_indices.push((i, true));
                            break;
                        }

                        if part.contains_node(&result, neighbor) {
                            neighbor_parts_indices.push((i, false));
                            break;
                        }
                    }
                    LSGPart::Path(_) => {
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
            panic!("Cannot add node that is not connected to any part of the LSG");
        }

        // thanks to the way we search for neighbors, the indices should be sorted
        let first_part = *neighbor_parts_indices.first().unwrap();
        let last_part = *neighbor_parts_indices.last().unwrap();

        // dbg!(&neighbor_parts_indices);

        let first_part_index = first_part.0 + usize::from(first_part.1);
        let last_part_index = last_part.0 - usize::from(last_part.1);

        let start_node = result.parts[first_part_index].start(&result).clone();
        let end_node = result.parts[last_part_index].end(&result).clone();

        let mut cut_sequence = result
            .parts
            .drain(first_part_index..=last_part_index)
            .collect_vec();

        if cut_sequence.is_empty() {
            assert_eq!(start_node, end_node);

            cut_sequence.push(LSGPart::Path(result.paths.len()));
            result
                .paths
                .push(MultiGraphPath::new(start_node.clone()).into());
        }

        let mut new_subgraph = DiGraph::<MultiGraphState, CFGCounterUpdate>::new();
        let mut node_map = HashMap::new();

        // add all nodes from the cut sequence to the new subgraph
        for part in &cut_sequence {
            for node in part.iter_nodes(&result) {
                // we may have already added this node, because start and end nodes overlap
                if node_map.contains_key(node) {
                    continue;
                }

                let new_node = new_subgraph.add_node(node.clone());
                node_map.insert(node.clone(), new_node);
            }
        }

        // add the new node
        let new_node = new_subgraph.add_node(node.clone());
        node_map.insert(node, new_node);

        // now we add all edges between the nodes in the new subgraph
        for (product_state, new_node) in &node_map {
            for letter in result.product.alphabet() {
                let Some(successor) = result.product.successor(product_state, letter) else {
                    continue;
                };

                if let Some(&new_target) = node_map.get(&successor) {
                    new_subgraph.add_edge(*new_node, new_target, *letter);
                }
            }
        }

        let new_start_node = *node_map
            .get(&start_node)
            .expect("Start node must be in the new subgraph");
        let new_end_node = *node_map
            .get(&end_node)
            .expect("End node must be in the new subgraph");

        // lastly we create the new LSGGraph and insert it into the parts
        let graph = LSGGraph::new(
            new_subgraph,
            new_start_node,
            new_end_node,
            result.product.alphabet().to_vec(),
        );

        let subgraph_index = result.subgraphs.len();
        result.subgraphs.push(graph);
        result
            .parts
            .insert(first_part_index, LSGPart::SubGraph(subgraph_index));

        result
    }

    /// Finds the strongly connected component around the given node in the main
    /// CFG and adds it as a subgraph part. The node must be contained in
    /// the LSG, otherwise the function will panic.
    pub fn add_scc_around_node(&self, state: MultiGraphState) -> Self {
        assert!(
            self.contains_state(&state),
            "Cannot add SCC around node that is not in the LSG"
        );

        let scc_nodes = self.product.find_scc_surrounding(state.clone());
        let scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        let scc = LSGGraph::from_subset(self.product, &scc_nodes_vec, state.clone(), state.clone());

        let mut result = LinearSubGraph::empty(self.product, self.dimension);

        // first we split all paths at the given node
        for part in &self.parts {
            match part {
                LSGPart::Path(idx) => {
                    let path = self.path(*idx);
                    for split in path.path.clone().split_at(|s, _| s == &state) {
                        result.add_path(split.into());
                    }
                }
                LSGPart::SubGraph(idx) => {
                    result.add_subgraph(self.subgraph(*idx).clone());
                }
            }
        }

        let mut final_result = LinearSubGraph::empty(self.product, self.dimension);
        let scc_idx = final_result.subgraphs.len();
        final_result.subgraphs.push(scc);

        for part in &result.parts {
            let ends_at_node = part.end(&result) == &state;
            match part {
                LSGPart::Path(idx) => final_result.add_path(result.paths[*idx].clone()),
                LSGPart::SubGraph(idx) => final_result.add_subgraph(result.subgraphs[*idx].clone()),
            }

            if ends_at_node {
                // we found a part that ends at node, so now we need to insert the SCC before
                // continuing with the rest
                final_result.parts.push(LSGPart::SubGraph(scc_idx));
            }
        }

        final_result
    }

    pub fn add_scc_around_position(&self, path_index: usize, node_index: usize) -> Self {
        let LSGPart::Path(path_idx) = self.parts[path_index] else {
            panic!("Part must be a path");
        };
        let state = &self.paths[path_idx].path.states[node_index];

        let scc_nodes = self.product.find_scc_surrounding(state.clone());
        let scc_nodes_vec = scc_nodes.into_iter().collect_vec();
        let scc = LSGGraph::from_subset(self.product, &scc_nodes_vec, state.clone(), state.clone());

        let mut result = LinearSubGraph::empty(self.product, self.dimension);

        for (i, part) in self.parts.iter().enumerate() {
            if i == path_index {
                if node_index == 0 {
                    result.add_subgraph(scc.clone());
                    result.add_path(self.paths[path_idx].clone());
                } else {
                    let mut path = self.paths[path_idx].clone();
                    let after = path.path.split_off(node_index - 1);

                    result.add_path(path);
                    result.add_subgraph(scc.clone());
                    result.add_path(after.into());
                }
            } else {
                match part {
                    LSGPart::Path(idx) => result.add_path(self.paths[*idx].clone()),
                    LSGPart::SubGraph(idx) => result.add_subgraph(self.subgraphs[*idx].clone()),
                }
            }
        }

        result
    }

    /// Checks if the LSG contains the given state from the product.
    pub fn contains_state(&self, state: &MultiGraphState) -> bool {
        for part in &self.parts {
            if part.contains_node(self, state) {
                return true;
            }
        }

        false
    }

    pub fn size(&self) -> usize {
        self.parts.iter().map(|part| part.size(self)).sum()
    }

    /// Converts the LSG into an NFA over CFGCounterUpdate.
    pub fn to_nfa(&self) -> NFA<(), CFGCounterUpdate> {
        let mut nfa: NFA<(), CFGCounterUpdate> =
            NFA::new(CFGCounterUpdate::alphabet(self.dimension));
        let start_state = nfa.add_node(DfaNode::non_accepting(()));
        nfa.set_initial(start_state);

        let mut state_offset = 0;
        let mut prev_state = start_state;

        for part in &self.parts {
            match part {
                LSGPart::Path(idx) => {
                    let path = self.path(*idx);
                    for update in path.path.iter() {
                        let next_state = nfa.add_node(DfaNode::non_accepting(()));

                        nfa.add_edge(&prev_state, &next_state, NFAEdge::Symbol(update));
                        prev_state = next_state;
                    }

                    state_offset += path.path.len();
                }
                LSGPart::SubGraph(idx) => {
                    let subgraph = self.subgraph(*idx);
                    // first add all states
                    for _ in subgraph.graph.node_indices() {
                        nfa.add_node(DfaNode::non_accepting(()));
                    }

                    // then connect the previous part to the start of the subgraph
                    for i in subgraph.graph.node_indices() {
                        if i == subgraph.start {
                            let end_index = (i.index() + state_offset) as u32;
                            nfa.add_edge(
                                &prev_state,
                                &NodeIndex::from(end_index),
                                NFAEdge::Epsilon,
                            );
                        }
                    }

                    // then set the prev_state to the end of the subgraph
                    for i in subgraph.graph.node_indices() {
                        if i == subgraph.end {
                            let end_index = (i.index() + state_offset) as u32;
                            prev_state = end_index.into();
                        }
                    }

                    // add all edges
                    for edge_ref in subgraph.graph.edge_references() {
                        let src =
                            NodeIndex::from((edge_ref.source().index() + state_offset) as u32);
                        let dst =
                            NodeIndex::from((edge_ref.target().index() + state_offset) as u32);
                        let weight = *edge_ref.weight();

                        nfa.add_edge(&src, &dst, NFAEdge::Symbol(weight));
                    }

                    state_offset += subgraph.graph.node_count();
                }
            }
        }

        nfa.set_accepting(prev_state);

        nfa
    }

    pub fn to_cfg(&self) -> VASSCFG<()> {
        tracing::debug!("Converting LSG to NFA");
        let nfa = self.to_nfa();
        tracing::debug!(
            "Converting NFA with {} states and {} edges to CFG",
            nfa.graph.node_count(),
            nfa.graph.edge_count()
        );
        nfa.determinize()
    }

    pub fn iter_parts<'b>(&'b self) -> impl Iterator<Item = &'b LSGPart> + 'b {
        self.parts.iter()
    }

    pub fn iter_path_parts<'b>(&'b self) -> impl Iterator<Item = &'b LSGPath> + 'b {
        self.parts.iter().filter_map(|part| match part {
            LSGPart::Path(idx) => Some(self.path(*idx)),
            LSGPart::SubGraph(_) => None,
        })
    }

    pub fn iter_subgraph_parts<'b>(&'b self) -> impl Iterator<Item = &'b LSGGraph> + 'b {
        self.parts.iter().filter_map(|part| match part {
            LSGPart::SubGraph(idx) => Some(self.subgraph(*idx)),
            LSGPart::Path(_) => None,
        })
    }
}

fn partial_accept_path<'a>(
    path: &LSGPath,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut index = 0;

    if path.path.is_empty() {
        return true;
    }

    while let Some(symbol) = input.peek() {
        let update = path.path.updates[index];

        if &update == *symbol {
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

fn partial_accept_subgraph<'a>(
    subgraph: &LSGGraph,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut current_state = subgraph.start;

    while let Some(symbol) = input.peek() {
        let mut found_next_state = false;
        for edge_ref in subgraph
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

    current_state == subgraph.end
}

impl<'a> Alphabet for LinearSubGraph<'a> {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        self.product.alphabet()
    }
}

impl<'a> Language for LinearSubGraph<'a> {
    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        let mut input = input.into_iter().peekable();
        for part in self.parts.iter() {
            let success = match part {
                LSGPart::Path(idx) => partial_accept_path(self.path(*idx), &mut input),
                LSGPart::SubGraph(idx) => partial_accept_subgraph(self.subgraph(*idx), &mut input),
            };

            if !success {
                return false;
            }
        }

        // lastly we need to check that we are at the end of the input
        input.next().is_none()
    }
}
