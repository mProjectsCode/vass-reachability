use hashbrown::{HashMap, HashSet};
use petgraph::{
    graph::{self, DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use super::Path;
use crate::{
    automaton::{AutomatonEdge, AutomatonNode, dfa::DFA},
    logger::{LogLevel, Logger},
};

#[derive(Debug, Clone)]
pub struct ParikhImage {
    pub image: HashMap<EdgeIndex, u32>,
}

impl Default for ParikhImage {
    fn default() -> Self {
        ParikhImage::empty()
    }
}

impl ParikhImage {
    pub fn new(image: HashMap<EdgeIndex, u32>) -> Self {
        ParikhImage { image }
    }

    pub fn empty() -> Self {
        ParikhImage {
            image: HashMap::new(),
        }
    }

    pub fn print(&self, logger: &Logger, level: LogLevel) {
        for (edge, count) in &self.image {
            logger.log(level.clone(), &format!("Edge: {}: {}", edge.index(), count));
        }
    }

    pub fn get(&self, edge: EdgeIndex) -> u32 {
        *self.image.get(&edge).unwrap_or(&0)
    }

    pub fn set(&mut self, edge: EdgeIndex, count: u32) {
        self.image.insert(edge, count);
    }

    pub fn add_to(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.entry(edge).or_insert(0);
        *entry += count;
    }

    pub fn sub_from(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.entry(edge).or_insert(0);
        if *entry < count {
            *entry = 0;
        } else {
            *entry -= count;
        }
    }

    /// Set an edge to the maximum of the current count and the given count.
    pub fn set_max(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.entry(edge).or_insert(0);
        *entry = count.max(*entry);
    }

    pub fn is_empty(&self) -> bool {
        self.image.is_empty() || self.image.values().all(|&x| x == 0)
    }

    /// Split the Parikh Image into possibly multiple connected components.
    /// The main connected component is the one that contains the start node.
    /// The connected components are determined by a depth-first search.
    pub fn split_into_connected_components<N, E>(
        mut self,
        graph: &DiGraph<N, E>,
        start: NodeIndex,
    ) -> (ParikhImage, Vec<ParikhImage>) {
        let mut components = vec![];
        let mut visited = vec![false; graph.node_count()];

        let main_component = self.split_connected_component(&mut visited, graph, start);

        for node in graph.node_indices() {
            if visited[node.index()] {
                continue;
            }

            let component = self.split_connected_component(&mut visited, graph, node);
            if !component.is_empty() {
                components.push(component);
            }
        }

        (main_component, components)
    }

    /// Create a new Parikh Image that contains the connected component that the
    /// start node is in. The connected component is determined by a
    /// depth-first search.
    ///
    /// Edges that are part of the connected component are removed from the
    /// original Parikh Image.
    fn split_connected_component<N, E>(
        &mut self,
        visited: &mut [bool],
        graph: &DiGraph<N, E>,
        start: NodeIndex,
    ) -> ParikhImage {
        let mut stack = vec![start];
        let mut component = ParikhImage::empty();

        while let Some(node) = stack.pop() {
            if visited[node.index()] {
                continue;
            }

            visited[node.index()] = true;

            for e in graph.edges(node) {
                let edge = e.id();

                if self.get(edge) == 0 {
                    continue;
                }

                let target = e.target();
                let target_visited = visited[target.index()];

                let count = self.get(edge);
                self.set(edge, 0);
                component.set_max(edge, count);

                if !target_visited {
                    stack.push(target);
                }
            }
        }

        component
    }

    /// Get the edges that go from the connected components, formed by this
    /// parikh image, to the outside. So from a node that is connected to by
    /// one edge of the parikh image to a node that is not connected.
    pub fn get_outgoing_edges<N, E>(&self, graph: &DiGraph<N, E>) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(graph);

        let mut edges = HashSet::new();

        // next we get all edges that go from a connected node to a node outside the
        // connected component
        for node in &connected_nodes {
            for edge in graph.edges(*node) {
                if !connected_nodes.contains(&edge.target()) {
                    edges.insert(edge.id());
                }
            }
        }

        edges
    }

    pub fn get_incoming_edges<N, E>(&self, graph: &DiGraph<N, E>) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(graph);

        let mut edges = HashSet::new();

        // next we get all edges that go from a node outside the connected component
        // to a connected node
        for node in &connected_nodes {
            for edge in graph.edges_directed(*node, petgraph::Direction::Incoming) {
                if !connected_nodes.contains(&edge.source()) {
                    edges.insert(edge.id());
                }
            }
        }

        edges
    }

    pub fn get_connected_nodes<N, E>(&self, graph: &DiGraph<N, E>) -> HashSet<NodeIndex> {
        let mut connected_nodes = HashSet::new();

        for edge in graph.edge_references() {
            if self.get(edge.id()) == 0 {
                continue;
            }

            connected_nodes.insert(edge.source());
            connected_nodes.insert(edge.target());
        }

        connected_nodes
    }

    pub fn iter(&self) -> impl Iterator<Item = (EdgeIndex, u32)> + use<'_> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, count)| (*edge, *count))
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = EdgeIndex> + use<'_> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, _)| *edge)
    }
}

impl From<&Path> for ParikhImage {
    fn from(path: &Path) -> Self {
        let mut map = HashMap::new();

        for (edge, _) in &path.transitions {
            *map.entry(*edge).or_insert(0) += 1;
        }

        ParikhImage::new(map)
    }
}
