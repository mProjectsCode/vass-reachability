use initialized::InitializedVASS;
use petgraph::{
    Direction,
    graph::{EdgeIndex, NodeIndex},
    stable_graph::StableDiGraph,
    visit::EdgeRef,
};

use crate::automaton::{
    AutBuild, AutomatonEdge, AutomatonNode,
    vass::counter::{VASSCounterUpdate, VASSCounterValuation},
};

pub mod counter;
pub mod initialized;

pub type VASSEdge<E> = (E, VASSCounterUpdate);

// todo epsilon transitions
#[derive(Debug, Clone)]
pub struct VASS<N: AutomatonNode, E: AutomatonEdge> {
    pub graph: StableDiGraph<N, VASSEdge<E>>,
    pub alphabet: Vec<E>,
    pub dimension: usize,
}

impl<N: AutomatonNode, E: AutomatonEdge> VASS<N, E> {
    pub fn new(dimension: usize, alphabet: Vec<E>) -> Self {
        let graph = StableDiGraph::new();
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

impl<N: AutomatonNode, E: AutomatonEdge> AutBuild<NodeIndex, EdgeIndex, N, VASSEdge<E>>
    for VASS<N, E>
{
    fn add_state(&mut self, data: N) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(
        &mut self,
        from: NodeIndex<u32>,
        to: NodeIndex<u32>,
        label: VASSEdge<E>,
    ) -> EdgeIndex<u32> {
        assert_eq!(
            label.1.dimension(),
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
}
