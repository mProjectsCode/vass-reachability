use std::iter::Peekable;

use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{graph::NodeIndex, prelude::StableDiGraph, visit::EdgeRef};

use crate::automaton::{
    Automaton, AutomatonNode,
    dfa::cfg::{CFGCounterUpdate, VASSCFG},
    path::{Path, path_like::PathLike},
};

#[derive(Debug, Clone)]
pub struct LSGGraph {
    pub graph: StableDiGraph<NodeIndex, CFGCounterUpdate>,
    pub start: NodeIndex,
    pub end: NodeIndex,
}

impl LSGGraph {
    pub fn new(
        graph: StableDiGraph<NodeIndex, CFGCounterUpdate>,
        start: NodeIndex,
        end: NodeIndex,
    ) -> Self {
        assert!(
            graph.contains_node(start),
            "Start node {:?} must be in the graph",
            start
        );
        assert!(
            graph.contains_node(end),
            "End node {:?} must be in the graph",
            end
        );

        LSGGraph { graph, start, end }
    }
}

#[derive(Debug, Clone)]
pub enum LSGPart {
    SubGraph(LSGGraph),
    Path(Path),
}

impl LSGPart {
    /// Checks if the part contains the given node.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn contains_node(&self, node: NodeIndex) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.graph.node_weights().contains(&node),
            LSGPart::Path(path) => path.contains_node(node),
        }
    }

    // Checks if the part has the given node as start or end node.
    pub fn has_node_extremal(&self, node: NodeIndex) -> bool {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.start == node || subgraph.end == node,
            LSGPart::Path(path) => path.start() == node || path.end() == node,
        }
    }

    /// Iters the nodes in this part.
    /// The node indices are in the context of the CFG, not the part itself.
    pub fn iter_nodes<'a>(&'a self) -> Box<dyn Iterator<Item = NodeIndex> + 'a> {
        match self {
            LSGPart::SubGraph(subgraph) => Box::new(subgraph.graph.node_weights().cloned()),
            LSGPart::Path(path) => Box::new(path.iter_nodes()),
        }
    }

    /// Returns the start node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn start(&self) -> NodeIndex {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.graph[subgraph.start],
            LSGPart::Path(path) => path.start(),
        }
    }

    /// Returns the end node of the part.
    /// The node index is in the context of the CFG, not the part itself.
    pub fn end(&self) -> NodeIndex {
        match self {
            LSGPart::SubGraph(subgraph) => subgraph.graph[subgraph.end],
            LSGPart::Path(path) => path.end(),
        }
    }

    pub fn is_path(&self) -> bool {
        matches!(self, LSGPart::Path(_))
    }

    pub fn is_subgraph(&self) -> bool {
        matches!(self, LSGPart::SubGraph(_))
    }
}

#[derive(Debug, Clone)]
pub struct LinearSubGraph<'a, N: AutomatonNode> {
    pub parts: Vec<LSGPart>,
    pub cfg: &'a VASSCFG<N>,
}

impl<'a, N: AutomatonNode> LinearSubGraph<'a, N> {
    pub fn from_path(path: Path, cfg: &'a VASSCFG<N>) -> Self {
        LinearSubGraph {
            parts: vec![LSGPart::Path(path)],
            cfg,
        }
    }

    pub fn add_node(&mut self, node: NodeIndex) {
        // first we need to find all parts that contain a neighbor of the node
        // then we build a new subgraph containing everything between the first and last
        // neighbor then we replace all those parts with the new subgraph.
        // For this to work correctly, we would need to ensure that paths get split,
        // otherwise we would end up with just a single giant SubGraph part.
        // As a simple solution, we split the paths beforehand, so that we don't have to
        // deal with the complexity of splitting paths later in this function.

        let neighbors = self.cfg.graph.neighbors_undirected(node).collect_vec();

        // first we split all paths at the given node
        let new_parts = self
            .parts
            .drain(..)
            .flat_map(|part| match part {
                LSGPart::Path(path) => path
                    .split_at_nodes(&neighbors)
                    .into_iter()
                    .map(LSGPart::Path)
                    .collect_vec(),
                LSGPart::SubGraph(_) => vec![part],
            })
            .collect_vec();

        self.parts = new_parts;

        // then we find all parts that contain a neighbor of the node
        let mut neighbor_parts_indices = vec![];

        for (i, part) in self.parts.iter().enumerate() {
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

        let first_part_index = first_part.0 + usize::from(first_part.1);
        let last_part_index = last_part.0 - usize::from(last_part.1);

        let start_node = self.parts[first_part_index].start();
        let end_node = self.parts[last_part_index].end();

        let mut cut_sequence = self
            .parts
            .drain(first_part_index..=last_part_index)
            .collect_vec();

        if cut_sequence.is_empty() {
            assert_eq!(start_node, end_node);

            cut_sequence.push(LSGPart::Path(Path::new(start_node)));
        }

        let mut new_subgraph = StableDiGraph::<NodeIndex, CFGCounterUpdate>::new();
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
            for edge_ref in self.cfg.graph.edges(*cfg_node) {
                if let Some(&new_target) = node_map.get(&edge_ref.target()) {
                    new_subgraph.add_edge(*new_node, new_target, *edge_ref.weight());
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

        self.parts
            .insert(first_part_index, LSGPart::SubGraph(graph));
    }
}

fn partial_accept_path<'a, N: AutomatonNode>(
    path: &Path,
    cfg: &VASSCFG<N>,
    input: &mut Peekable<impl Iterator<Item = &'a CFGCounterUpdate>>,
) -> bool {
    let mut index = 0;

    if path.len() == 0 {
        return true;
    }

    while let Some(symbol) = input.peek() {
        let (edge_index, _) = path.get(index);
        let edge = cfg
            .graph
            .edge_weight(edge_index)
            .expect("Edge in path must exist in CFG");

        if edge == *symbol {
            index += 1;
            input.next();
        } else {
            return false;
        }

        if index == path.len() {
            return true;
        }
    }

    index == path.len()
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

impl<'a, N: AutomatonNode> Automaton<CFGCounterUpdate> for LinearSubGraph<'a, N> {
    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        let mut input = input.into_iter().peekable();
        for part in self.parts.iter() {
            let success = match part {
                LSGPart::Path(path) => partial_accept_path(path, self.cfg, &mut input),
                LSGPart::SubGraph(subgraph) => partial_accept_subgraph(subgraph, &mut input),
            };

            if !success {
                return false;
            }
        }

        // lastly we need to check that we are at the end of the input
        input.next().is_none()
    }

    fn alphabet(&self) -> &Vec<CFGCounterUpdate> {
        self.cfg.alphabet()
    }
}
