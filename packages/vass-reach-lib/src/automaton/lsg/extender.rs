use std::fmt::Debug;

use petgraph::graph::NodeIndex;
use rand::{Rng, SeedableRng, rngs::StdRng};

use crate::{
    automaton::{
        cfg::{ExplicitEdgeCFG, update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath},
        lsg::{LinearSubGraph, part::LSGPart},
        path::Path,
        vass::counter::VASSCounterValuation,
    },
    solver::{
        SolverStatus,
        lsg_reach::{LSGReachSolverOptions, LSGSolution},
    },
};

/// Struct to iteratively extend a Linear Subgraph (LSG) by adding nodes chosen
/// by a `NodeChooser`, while keeping the LSG unreachable.
#[derive(Debug, Clone)]
pub struct LSGExtender<'a, C: ExplicitEdgeCFG, Strategy: ExtensionStrategy<C>> {
    /// The current Linear Subgraph being extended.
    pub lsg: LinearSubGraph<'a, C>,
    /// The previous Linear Subgraph before the last extension.
    /// This is used to backtrack if the current LSG becomes reachable.
    pub old_lsg: Option<LinearSubGraph<'a, C>>,
    /// Reference to the underlying CFG.
    pub cfg: &'a C,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// The strategy used to select nodes to add to the LSG.
    pub strategy: Strategy,
    /// Maximum number of refinement steps to perform.
    pub max_refinements: u64,
}

impl<'a, C: ExplicitEdgeCFG, Strategy: ExtensionStrategy<C>> LSGExtender<'a, C, Strategy> {
    pub fn new(
        path: Path<NodeIndex, CFGCounterUpdate>,
        cfg: &'a C,
        dimension: usize,
        strategy: Strategy,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        let lsg = LinearSubGraph::from_path(path, cfg, dimension);

        LSGExtender {
            old_lsg: None,
            lsg,
            dimension,
            cfg,
            strategy,
            initial_valuation,
            final_valuation,
            max_refinements,
        }
    }

    /// Refines `self.lsg` by trying to extend it using the `node_chooser`
    /// until no more nodes can be added or the maximum number of refinements is
    /// reached.
    pub fn run(&mut self) -> VASSCFG<()> {
        let mut refinement_step = 0;

        loop {
            let solver_result = LSGReachSolverOptions::default()
                .to_solver(&self.lsg, &self.initial_valuation, &self.final_valuation)
                .solve();

            match &solver_result.status {
                SolverStatus::True(solution) => {
                    // we became reachable, so we need to remove the last extension
                    let old = self.old_lsg.take();
                    self.lsg = old.expect("LSGExtender: Something went wrong, we became reachable but have no old LSG to backtrack to. Maybe the initial LSG was already reachable?");

                    self.strategy.on_rollback(solution);
                }
                SolverStatus::False(_) => {
                    // we are still unreachable, so we can try to extend the LSG
                    if let Some(extended) = self.strategy.extend(&self.lsg, refinement_step) {
                        self.old_lsg = Some(std::mem::replace(&mut self.lsg, extended));
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

        self.lsg.to_cfg()
    }
}

impl<'a, Chooser: ExtensionStrategy<VASSCFG<()>>> LSGExtender<'a, VASSCFG<()>, Chooser> {
    pub fn from_cfg_product(
        path: &MultiGraphPath,
        cfg_product: &'a ImplicitCFGProduct,
        node_chooser: Chooser,
        max_refinements: u64,
    ) -> Self {
        let path = path.to_path(cfg_product.main_cfg()).into();

        Self::new(
            path,
            &cfg_product.main_cfg(),
            cfg_product.dimension,
            node_chooser,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            max_refinements,
        )
    }
}

pub trait ExtensionStrategy<C: ExplicitEdgeCFG> {
    fn extend<'a>(
        &mut self,
        lsg: &LinearSubGraph<'a, C>,
        step: u64,
    ) -> Option<LinearSubGraph<'a, C>>;

    fn on_rollback(&mut self, solution: &LSGSolution);
}

pub struct RandomNodeStrategy<C: ExplicitEdgeCFG> {
    pub max_retries: usize,
    pub seed: u64,
    random: StdRng,
    pub blacklist: Vec<C::NIndex>,
    pub last_added: Option<C::NIndex>,
}

impl<C: ExplicitEdgeCFG> RandomNodeStrategy<C> {
    pub fn new(max_retries: usize, seed: u64) -> Self {
        RandomNodeStrategy {
            max_retries,
            seed,
            random: StdRng::seed_from_u64(seed),
            blacklist: Vec::new(),
            last_added: None,
        }
    }
}

impl<C: ExplicitEdgeCFG> ExtensionStrategy<C> for RandomNodeStrategy<C> {
    fn extend<'a>(
        &mut self,
        lsg: &LinearSubGraph<'a, C>,
        _step: u64,
    ) -> Option<LinearSubGraph<'a, C>> {
        for _ in 0..self.max_retries {
            let node = C::NIndex::new(self.random.gen_range(0..lsg.cfg.node_count()));
            if !lsg.contains_node(node) {
                continue;
            }

            let neighbors: Vec<_> = lsg.cfg.undirected_neighbors(node);

            let selected = neighbors
                .iter()
                .find(|n| !lsg.contains_node(**n) && !self.blacklist.contains(n));

            if let Some(selected) = selected {
                self.last_added = Some(*selected);
                return Some(lsg.add_node(*selected));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &LSGSolution) {
        if let Some(last_node) = self.last_added {
            self.blacklist.push(last_node);
        }
    }
}

pub struct RandomSCCStrategy<C: ExplicitEdgeCFG> {
    pub max_retries: usize,
    pub seed: u64,
    random: StdRng,
    pub blacklist: Vec<C::NIndex>,
    pub last_added: Vec<C::NIndex>,
}

impl<C: ExplicitEdgeCFG> RandomSCCStrategy<C> {
    pub fn new(max_retries: usize, seed: u64) -> Self {
        RandomSCCStrategy {
            max_retries,
            seed,
            random: StdRng::seed_from_u64(seed),
            blacklist: Vec::new(),
            last_added: Vec::new(),
        }
    }
}

impl<C: ExplicitEdgeCFG> ExtensionStrategy<C> for RandomSCCStrategy<C> {
    fn extend<'a>(
        &mut self,
        lsg: &LinearSubGraph<'a, C>,
        _step: u64,
    ) -> Option<LinearSubGraph<'a, C>> {
        for _ in 0..self.max_retries {
            let paths = lsg
                .parts
                .iter()
                .filter_map(|p| {
                    if let LSGPart::Path(path) = p
                        && path.path.len() > 1
                    {
                        Some(path)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            if paths.is_empty() {
                return None;
            }

            for _ in 0..self.max_retries {
                let path_index = self.random.gen_range(0..paths.len());
                let path = &paths[path_index];

                let node_index = self.random.gen_range(0..path.path.len());
                let node = path.path.get_node(node_index);

                if self.blacklist.contains(&node) {
                    continue;
                }

                self.last_added.push(node);
                return Some(lsg.add_scc_around_node(node));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &LSGSolution) {
        for last_node in &self.last_added {
            self.blacklist.push(*last_node);
        }
        self.last_added.clear();
    }
}
