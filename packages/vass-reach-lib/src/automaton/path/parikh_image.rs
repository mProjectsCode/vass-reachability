use hashbrown::HashSet;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::automaton::{
    AutomatonIterators, CompactGIndex, GIndex,
    cfg::{
        ExplicitEdgeCFG,
        update::{CFGCounterUpdatable, CFGCounterUpdate},
    },
    index_map::{IndexMap, IndexSet},
    path::{Path, transition_sequence::TransitionSequence},
    vass::counter::{VASSCounterUpdate, VASSCounterValuation},
};

#[derive(Debug, Clone)]
pub struct ParikhImage<EIndex: CompactGIndex> {
    pub image: IndexMap<EIndex, u32>,
}

impl<EIndex: CompactGIndex> ParikhImage<EIndex> {
    pub fn new(image: IndexMap<EIndex, u32>) -> Self {
        ParikhImage { image }
    }

    pub fn empty(edge_count: usize) -> Self {
        ParikhImage {
            image: IndexMap::new(edge_count),
        }
    }

    pub fn get(&self, edge: EIndex) -> u32 {
        *self.image.get(edge)
    }

    pub fn set(&mut self, edge: EIndex, count: u32) {
        self.image.insert(edge, count);
    }

    pub fn add_to(&mut self, edge: EIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        *entry += count;
    }

    pub fn sub_from(&mut self, edge: EIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        if *entry < count {
            *entry = 0;
        } else {
            *entry -= count;
        }
    }

    /// Set an edge to the maximum of the current count and the given count.
    pub fn set_max(&mut self, edge: EIndex, count: u32) {
        let entry = self.image.get_mut(edge);
        *entry = count.max(*entry);
    }

    pub fn is_empty(&self) -> bool {
        self.image.iter().all(|(_, v)| *v == 0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (EIndex, u32)> + use<'_, EIndex> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, count)| (edge, *count))
    }

    pub fn iter_edges(&self) -> impl Iterator<Item = EIndex> + use<'_, EIndex> {
        self.image
            .iter()
            .filter(|(_, count)| **count > 0)
            .map(|(edge, _)| edge)
    }

    pub fn from_path<NIndex: GIndex>(path: &Path<NIndex, EIndex>, edge_count: usize) -> Self {
        let mut map = IndexMap::new(edge_count);

        for (edge, _) in &path.transitions {
            let entry = map.get_mut(*edge);
            *entry += 1;
        }

        ParikhImage::new(map)
    }
}

impl ParikhImage<EdgeIndex> {
    /// Split the Parikh Image into possibly multiple connected components.
    /// The main connected component is the one that contains the start node.
    /// The connected components are determined by a depth-first search.
    pub fn split_into_connected_components<C: ExplicitEdgeCFG>(
        mut self,
        cfg: &C,
    ) -> (Self, Vec<Self>) {
        let mut components = vec![];
        let mut visited = IndexSet::new(cfg.node_count());

        let main_component = self.split_connected_component(cfg, cfg.get_initial(), &mut visited);

        for node in cfg.iter_node_indices() {
            if visited[node] {
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
    fn split_connected_component<C: ExplicitEdgeCFG>(
        &mut self,
        cfg: &C,
        start: C::NIndex,
        visited: &mut IndexSet<C::NIndex>,
    ) -> Self {
        let mut stack = vec![start];
        let mut component = ParikhImage::empty(self.image.size());

        while let Some(node) = stack.pop() {
            if visited[node] {
                continue;
            }

            visited[node] = true;

            for edge in cfg.outgoing_edge_indices(&node) {
                if self.get(edge) == 0 {
                    continue;
                }

                let count = self.get(edge);
                self.set(edge, 0);
                component.set_max(edge, count);

                let target = cfg.edge_target_unchecked(&edge);
                if !visited[target] {
                    stack.push(target);
                }
            }
        }

        component
    }

    /// Get the edges that go from the connected components, formed by this
    /// parikh image, to the outside. So from a node that is connected to by
    /// one edge of the parikh image to a node that is not connected.
    pub fn get_outgoing_edges<C: ExplicitEdgeCFG>(&self, cfg: &C) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(cfg);

        let mut edges = HashSet::new();

        // next we get all edges that go from a connected node to a node outside the
        // connected component
        for node in &connected_nodes {
            for edge in cfg.outgoing_edge_indices(node) {
                if !connected_nodes.contains(&cfg.edge_target_unchecked(&edge)) {
                    edges.insert(edge);
                }
            }
        }

        edges
    }

    pub fn get_incoming_edges<C: ExplicitEdgeCFG>(&self, cfg: &C) -> HashSet<EdgeIndex> {
        let connected_nodes = self.get_connected_nodes(cfg);

        let mut edges = HashSet::new();

        // next we get all edges that go from a node outside the connected component
        // to a connected node
        for node in &connected_nodes {
            for edge in cfg.incoming_edge_indices(node) {
                if !connected_nodes.contains(&cfg.edge_source_unchecked(&edge)) {
                    edges.insert(edge);
                }
            }
        }

        edges
    }

    pub fn get_connected_nodes<C: ExplicitEdgeCFG>(&self, cfg: &C) -> HashSet<C::NIndex> {
        let mut connected_nodes = HashSet::new();

        for edge in cfg.iter_edge_indices() {
            if self.get(edge) == 0 {
                continue;
            }

            let (source, target) = cfg.edge_endpoints_unchecked(&edge);

            connected_nodes.insert(source);
            connected_nodes.insert(target);
        }

        connected_nodes
    }

    pub fn build_run<C: ExplicitEdgeCFG>(
        &self,
        cfg: &C,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
        n_run: bool,
    ) -> Option<Path<C::NIndex, CFGCounterUpdate>> {
        let valuation = initial_valuation.clone();

        let ts = rec_build_run(
            self.clone(),
            cfg,
            cfg.get_initial(),
            valuation,
            final_valuation,
            n_run,
        );

        if let Some(mut transition_sequence) = ts {
            transition_sequence.reverse();
            Some(Path::new_from_sequence(
                cfg.get_initial(),
                transition_sequence,
            ))
        } else {
            None
        }
    }

    pub fn get_total_counter_effect<C: ExplicitEdgeCFG>(
        &self,
        cfg: &C,
        dimension: usize,
    ) -> VASSCounterUpdate {
        let mut total_effect = VASSCounterUpdate::zero(dimension);

        for (edge_index, count) in self.image.iter() {
            let edge = cfg.get_edge_unchecked(&edge_index);

            total_effect[edge.counter()] += edge.op() * (*count as i32);
        }

        total_effect
    }
}

fn rec_build_run<C: ExplicitEdgeCFG>(
    parikh_image: ParikhImage<EdgeIndex>,
    cfg: &C,
    node_index: NodeIndex,
    valuation: VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    n_run: bool,
) -> Option<TransitionSequence<NodeIndex, CFGCounterUpdate>> {
    // if the parikh image is empty, we have reached the end of the path, which also
    // means that the path exists if the node is final
    if parikh_image.image.iter().all(|(_, v)| *v == 0) {
        assert_eq!(&valuation, final_valuation);
        return if cfg.is_accepting(&node_index) {
            Some(TransitionSequence::new())
        } else {
            None
        };
    }

    for edge in cfg.outgoing_edge_indices(&node_index) {
        // first we check that the edge can still be taken
        let edge_count = parikh_image.image.get(edge);
        if *edge_count == 0 {
            continue;
        }

        // next we check that taking the edge does not make a counter in the valuation
        // negative
        let update = cfg.get_edge_unchecked(&edge);
        if n_run && !valuation.can_apply_cfg_update(update) {
            continue;
        }

        // we can take the edge, so we update the parikh image and the valuation
        let mut valuation = valuation.clone();
        valuation.apply_cfg_update(*update);

        let mut parikh = parikh_image.clone();
        parikh.image.insert(edge, edge_count - 1);

        let target = cfg.edge_target_unchecked(&edge);

        let res = rec_build_run(parikh, cfg, target, valuation, final_valuation, n_run);

        match res {
            Some(mut seq) => {
                seq.add(*update, target);
                return Some(seq);
            }
            None => {
                // try next edge
            }
        }
    }

    None
}
