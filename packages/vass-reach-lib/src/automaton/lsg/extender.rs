use std::fmt::Debug;

use petgraph::graph::NodeIndex;

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
}

impl<'a, N: AutomatonNode, Chooser: NodeChooser<N>> LSGExtender<'a, N, Chooser> {
    pub fn new(
        path: Path,
        cfg: &'a VASSCFG<N>,
        dimension: usize,
        node_chooser: Chooser,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
    ) -> Self {
        let lsg = LinearSubGraph::from_path(path, cfg, dimension);

        LSGExtender {
            lsgs: vec![lsg],
            dimension,
            cfg,
            node_chooser,
            initial_valuation,
            final_valuation,
        }
    }

    pub fn run(&mut self) -> LinearSubGraph<'a, N> {
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
        }

        self.lsgs.pop().unwrap()
    }
}

pub trait NodeChooser<N: AutomatonNode> {
    fn choose_node(&self, lsg: &LinearSubGraph<N>) -> Option<NodeIndex>;
}

pub struct RandomNodeChooser {
    pub max_retries: usize,
    pub seed: u64,
}

impl<N: AutomatonNode> NodeChooser<N> for RandomNodeChooser {
    fn choose_node(&self, lsg: &LinearSubGraph<N>) -> Option<NodeIndex> {
        // first pick a node in the lsg at random, then pick one of its neighbors at
        // random if the chosen neighbor is already in the lsg, retry a fixed
        // number of times

        unimplemented!()
    }
}
