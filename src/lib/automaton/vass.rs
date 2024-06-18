use std::fmt::Debug;

use petgraph::{graph::NodeIndex, stable_graph::StableDiGraph, visit::EdgeRef, Direction};

use super::{dfa::{DfaNodeData, DFA}, AutEdge, AutNode, Automaton, AutBuild};

pub type VassEdge<E, const D: usize> = (E, [i32; D]);

#[derive(Debug, Clone)]
pub struct VASS<N: AutNode, E: AutEdge, const D: usize> {
    start: NodeIndex<u32>,
    graph: StableDiGraph<DfaNodeData<N>, VassEdge<E, D>>,
    alphabet: Vec<E>,
}

impl<N: Debug + Clone + PartialEq, E: AutEdge, const D: usize> VASS<N, E, D> {
    pub fn new(alphabet: Vec<E>, start_accepting: bool, data: N) -> Self {
        let mut graph = StableDiGraph::new();
        let start = graph.add_node(DfaNodeData::new(start_accepting, data));

        VASS { alphabet,  start, graph }
    }

    pub fn new_from_data(alphabet: Vec<E>, data: DfaNodeData<N>) -> Self {
        let mut graph = StableDiGraph::new();
        let start = graph.add_node(data);

        VASS { alphabet, start, graph }
    }

    pub fn init(&self, initial_valuation: [i32; D], final_valuation: [i32; D]) -> InitializedVASS<N, E, D> {
        InitializedVASS {
            vass: self,
            initial_valuation,
            final_valuation,
        }
    }

    pub fn start(&self) -> NodeIndex<u32> {
        self.start
    }

    /// Control flow language, not context-free language
    pub fn to_cfl(&self) -> DFA<N, E> {
        // we probably need a DFA trait and then blanket implement things like invert, intersection, etc.
        // that way we can reuse the graph here and don't need to construct a separate DFA
        todo!()
    }
}

impl<N: AutNode, E: AutEdge, const D: usize>
    AutBuild<NodeIndex, DfaNodeData<N>, VassEdge<E, D>> for VASS<N, E, D> 
{
    fn add_state(&mut self, data: DfaNodeData<N>) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(&mut self, from: NodeIndex<u32>, to: NodeIndex<u32>, label: VassEdge<E, D>) {
        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!("Transition conflict, adding the new transition causes this automaton to no longer be a VASS, as VASS have to be deterministic. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}", from, label, target, from, label, to);
            }
        }

        self.graph.add_edge(from, to, label);
    }
}

pub fn add_arrays<const D: usize>(lhs: [i32; D], rhs: [i32; D]) -> [i32; D] {
    let mut lhs = lhs;
    for i in 0..D {
        lhs[i] += rhs[i];
    }
    lhs
}

pub fn sub_arrays<const D: usize>(mut lhs: [i32; D], rhs: [i32; D]) -> [i32; D] {
    for i in 0..D {
        lhs[i] -= rhs[i];
    }
    lhs
}

pub fn neg_array<const D: usize>(arr: [i32; D]) -> [i32; D] {
    let mut arr = arr;
    for i in 0..D {
        arr[i] = -arr[i];
    }
    arr
}

#[derive(Debug, Clone)]
pub struct InitializedVASS<'a, N: AutNode, E: AutEdge, const D: usize> {
    vass: &'a VASS<N, E, D>,
    initial_valuation: [i32; D],
    final_valuation: [i32; D],
}

impl<'a, N: AutNode, E: AutEdge, const D: usize>
    Automaton<NodeIndex, E> for InitializedVASS<'a, N, E, D>
{
    fn accepts(&self, input: &[E]) -> bool {
        let mut current_state = Some(self.start());
        let mut current_valuation = self.initial_valuation.clone();

        for symbol in input {
            if let Some(state) = current_state {
                let next_state = self.vass
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|neighbor| {
                        let edge = neighbor.weight();
                        // check that we can take the edge
                        edge.0 == *symbol && current_valuation >= neg_array(edge.1)
                    })
                    .map(|edge| {
                        // subtract the valuation of the edge from the current valuation
                        current_valuation = add_arrays(current_valuation, edge.weight().1);
                        edge.target()
                    });
                current_state = next_state;
            } else {
                return false;
            }
        }

        match current_state.and_then(|state| self.vass.graph.node_weight(state)) {
            Some(data) => data.accepting && current_valuation == self.final_valuation,
            None => false,
        }
    }

    fn start(&self) -> NodeIndex {
        self.vass.start
    }
}