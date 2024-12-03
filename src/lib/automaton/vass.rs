use std::collections::HashMap;

use petgraph::{graph::NodeIndex, stable_graph::StableDiGraph, visit::EdgeRef, Direction};
use primes::{PrimeSet, Sieve};

use crate::automaton::{dfa::VASSCFG, path::ZeroReaching};

use super::{
    dfa::{DfaNodeData, DFA},
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
        let mut cfg = NFA::new(dimension_to_cfg_alphabet(self.vass.counter_count));

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
        let dimension = self.vass.counter_count;

        let time = std::time::Instant::now();

        println!();
        println!("--- VASS N-Reach ---");
        println!(
            "VASS: {:?} states, {:?} transitions",
            self.vass.graph.node_count(),
            self.vass.graph.edge_count()
        );
        println!("Dimension: {:?}", dimension);

        let mut cfg: DFA<Vec<Option<N>>, i32> = self.to_cfg();
        cfg.add_failure_state(vec![]);
        cfg = cfg.minimize();
        // dbg!(&cfg);
        // let min_cfg = cfg.minimize();
        // dbg!(&min_cfg);

        println!(
            "CFG: {:?} states, {:?} transitions",
            cfg.state_count(),
            cfg.graph.edge_count()
        );
        println!("Time to convert to CFG: {:?}", time.elapsed());
        println!("-----");

        // println!("Minimized CFG with {} states", min_cfg.state_count());

        let mut pset = Sieve::new();
        let mut piter = pset.iter();
        let mut mu = piter.next().unwrap();

        let mut step_time;

        let result;
        let mut step_count = 0;

        loop {
            step_count += 1;

            if mu > 100 {
                panic!("No solution with mu < 100 found");
            }

            step_time = std::time::Instant::now();

            println!();
            println!("--- Step: {} ---", step_count);
            println!("Mu: {}", mu);
            println!(
                "CFG: {:?} states, {:?} transitions",
                cfg.state_count(),
                cfg.graph.edge_count()
            );

            let reach_paths = cfg.modulo_reach(dimension, mu as i32);

            if reach_paths.is_empty() {
                result = false;
                break;
            }

            let path = &reach_paths[0];
            let zero_reaching = path.is_n_zero_reaching(dimension, |x| *cfg.edge_weight(x));

            if zero_reaching == ZeroReaching::ReachesZero {
                println!(
                    "Zero reaching: {:?}",
                    path.simple_print(|x| *cfg.edge_weight(x))
                );
                result = true;
                break;
            } else {
                println!(
                    "Not zero reaching: {:?}",
                    path.simple_print(|x| *cfg.edge_weight(x))
                );

                if path.has_loop() {
                    let (ltc, dfa) = path.to_ltc(dimension, |x| *cfg.edge_weight(x));

                    if ltc.reach_n() {
                        println!("LTC is reachable in N");

                        result = true;
                        break;
                    } else {
                        println!("LTC is not reachable in N");

                        // cfg.assert_complete();
                        // dfa.assert_complete();

                        cfg = cfg.intersect(&dfa);
                        cfg = cfg.minimize();
                    }
                } else if let ZeroReaching::FallsBelowZero(index) = zero_reaching {
                    let sliced_path = path.slice(index);
                    println!("Does not stay positive at index {:?}", index);
                    // dbg!(&sliced_path);
                    let dfa = sliced_path.simple_to_dfa(true, dimension, |x| *cfg.edge_weight(x));
                    // dbg!(&dfa);
                    cfg = cfg.intersect(&dfa);
                    // dbg!(&cfg);
                    cfg = cfg.minimize();
                    // dbg!(&cfg);
                } else {
                    mu = piter.next().unwrap();
                }
            }

            println!("Time for step: {:?}", step_time.elapsed());
        }

        println!();
        println!("--- Results ---");
        println!("Result: {}", result);
        println!("Step count: {}", step_count);
        println!("Time: {:?}", time.elapsed());
        println!("-----");
        println!();

        result
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

pub fn dimension_to_cfg_alphabet(dimension: usize) -> Vec<i32> {
    (1..=dimension as i32)
        .chain((1..=dimension as i32).map(|x| -x))
        .collect()
}
