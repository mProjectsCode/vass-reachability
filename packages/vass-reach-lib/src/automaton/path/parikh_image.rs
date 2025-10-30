use hashbrown::{HashMap, HashSet};
use petgraph::{
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use super::Path;
use crate::{automaton::{AutomatonNode, dfa::cfg::{CFGCounterUpdatable, VASSCFG}, vass::counter::VASSCounterValuation}, 
    logger::{LogLevel, Logger}}
;

#[derive(Debug, Clone)]
pub struct ParikhImage {
    // TODO: since edge indices are dense, we could use a Vec<u32> instead of a HashMap
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

        pub fn can_build_n_run<N: AutomatonNode>(
        &self,
        cfg: &VASSCFG<N>,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> bool {
        let valuation = initial_valuation.clone();

        rec_can_build_run(
            self.clone(),
            valuation,
            final_valuation,
            cfg,
            cfg.get_start().expect("CFG has no start node"),
            true,
        )
    }

    pub fn can_build_z_run<N: AutomatonNode>(
        &self,
        cfg: &VASSCFG<N>,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> bool {
        let valuation = initial_valuation.clone();

        rec_can_build_run(
            self.clone(),
            valuation,
            final_valuation,
            cfg,
            cfg.get_start().expect("CFG has no start node"),
            false,
        )
    }

    pub fn get_total_counter_effect<N: AutomatonNode>(
        &self,
        cfg: &VASSCFG<N>,
        dimension: usize,
    ) -> VASSCounterValuation {
        let mut total_effect = VASSCounterValuation::zero(dimension);

        for (edge_index, count) in &self.image {
            let edge = cfg.graph.edge_weight(*edge_index).expect("Edge not found in CFG");
        
            total_effect[edge.counter()] += edge.op() * (*count as i32);
        }

        total_effect
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

fn rec_can_build_run<N: AutomatonNode>(
    parikh_image: ParikhImage,
    valuation: VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    cfg: &VASSCFG<N>,
    node_index: NodeIndex,
    n_run: bool,
) -> bool {
    let is_final = cfg.graph[node_index].accepting;
    // if the parikh image is empty, we have reached the end of the path, which also
    // means that the path exists if the node is final
    if parikh_image.image.iter().all(|(_, v)| *v == 0) {
        assert_eq!(&valuation, final_valuation);
        return is_final;
    }

    let outgoing = cfg
        .graph
        .edges_directed(node_index, petgraph::Direction::Outgoing);

    for edge in outgoing {
        // first we check that the edge can still be taken
        let edge_index = edge.id();
        let Some(edge_count) = parikh_image.image.get(&edge_index) else {
            continue;
        };
        if *edge_count == 0 {
            continue;
        }

        // next we check that taking the edge does not make a counter in the valuation
        // negative
        let update = edge.weight();
        if n_run && !valuation.can_apply_cfg_update(update) {
            continue;
        }

        // we can take the edge, so we update the parikh image and the valuation
        let mut valuation = valuation.clone();
        valuation.apply_cfg_update(*update);

        let mut parikh = parikh_image.clone();
        parikh.image.insert(edge_index, edge_count - 1);

        if rec_can_build_run(parikh, valuation, final_valuation, cfg, edge.target(), n_run) {
            return true;
        }
    }

    false
}