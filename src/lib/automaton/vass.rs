use std::collections::HashMap;

use petgraph::{graph::NodeIndex, stable_graph::StableDiGraph, visit::EdgeRef, Direction};
use primes::{PrimeSet, Sieve};

use super::{
    dfa::{DfaNodeData, DFA},
    modulo::ModuloDFA,
    nfa::NFA,
    utils::{mut_add_vec, neg_vec},
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

impl<N: AutNode, E: AutEdge> VASS<N, E> {
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

pub fn marking_to_vec(marking: &[i32]) -> Vec<i32> {
    let mut vec = vec![];

    for (d, m) in marking.iter().enumerate() {
        let label = if *m > 0 {
            (d + 1) as i32
        } else {
            -((d + 1) as i32)
        };

        for _ in 0..m.abs() {
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

impl<N: AutNode, E: AutEdge> InitializedVASS<'_, N, E> {
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
                let marking_vec = marking_to_vec(vass_label);

                if marking_vec.is_empty() {
                    cfg.add_transition(cfg_state, cfg_target, None);
                } else {
                    let mut cfg_source = cfg_state;

                    for label in marking_vec.iter().take(marking_vec.len() - 1) {
                        let target = cfg.add_state(DfaNodeData::new(false, None));
                        cfg.add_transition(cfg_source, target, Some(*label));
                        cfg_source = target;
                    }
    
                    let label = marking_vec[marking_vec.len() - 1];
                    cfg.add_transition(cfg_source, cfg_target, Some(label));
                }
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
        let mut cfg = self.to_cfg();
        cfg.add_failure_state(vec![]);
        // dbg!(&cfg);
        let min_cfg = cfg.minimize();
        // dbg!(&min_cfg);

        println!("Generated CFG with {} states", cfg.state_count());
        println!("Minimized CFG with {} states", min_cfg.state_count());

        let mut pset = Sieve::new();

        for mu in pset.iter() {
            if mu > 100 {
                panic!("No solution with mu < 100 found");
            }

            println!("Step mu = {}", mu);

            // IDEA: we only need the paths
            // we can probably do that by lazily generating the modulo dfa as we need it, as the modulo dfa is massive
            // maybe we can even generate the intersection lazily
            let aprox = ModuloDFA::new(self.vass.counter_count, mu as usize, true);
            // dbg!(&aprox);
            let intersection = min_cfg.intersect(aprox.dfa());
            // dbg!(&intersection);
            let reach_paths = intersection.bfs_accepting_states();

            println!("Found {} possible reaching paths", reach_paths.len());

            if reach_paths.is_empty() {
                return false;
            }

            for (i, path) in reach_paths.iter().enumerate() {
                print!("Path {}: ", i);

                // TODO: if the path goes into the negative, the prefix from the cfg
                if path.is_zero_reaching(self.vass.counter_count, |x| *intersection.edge_weight(x))
                {
                    println!("Zero reaching: {:?}", path);
                    return true;
                } else {
                    println!("Not zero reaching: {:?}", path);

                    if path.has_loop() {
                        let ltc =
                            path.to_ltc(self.vass.counter_count, |x| *intersection.edge_weight(x));

                        if ltc.reach_n() {
                            println!("LTC is reachable in N");
                            return true;
                        } else {
                            println!("LTC is not reachable in N");
                            // TODO: cut ltc from cfg
                        }
                    } else {
                        // TODO: cut path from cfg
                    }
                }
            }
        }

        true
    }
}

impl<N: AutNode, E: AutEdge> Automaton<E> for InitializedVASS<'_, N, E> {
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
