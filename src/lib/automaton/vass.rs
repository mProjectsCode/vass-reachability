use std::{collections::HashMap, fmt::Debug, ops::Neg};

use petgraph::{graph::NodeIndex, stable_graph::StableDiGraph, visit::EdgeRef, Direction};
use primes::{PrimeSet, Sieve};

use super::{
    dfa::{DfaNodeData, DFA},
    modulo::ModuloDFA,
    nfa::NFA,
    AutBuild, AutEdge, AutNode, Automaton,
};

pub type VassEdge<E> = (E, Vec<i32>);

// todo epsilon transitions
#[derive(Debug, Clone)]
pub struct VASS<N: AutNode, E: AutEdge> {
    graph: StableDiGraph<N, VassEdge<E>>,
    alphabet: Vec<E>,
    counter_count: usize,
}

impl<N: Debug + Clone + PartialEq, E: AutEdge> VASS<N, E> {
    pub fn new(counter_count: usize, alphabet: Vec<E>) -> Self {
        let graph = StableDiGraph::new();
        VASS {
            alphabet,
            graph,
            counter_count,
        }
    }

    pub fn init(
        &self,
        initial_valuation: Vec<i32>,
        final_valuation: Vec<i32>,
        initial_node: NodeIndex<u32>,
        final_node: NodeIndex<u32>,
    ) -> InitializedVASS<N, E> {
        assert!(
            initial_valuation.len() == self.counter_count,
            "Initial valuation has to have the same length as the counter count"
        );
        assert!(
            final_valuation.len() == self.counter_count,
            "Final valuation has to have the same length as the counter count"
        );

        InitializedVASS {
            vass: self,
            initial_valuation,
            final_valuation,
            initial_node,
            final_node,
        }
    }
}

impl<N: AutNode, E: AutEdge> AutBuild<NodeIndex, N, VassEdge<E>> for VASS<N, E> {
    fn add_state(&mut self, data: N) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(&mut self, from: NodeIndex<u32>, to: NodeIndex<u32>, label: VassEdge<E>) {
        assert!(
            label.1.len() == self.counter_count,
            "Update has to have the same length as the counter count"
        );

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

pub fn marking_to_vec(marking: &Vec<i32>) -> Vec<i32> {
    let mut vec = vec![];

    for d in 0..marking.len() {
        let label = if marking[d] > 0 {
            (d + 1) as i32
        } else {
            -((d + 1) as i32)
        };

        for _ in 0..marking[d].abs() {
            vec.push(label);
        }
    }

    vec
}

#[derive(Debug, Clone)]
pub struct InitializedVASS<'a, N: AutNode, E: AutEdge> {
    vass: &'a VASS<N, E>,
    initial_valuation: Vec<i32>,
    final_valuation: Vec<i32>,
    initial_node: NodeIndex<u32>,
    final_node: NodeIndex<u32>,
}

impl<'a, N: AutNode, E: AutEdge> InitializedVASS<'a, N, E> {
    pub fn to_cfg(&self) -> DFA<Vec<Option<N>>, i32> {
        let cfg_alphabet = (1..=self.vass.counter_count as i32)
            .chain((1..=self.vass.counter_count as i32).map(|x| -x))
            .collect();
        let mut cfg = NFA::new(cfg_alphabet);

        let cfg_start = cfg.add_state(self.state_to_cfg_state(self.initial_node));
        cfg.set_start(cfg_start);

        let mut visited = HashMap::<NodeIndex, NodeIndex>::new();
        let mut stack = vec![(self.initial_node, cfg_start)];

        while let Some((vass_state, cfg_state)) = stack.pop() {
            visited.insert(vass_state, cfg_state);

            for vass_edge in self
                .vass
                .graph
                .edges_directed(vass_state, Direction::Outgoing)
            {
                let cfg_target = if let Some(&target) = visited.get(&vass_edge.target()) {
                    target
                } else {
                    let target = cfg.add_state(self.state_to_cfg_state(vass_edge.target()));
                    stack.push((vass_edge.target(), target));
                    target
                };

                let vass_label = &vass_edge.weight().1;

                assert!(
                    !vass_label.iter().all(|x| *x == 0),
                    "0 edge marking not implemented"
                );

                let marking_vec = marking_to_vec(vass_label);

                let mut cfg_source = cfg_state;

                for i in 0..marking_vec.len() - 1 {
                    let label = marking_vec[i];
                    let target = cfg.add_state(DfaNodeData::new(false, None));
                    cfg.add_transition(cfg_source, target, Some(label));
                    cfg_source = target;
                }

                let label = marking_vec[marking_vec.len() - 1];
                cfg.add_transition(cfg_source, cfg_target, Some(label));
            }
        }

        cfg.determinize()
    }

    fn state_to_cfg_state(&self, state: NodeIndex<u32>) -> DfaNodeData<Option<N>> {
        DfaNodeData::new(
            state == self.final_node,
            Some(self.node_data(state).clone()),
        )
    }

    pub fn node_data(&self, node: NodeIndex<u32>) -> &N {
        &self.vass.graph[node]
    }

    pub fn reach_1(&self) -> bool {
        let cfg = self.to_cfg();
        let mut pset = Sieve::new();

        for p in pset.iter() {
            if p > 100 {
                panic!("No solution with mu < 100 found");
            }

            let aprox = ModuloDFA::new(self.vass.counter_count, p as usize);
            if cfg.is_subset_of(aprox.dfa()) {
                return false;
            }
        }

        true
    }
}

fn neg_vec(vec: &Vec<i32>) -> Vec<i32> {
    vec.iter().map(|x| x.neg()).collect()
}

fn add_vec(a: &Vec<i32>, b: &Vec<i32>) -> Vec<i32> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

fn mut_add_vec(a: &mut Vec<i32>, b: &Vec<i32>) {
    for i in 0..a.len() {
        a[i] += b[i];
    }
}

impl<'a, N: AutNode, E: AutEdge> Automaton<E> for InitializedVASS<'a, N, E> {
    fn accepts(&self, input: &[E]) -> bool {
        let mut current_state = Some(self.initial_node);
        let mut current_valuation = self.initial_valuation.clone();

        for symbol in input {
            if let Some(state) = current_state {
                let next_state = self
                    .vass
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|neighbor| {
                        let edge = neighbor.weight();
                        // check that we can take the edge
                        edge.0 == *symbol && current_valuation >= neg_vec(&edge.1)
                    })
                    .map(|edge| {
                        // subtract the valuation of the edge from the current valuation
                        mut_add_vec(&mut current_valuation, &edge.weight().1);
                        edge.target()
                    });
                current_state = next_state;
            } else {
                return false;
            }
        }

        match current_state {
            Some(state) => state == self.final_node && current_valuation == self.final_valuation,
            None => false,
        }
    }

    fn alphabet(&self) -> &Vec<E> {
        &self.vass.alphabet
    }
}
