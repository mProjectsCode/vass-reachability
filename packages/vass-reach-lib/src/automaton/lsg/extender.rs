use std::fmt::Debug;

use petgraph::graph::NodeIndex;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::{
    automaton::{
        AutomatonNode, cfg::vasscfg::VASSCFG, lsg::LinearSubGraph, path::Path,
        vass::counter::VASSCounterValuation,
    },
    solver::{SolverStatus, lsg_reach::LSGReachSolverOptions},
};

/// Struct to iteratively extend a Linear Subgraph (LSG) by adding nodes chosen
/// by a `NodeChooser`, while keeping the LSG unreachable.
#[derive(Debug, Clone)]
pub struct LSGExtender<'a, N: AutomatonNode, Chooser: NodeChooser<N>> {
    /// The current Linear Subgraph being extended.
    pub lsg: LinearSubGraph<'a, N>,
    /// The previous Linear Subgraph before the last extension.
    /// This is used to backtrack if the current LSG becomes reachable.
    pub old_lsg: Option<LinearSubGraph<'a, N>>,
    /// The last node added to the LSG during the extension process.
    /// We use this to blacklist nodes that lead to reachability, when we
    /// backtrack.
    pub last_added_node: Option<NodeIndex>,
    /// Blacklisted nodes that should not be added to the LSG, because they led
    /// to reachability in previous attempts.
    pub blacklist: Vec<NodeIndex>,
    /// Reference to the underlying CFG.
    pub cfg: &'a VASSCFG<N>,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// The node chooser used to select nodes to add to the LSG.
    pub node_chooser: Chooser,
    /// Maximum number of refinement steps to perform.
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
            old_lsg: None,
            lsg,
            last_added_node: None,
            dimension,
            cfg,
            node_chooser,
            initial_valuation,
            final_valuation,
            max_refinements,
            blacklist: Vec::new(),
        }
    }

    /// Refines `self.lsg` by trying to extend it using the `node_chooser`
    /// until no more nodes can be added or the maximum number of refinements is
    /// reached.
    pub fn run(&mut self) {
        let mut refinement_step = 0;

        loop {
            let solver_result = LSGReachSolverOptions::default()
                .to_solver(&self.lsg, &self.initial_valuation, &self.final_valuation)
                .solve();

            match &solver_result.status {
                SolverStatus::True(_) => {
                    // we became reachable, so we need to remove the last extension
                    let old = self.old_lsg.take();
                    self.lsg = old.expect("LSGExtender: Something went wrong, we became reachable but have no old LSG to backtrack to. Maybe the initial LSG was already reachable?");

                    // we also blacklist the last added node
                    let last_node = self.last_added_node.take();
                    self.blacklist.push(
                        last_node.expect("LSGExtender: Something went wrong, we were able to backtrack but have no last added node to blacklist."),
                    );
                }
                SolverStatus::False(_) => {
                    // we are still unreachable, so we can try to extend the LSG
                    if let Some(node_index) =
                        self.node_chooser
                            .choose_node(&self.lsg, refinement_step, &self.blacklist)
                    {
                        self.last_added_node = Some(node_index);
                        let extended_lsg = self.lsg.add_node(node_index);
                        self.old_lsg = Some(std::mem::replace(&mut self.lsg, extended_lsg));
                    } else {
                        // No more nodes to extend, we can stop
                        break;
                    }
                }
                SolverStatus::Unknown(_) => {
                    // Solver returned unknown, we just stop here
                    break;
                }
            }

            refinement_step += 1;
            if refinement_step >= self.max_refinements {
                break;
            }
        }
    }
}

pub trait NodeChooser<N: AutomatonNode> {
    fn choose_node(
        &mut self,
        lsg: &LinearSubGraph<N>,
        step: usize,
        black_list: &[NodeIndex],
    ) -> Option<NodeIndex>;
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
    fn choose_node(
        &mut self,
        lsg: &LinearSubGraph<N>,
        _step: usize,
        black_list: &[NodeIndex],
    ) -> Option<NodeIndex> {
        // first pick a node in the lsg at random, then pick one of its neighbors at
        // random if the chosen neighbor is already in the lsg, retry a fixed
        // number of times

        for _ in 0..self.max_retries {
            let node_index = NodeIndex::new(self.random.gen_range(0..lsg.cfg.state_count()));
            if !lsg.contains_node(node_index) {
                continue;
            }

            let neighbors: Vec<_> = lsg.cfg.graph.neighbors(node_index).collect();

            let node = neighbors
                .iter()
                .find(|n| !lsg.contains_node(**n) && !black_list.contains(n));

            if let Some(node) = node {
                return Some(*node);
            }
        }

        None
    }
}
