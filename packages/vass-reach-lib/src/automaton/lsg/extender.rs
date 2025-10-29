use std::fmt::Debug;

use petgraph::graph::NodeIndex;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::{
    automaton::{
        AutomatonNode, dfa::cfg::VASSCFG, lsg::LinearSubGraph, path::Path,
        vass::counter::VASSCounterValuation,
    },
    solver::{SolverStatus, lsg_reach::LSGReachSolverOptions},
};

#[derive(Debug, Clone)]
pub struct LSGExtender<'a, N: AutomatonNode, Chooser: NodeChooser<N>> {
    pub lsgs: Vec<LinearSubGraph<'a, N>>,
    pub dimension: usize,
    pub cfg: &'a VASSCFG<N>,
    pub node_chooser: Chooser,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    pub max_refinements: usize,
}

impl<'a, N: AutomatonNode, Chooser: NodeChooser<N>> LSGExtender<'a, N, Chooser> {
    pub fn new(
        path: Path,
        cfg: &'a VASSCFG<N>,
        dimension: usize,
        node_chooser: Chooser,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: usize,
    ) -> Self {
        let lsg = LinearSubGraph::from_path(path, cfg, dimension);

        LSGExtender {
            lsgs: vec![lsg],
            dimension,
            cfg,
            node_chooser,
            initial_valuation,
            final_valuation,
            max_refinements,
        }
    }

    pub fn run(&mut self) -> LinearSubGraph<'a, N> {
        let mut refinement_step = 0;

        loop {
            let lsg = self.lsgs.last().unwrap();
            let mut solver = LSGReachSolverOptions::default().to_solver(
                lsg,
                &self.initial_valuation,
                &self.final_valuation,
            );
            let solver_result = solver.solve();

            match &solver_result.status {
                SolverStatus::True(_) => {
                    // we became reachable, so we need to remove the last extension
                    self.lsgs.pop();
                }
                SolverStatus::False(_) => {
                    // we are still unreachable, so we can try to extend the LSG
                    if let Some(node_index) = self.node_chooser.choose_node(lsg) {
                        let extended_lsg = lsg.add_node(node_index);
                        self.lsgs.push(extended_lsg);
                    } else {
                        // No more nodes to extend, we can stop
                        break;
                    }
                }
                SolverStatus::Unknown(_) => {
                    // Solver returned unknown, handle accordingly
                    break;
                }
            }

            refinement_step += 1;
            if refinement_step >= self.max_refinements {
                break;
            }
        }

        self.lsgs.pop().unwrap()
    }
}

pub trait NodeChooser<N: AutomatonNode> {
    fn choose_node(&mut self, lsg: &LinearSubGraph<N>) -> Option<NodeIndex>;
}

pub struct RandomNodeChooser {
    pub max_retries: usize,
    pub seed: u64,
    random: StdRng,
}

impl RandomNodeChooser {
    pub fn new(max_retries: usize, seed: u64) -> Self {
        RandomNodeChooser {
            max_retries,
            seed,
            random: StdRng::seed_from_u64(seed),
        }
    }
}

impl<N: AutomatonNode> NodeChooser<N> for RandomNodeChooser {
    fn choose_node(&mut self, lsg: &LinearSubGraph<N>) -> Option<NodeIndex> {
        // first pick a node in the lsg at random, then pick one of its neighbors at
        // random if the chosen neighbor is already in the lsg, retry a fixed
        // number of times

        for _ in 0..self.max_retries {
            let node_index = NodeIndex::new(self.random.gen_range(0..lsg.cfg.state_count()));
            if !lsg.contains_node(node_index) {
                continue;
            }

            let neighbors: Vec<_> = lsg.cfg.graph.neighbors(node_index).collect();

            let node = neighbors.iter().find(|n| !lsg.contains_node(**n));

            if let Some(node) = node {
                return Some(*node);
            }
        }

        None
    }
}
