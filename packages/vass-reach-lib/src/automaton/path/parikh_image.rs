use hashbrown::HashSet;
use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::{
    automaton::{
        cfg::{CFG, update::CFGCounterUpdatable},
        index_map::IndexMap,
        path::{Path, path_like::PathLike, transition_sequence::TransitionSequence},
        vass::counter::{VASSCounterUpdate, VASSCounterValuation},
    },
    logger::{LogLevel, Logger},
};

#[derive(Debug, Clone)]
pub struct ParikhImage {
    pub image: IndexMap<EdgeIndex, u32>,
}

impl ParikhImage {
    pub fn new(image: IndexMap<EdgeIndex, u32>) -> Self {
        ParikhImage { image }
    }

    pub fn empty(edge_count: usize) -> Self {
        ParikhImage {
            image: IndexMap::new(edge_count),
        }
    }

    pub fn print(&self, logger: &Logger, level: LogLevel) {
        for (edge, count) in self.image.iter() {
            logger.log(level.clone(), &format!("Edge: {}: {}", edge.index(), count));
        }
    }

    pub fn get(&self, edge: EdgeIndex) -> u32 {
        *self.image.get(edge)
    }

    pub fn set(&mut self, edge: EdgeIndex, count: u32) {
        self.image.insert(edge, count);
    }

    pub fn add_to(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        *entry += count;
    }

    pub fn sub_from(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        if *entry < count {
            *entry = 0;
        } else {
            *entry -= count;
        }
    }

    /// Set an edge to the maximum of the current count and the given count.
    pub fn set_max(&mut self, edge: EdgeIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        *entry = count.max(*entry);
    }

    pub fn is_empty(&self) -> bool {
        self.image.iter().all(|(_, v)| *v == 0)
    }

    /// Split the Parikh Image into possibly multiple connected components.
    /// The main connected component is the one that contains the start node.
    /// The connected components are determined by a depth-first search.
    pub fn split_into_connected_components(
        mut self,
        cfg: &impl CFG,
    ) -> (ParikhImage, Vec<ParikhImage>) {
        let mut components = vec![];
        let mut visited = vec![false; cfg.state_count()];

        let main_component = self.split_connected_component(cfg, cfg.get_start(), &mut visited);

        for node in cfg.get_graph().node_indices() {
            if visited[node.index()] {
                continue;
            }

            let component = self.split_connected_component(cfg, node, &mut visited);
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
    fn split_connected_component(
        &mut self,
        cfg: &impl CFG,
        start: NodeIndex,
        visited: &mut [bool],
    ) -> ParikhImage {
        let mut stack = vec![start];
        let mut component = ParikhImage::empty(self.image.size());

        while let Some(node) = stack.pop() {
            if visited[node.index()] {
                continue;
            }

            visited[node.index()] = true;

            for e in cfg.get_graph().edges(node) {
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
    pub fn get_outgoing_edges(&self, cfg: &impl CFG) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(cfg);

        let mut edges = HashSet::new();

        // next we get all edges that go from a connected node to a node outside the
        // connected component
        for node in &connected_nodes {
            for edge in cfg.get_graph().edges(*node) {
                if !connected_nodes.contains(&edge.target()) {
                    edges.insert(edge.id());
                }
            }
        }

        edges
    }

    pub fn get_incoming_edges(&self, cfg: &impl CFG) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(cfg);

        let mut edges = HashSet::new();

        // next we get all edges that go from a node outside the connected component
        // to a connected node
        for node in &connected_nodes {
            for edge in cfg
                .get_graph()
                .edges_directed(*node, petgraph::Direction::Incoming)
            {
                if !connected_nodes.contains(&edge.source()) {
                    edges.insert(edge.id());
                }
            }
        }

        edges
    }

    pub fn get_connected_nodes(&self, cfg: &impl CFG) -> HashSet<NodeIndex> {
        let mut connected_nodes = HashSet::new();

        for edge in cfg.get_graph().edge_references() {
            if self.get(edge.id()) == 0 {
                continue;
            }

            connected_nodes.insert(edge.source());
            connected_nodes.insert(edge.target());
        }

        connected_nodes
    }

    pub fn build_run(
        &self,
        cfg: &impl CFG,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
        n_run: bool,
    ) -> Option<Path> {
        let valuation = initial_valuation.clone();

        let ts = rec_build_run(
            self.clone(),
            cfg,
            cfg.get_start(),
            valuation,
            final_valuation,
            n_run,
        );

        if let Some(mut transition_sequence) = ts {
            transition_sequence.reverse();
            Some(Path::new_from_sequence(
                cfg.get_start(),
                transition_sequence,
            ))
        } else {
            None
        }
    }

    pub fn get_total_counter_effect(&self, cfg: &impl CFG, dimension: usize) -> VASSCounterUpdate {
        let mut total_effect = VASSCounterUpdate::zero(dimension);

        for (edge_index, count) in self.image.iter() {
            let edge = cfg.edge_update(edge_index);

            total_effect[edge.counter()] += edge.op() * (*count as i32);
        }

        total_effect
    }

    pub fn iter(&self) -> impl Iterator<Item = (EdgeIndex, u32)> + use<'_> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, count)| (edge, *count))
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = EdgeIndex> + use<'_> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, _)| edge)
    }

    pub fn from_path(path: &Path, edge_count: usize) -> Self {
        let mut map = IndexMap::new(edge_count);

        for (edge, _) in &path.transitions {
            let entry = map.get_mut(*edge);
            *entry += 1;
        }

        ParikhImage::new(map)
    }
}

fn rec_build_run(
    parikh_image: ParikhImage,
    cfg: &impl CFG,
    node_index: NodeIndex,
    valuation: VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    n_run: bool,
) -> Option<TransitionSequence> {
    // if the parikh image is empty, we have reached the end of the path, which also
    // means that the path exists if the node is final
    if parikh_image.image.iter().all(|(_, v)| *v == 0) {
        assert_eq!(&valuation, final_valuation);
        return if cfg.is_accepting(node_index) {
            Some(TransitionSequence::new())
        } else {
            None
        };
    }

    let outgoing = cfg
        .get_graph()
        .edges_directed(node_index, petgraph::Direction::Outgoing);

    for edge in outgoing {
        // first we check that the edge can still be taken
        let edge_index = edge.id();
        let edge_count = parikh_image.image.get(edge_index);
        if *edge_count == 0 {
            continue;
        }

        // next we check that taking the edge does not make a counter in the valuation
        // negative
        let update = cfg.edge_update(edge_index);
        if n_run && !valuation.can_apply_cfg_update(&update) {
            continue;
        }

        // we can take the edge, so we update the parikh image and the valuation
        let mut valuation = valuation.clone();
        valuation.apply_cfg_update(update);

        let mut parikh = parikh_image.clone();
        parikh.image.insert(edge_index, edge_count - 1);

        let res = rec_build_run(
            parikh,
            cfg,
            edge.target(),
            valuation,
            final_valuation,
            n_run,
        );

        match res {
            Some(mut seq) => {
                seq.add(edge_index, edge.target());
                return Some(seq);
            }
            None => {
                // try next edge
            }
        }
    }

    None
}
