use std::iter::Peekable;

use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{graph::DiGraph, visit::EdgeRef};

use super::nfa::NFAEdge;
use crate::automaton::{
    Automaton, Language,
    cfg::{CFG, update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    lsg::part::{LSGGraph, LSGPart, LSGPath},
    nfa::NFA,
    path::{Path, path_like::IndexPath},
};

pub mod extender;
pub mod part;

#[derive(Debug, Clone)]
pub struct LinearSubGraph<'a, C: CFG> {
    pub parts: Vec<LSGPart<C::NIndex, C::EIndex>>,
    pub cfg: &'a C,
    pub dimension: usize,
}

impl<'a, C: CFG> LinearSubGraph<'a, C> {
    pub fn from_path(path: Path<C::NIndex, C::EIndex>, cfg: &'a C, dimension: usize) -> Self {
        LinearSubGraph {
            parts: vec![LSGPart::Path(path.into())],
            cfg,
            dimension,
        }
    }

    /// Adds a node from the CFG to the LSG. The node needs to be connected to
    /// at least one node in the LSG, otherwise the function will panic.
    /// This function will also add all existing connections between the new
    /// node and the existing LSG nodes. This may quickly lead to large
    /// subgraphs and little path like structure.
    pub fn add_node(&self, node: C::NIndex) -> Self {
        // first we need to find all parts that contain a neighbor of the node
        // then we build a new subgraph containing everything between the first and last
        // neighbor then we replace all those parts with the new subgraph.
        // For this to work correctly, we would need to ensure that paths get split,
        // otherwise we would end up with just a single giant SubGraph part.
        // As a simple solution, we split the paths beforehand, so that we don't have to
        // deal with the complexity of splitting paths later in this function.

        // dbg!(&self.parts);
        // dbg!(node);

        let neighbors = self.cfg.undirected_neighbor_indices(node);

        // first we split all paths at the given node
        let mut new_parts = self
            .parts
            .iter()
            .flat_map(|part| match part {
                LSGPart::Path(path) => path
                    .path
                    .clone()
                    .split_at_nodes(&neighbors)
                    .into_iter()
                    .map(|p| LSGPart::Path(p.into()))
                    .collect_vec(),
                LSGPart::SubGraph(_) => vec![part.clone()],
            })
            .collect_vec();

        // then we find all parts that contain a neighbor of the node
        let mut neighbor_parts_indices = vec![];

        for (i, part) in new_parts.iter().enumerate() {
            for neighbor in &neighbors {
                match part {
                    LSGPart::SubGraph(_) => {
                        if part.start() == *neighbor || part.end() == *neighbor {
                            neighbor_parts_indices.push((i, true));
                            break;
                        }

                        if part.contains_node(*neighbor) {
                            neighbor_parts_indices.push((i, false));
                            break;
                        }
                    }
                    LSGPart::Path(_) => {
                        // since we split the paths beforehand, we only need to check the start and
                        // end nodes
                        if part.start() == *neighbor || part.end() == *neighbor {
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

        let start_node = new_parts[first_part_index].start();
        let end_node = new_parts[last_part_index].end();

        let mut cut_sequence = new_parts
            .drain(first_part_index..=last_part_index)
            .collect_vec();

        if cut_sequence.is_empty() {
            assert_eq!(start_node, end_node);

            cut_sequence.push(LSGPart::Path(Path::new(start_node).into()));
        }

        let mut new_subgraph = DiGraph::<C::NIndex, CFGCounterUpdate>::new();
        let mut node_map = HashMap::new();

        // add all nodes from the cut sequence to the new subgraph
        for part in cut_sequence {
            for node in part.iter_nodes() {
                // we may have already added this node, because start and end nodes overlap
                if node_map.contains_key(&node) {
                    continue;
                }

                let new_node = new_subgraph.add_node(node);
                node_map.insert(node, new_node);
            }
        }

        // add the new node
        let new_node = new_subgraph.add_node(node);
        node_map.insert(node, new_node);

        // now we add all edges between the nodes in the new subgraph
        for (cfg_node, new_node) in &node_map {
            for edge in self.cfg.outgoing_edge_indices(*cfg_node) {
                if let Some(&new_target) = node_map.get(&self.cfg.edge_target_unchecked(edge)) {
                    new_subgraph.add_edge(
                        *new_node,
                        new_target,
                        *self.cfg.get_edge_unchecked(edge),
                    );
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
        let graph = LSGGraph::new(new_subgraph, new_start_node, new_end_node);

        new_parts.insert(first_part_index, LSGPart::SubGraph(graph));

        LinearSubGraph {
            parts: new_parts,
            cfg: self.cfg,
            dimension: self.dimension,
        }
    }

    /// Finds the strongly connected component around the given node and adds it
    /// as a subgraph part. The node must be contained in the LSG, otherwise
    /// the function will panic.
    pub fn add_scc_around_node(&self, node: C::NIndex) -> Self {
        assert!(
            self.contains_node(node),
            "Cannot add SCC around node that is not in the LSG"
        );

        unimplemented!()
    }

    /// Checks if the LSG contains the given node from the CFG.
    pub fn contains_node(&self, node: C::NIndex) -> bool {
        for part in &self.parts {
            if part.contains_node(node) {
                return true;
            }
        }

        false
    }

    /// Converts the LSG into an NFA over CFGCounterUpdate.
    pub fn to_nfa(&self) -> NFA<(), CFGCounterUpdate> {
        let mut nfa: NFA<(), CFGCounterUpdate> =
            NFA::new(CFGCounterUpdate::alphabet(self.dimension));
        let start_state = nfa.add_node(DfaNode::non_accepting(()));
        nfa.set_start(start_state);

        let mut state_offset = 0;
        let mut prev_state = start_state;

        for part in &self.parts {
            match part {
                LSGPart::Path(path) => {
                    for (edge_index, _) in path.path.iter() {
                        let next_state = nfa.add_node(DfaNode::non_accepting(()));
                        let edge_weight = *self.cfg.get_edge_unchecked(*edge_index);

                        nfa.add_edge(prev_state, next_state, NFAEdge::Symbol(edge_weight));
                        prev_state = next_state;
                    }

                    state_offset += path.path.len();
                }
                LSGPart::SubGraph(subgraph) => {
                    // first add all states
                    for _ in subgraph.graph.node_indices() {
                        nfa.add_node(DfaNode::non_accepting(()));
                    }

                    // then connect the previous part to the start of the subgraph
                    for i in subgraph.graph.node_indices() {
                        if i == subgraph.start {
                            let end_index = (i.index() + state_offset) as u32;
                            nfa.add_edge(prev_state, end_index.into(), NFAEdge::Epsilon);
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
                        let src = (edge_ref.source().index() + state_offset) as u32;
                        let dst = (edge_ref.target().index() + state_offset) as u32;
                        let weight = *edge_ref.weight();

                        nfa.add_edge(src.into(), dst.into(), NFAEdge::Symbol(weight));
                    }

                    state_offset += subgraph.graph.node_count();
                }
            }
        }

        nfa.set_accepting(prev_state);

        nfa
    }

    pub fn to_cfg(&self) -> VASSCFG<()> {
        let nfa = self.to_nfa();
        nfa.determinize()
    }

    pub fn iter_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LSGPart<C::NIndex, C::EIndex>> + 'b {
        self.parts.iter()
    }

    pub fn iter_path_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LSGPath<C::NIndex, C::EIndex>> + 'b {
        self.parts.iter().filter_map(|part| match part {
            LSGPart::Path(path) => Some(path),
            LSGPart::SubGraph(_) => None,
        })
    }

    pub fn iter_subgraph_parts<'b>(
        &'b self,
    ) -> impl Iterator<Item = &'b LSGGraph<C::NIndex, C::EIndex>> + 'b {
        self.parts.iter().filter_map(|part| match part {
            LSGPart::SubGraph(subgraph) => Some(subgraph),
            LSGPart::Path(_) => None,
        })
    }
}

fn partial_accept_path<'a, C: CFG>(
    path: &LSGPath<C::NIndex, C::EIndex>,
    cfg: &C,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut index = 0;

    if path.path.len() == 0 {
        return true;
    }

    while let Some(symbol) = input.peek() {
        let (edge, _) = path.path.get(index);
        let update = cfg.get_edge(edge).expect("edge must exist in CFG");

        if update == *symbol {
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

fn partial_accept_subgraph<'a, C: CFG>(
    subgraph: &LSGGraph<C::NIndex, C::EIndex>,
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

impl<'a, C: CFG> Language for LinearSubGraph<'a, C> {
    type Letter = CFGCounterUpdate;

    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        let mut input = input.into_iter().peekable();
        for part in self.parts.iter() {
            let success = match part {
                LSGPart::Path(path) => partial_accept_path::<C>(path, self.cfg, &mut input),
                LSGPart::SubGraph(subgraph) => partial_accept_subgraph::<C>(subgraph, &mut input),
            };

            if !success {
                return false;
            }
        }

        // lastly we need to check that we are at the end of the input
        input.next().is_none()
    }

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        self.cfg.alphabet()
    }
}
