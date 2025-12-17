use core::panic;

use initialized::InitializedVASS;
use petgraph::{
    Direction,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::automaton::{
    Automaton, AutomatonEdge, AutomatonNode, FromLetter, Frozen, ModifiableAutomaton,
    NodeAutomaton,
    vass::counter::{VASSCounterUpdate, VASSCounterValuation},
};

pub mod counter;
pub mod initialized;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VASSEdge<E: AutomatonEdge + FromLetter> {
    pub data: E,
    pub update: VASSCounterUpdate,
}

impl<E: AutomatonEdge + FromLetter> VASSEdge<E> {
    pub fn new(data: E, update: VASSCounterUpdate) -> Self {
        Self { data, update }
    }
}

// todo epsilon transitions
#[derive(Debug, Clone)]
pub struct VASS<N: AutomatonNode, E: AutomatonEdge + FromLetter> {
    pub graph: DiGraph<N, VASSEdge<E>>,
    pub alphabet: Vec<E::Letter>,
    pub dimension: usize,
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> VASS<N, E> {
    pub fn new(dimension: usize, alphabet: Vec<E::Letter>) -> Self {
        let graph = DiGraph::new();
        VASS {
            alphabet,
            graph,
            dimension,
        }
    }

    pub fn init(
        self,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        initial_node: NodeIndex<u32>,
        final_node: NodeIndex<u32>,
    ) -> InitializedVASS<N, E> {
        assert_eq!(
            initial_valuation.dimension(),
            self.dimension,
            "Initial valuation has to have the same length as the dimension"
        );
        assert_eq!(
            final_valuation.dimension(),
            self.dimension,
            "Final valuation has to have the same length as the dimension"
        );

        InitializedVASS {
            vass: self,
            initial_valuation,
            final_valuation,
            initial_node,
            final_node,
        }
    }

    pub fn state_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn transition_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> NodeAutomaton for VASS<N, E> {
    type NIndex = NodeIndex;
    type N = N;

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn get_node(&self, index: Self::NIndex) -> Option<&N> {
        self.graph.node_weight(index)
    }

    fn get_node_unchecked(&self, index: Self::NIndex) -> &N {
        &self.graph[index]
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Automaton for VASS<N, E> {
    type EIndex = EdgeIndex;

    type E = VASSEdge<E>;

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn get_edge(&self, index: Self::EIndex) -> Option<&VASSEdge<E>> {
        self.graph.edge_weight(index)
    }

    fn get_edge_unchecked(&self, index: Self::EIndex) -> &VASSEdge<E> {
        self.graph.edge_weight(index).unwrap()
    }

    fn edge_endpoints(&self, edge: Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)> {
        self.graph.edge_endpoints(edge)
    }

    fn edge_endpoints_unchecked(&self, edge: Self::EIndex) -> (Self::NIndex, Self::NIndex) {
        self.graph.edge_endpoints(edge).unwrap()
    }

    fn outgoing_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, Direction::Outgoing)
            .map(|edge| edge.id())
    }

    fn incoming_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, Direction::Incoming)
            .map(|edge| edge.id())
    }

    fn connecting_edge_indices(
        &self,
        from: Self::NIndex,
        to: Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex> {
        self.graph.edges_connecting(from, to).map(|edge| edge.id())
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> ModifiableAutomaton for VASS<N, E> {
    fn add_node(&mut self, data: N) -> Self::NIndex {
        self.graph.add_node(data)
    }

    fn add_edge(
        &mut self,
        from: Self::NIndex,
        to: Self::NIndex,
        label: VASSEdge<E>,
    ) -> Self::EIndex {
        assert_eq!(
            label.update.dimension(),
            self.dimension,
            "Update has to have the same dimension as the vass"
        );

        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!(
                    "Transition conflict, adding the new transition causes this automaton to no longer be a VASS, as VASS have to be deterministic. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}",
                    from, label, target, from, label, to
                );
            }
        }

        self.graph.add_edge(from, to, label)
    }

    fn remove_node(&mut self, node: Self::NIndex) {
        self.graph.remove_node(node);
    }

    fn remove_edge(&mut self, edge: Self::EIndex) {
        self.graph.remove_edge(edge);
    }

    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool,
    {
        for index in self.iter_node_indices().rev() {
            if !f(Frozen::from(&mut *self), index) {
                self.remove_node(index);
            }
        }
    }
}
