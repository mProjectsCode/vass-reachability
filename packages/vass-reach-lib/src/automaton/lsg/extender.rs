use std::fmt::Debug;

use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::{
    automaton::{
        TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        lsg::{LinearSubGraph, part::LSGPart},
        path::Path,
        vass::counter::VASSCounterValuation,
    },
    config::ExtensionStrategyConfig,
    solver::{
        SolverStatus,
        lsg_reach::{LSGReachSolverOptions, LSGSolution},
    },
};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

/// Struct to iteratively extend a Linear Subgraph (LSG) by adding nodes chosen
/// by a `NodeChooser`, while keeping the LSG unreachable.
#[derive(Debug)]
pub struct LSGExtender<'a> {
    /// The current Linear Subgraph being extended.
    pub lsg: LinearSubGraph<'a>,
    /// The previous Linear Subgraph before the last extension.
    /// This is used to backtrack if the current LSG becomes reachable.
    pub old_lsg: Option<LinearSubGraph<'a>>,
    /// Reference to the underlying CFG.
    pub product: &'a ImplicitCFGProduct,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// The strategy used to select nodes to add to the LSG.
    pub strategy: ExtensionStrategyEnum,
    /// Maximum number of refinement steps to perform.
    pub max_refinements: u64,
}

impl<'a> LSGExtender<'a> {
    pub fn new(
        path: MultiGraphPath,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        strategy: ExtensionStrategyEnum,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        let lsg = LinearSubGraph::from_path(path, product, dimension);

        LSGExtender {
            old_lsg: None,
            lsg,
            dimension,
            product,
            strategy,
            initial_valuation,
            final_valuation,
            max_refinements,
        }
    }

    pub fn from_cfg_product(
        path: MultiGraphPath,
        cfg_product: &'a ImplicitCFGProduct,
        node_chooser: ExtensionStrategyEnum,
        max_refinements: u64,
    ) -> Self {
        Self::new(
            path,
            cfg_product,
            cfg_product.dimension,
            node_chooser,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            max_refinements,
        )
    }

    /// Refines `self.lsg` by trying to extend it using the `node_chooser`
    /// until no more nodes can be added or the maximum number of refinements is
    /// reached.
    pub fn run(&mut self) -> VASSCFG<()> {
        tracing::span!(tracing::Level::DEBUG, "LSGExtender::run");

        let mut refinement_step = 0;

        loop {
            let solver_result = LSGReachSolverOptions::default()
                .to_solver(&self.lsg, &self.initial_valuation, &self.final_valuation)
                .solve();

            match &solver_result.status {
                SolverStatus::True(solution) => {
                    tracing::debug!(
                        "LSGExtender step {}: LSG became reachable, rolling back",
                        refinement_step
                    );

                    // we became reachable, so we need to remove the last extension
                    let old = self.old_lsg.take();
                    self.lsg = old.expect("LSGExtender: Something went wrong, we became reachable but have no old LSG to backtrack to. Maybe the initial LSG was already reachable?");

                    self.strategy.on_rollback(solution);
                }
                SolverStatus::False(_) => {
                    tracing::debug!(
                        "LSGExtender step {}: LSG is still unreachable, trying to extend",
                        refinement_step
                    );

                    // we are still unreachable, so we can try to extend the LSG
                    if let Some(extended) = self.strategy.extend(&self.lsg, refinement_step) {
                        tracing::debug!(
                            "LSGExtender step {}: Strategy provided extended LSG with {} states",
                            refinement_step,
                            extended.size()
                        );

                        self.old_lsg = Some(std::mem::replace(&mut self.lsg, extended));
                    } else {
                        tracing::debug!(
                            "LSGExtender step {}: Strategy did not provide extended LSG, stopping",
                            refinement_step
                        );

                        // No more nodes to extend, we can stop
                        break;
                    }
                }
                SolverStatus::Unknown(_) => {
                    tracing::debug!(
                        "LSGExtender step {}: Solver returned unknown, stopping",
                        refinement_step
                    );

                    // Solver returned unknown, we just stop here
                    break;
                }
            }

            refinement_step += 1;
            if refinement_step >= self.max_refinements {
                tracing::debug!(
                    "LSGExtender step {}: Reached max refinement steps, stopping",
                    refinement_step
                );
                break;
            }
        }

        self.lsg.to_cfg()
    }
}

pub trait ExtensionStrategy {
    fn extend<'a>(&mut self, lsg: &LinearSubGraph<'a>, step: u64) -> Option<LinearSubGraph<'a>>;

    fn on_rollback(&mut self, solution: &LSGSolution);
}

#[derive(Debug)]
pub enum ExtensionStrategyEnum {
    RandomNode(RandomNodeStrategy),
    RandomSCC(RandomSCCStrategy),
}

impl ExtensionStrategyEnum {
    pub fn from_config(node_chooser: ExtensionStrategyConfig, seed: u64) -> Self {
        match node_chooser {
            ExtensionStrategyConfig::Random => {
                ExtensionStrategyEnum::RandomNode(RandomNodeStrategy::new(20, seed))
            }
            ExtensionStrategyConfig::RandomSCC => {
                ExtensionStrategyEnum::RandomSCC(RandomSCCStrategy::new(20, seed))
            }
        }
    }

    pub fn extend<'a>(
        &mut self,
        lsg: &LinearSubGraph<'a>,
        step: u64,
    ) -> Option<LinearSubGraph<'a>> {
        match self {
            ExtensionStrategyEnum::RandomNode(strategy) => strategy.extend(lsg, step),
            ExtensionStrategyEnum::RandomSCC(strategy) => strategy.extend(lsg, step),
        }
    }

    pub fn on_rollback(&mut self, solution: &LSGSolution) {
        match self {
            ExtensionStrategyEnum::RandomNode(strategy) => strategy.on_rollback(solution),
            ExtensionStrategyEnum::RandomSCC(strategy) => strategy.on_rollback(solution),
        }
    }
}

#[derive(Debug)]
pub struct RandomNodeStrategy {
    pub max_retries: usize,
    pub seed: u64,
    random: StdRng,
    pub blacklist: Vec<MultiGraphState>,
    pub last_added: Option<MultiGraphState>,
}

impl RandomNodeStrategy {
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

impl ExtensionStrategy for RandomNodeStrategy {
    fn extend<'a>(&mut self, lsg: &LinearSubGraph<'a>, _step: u64) -> Option<LinearSubGraph<'a>> {
        for _ in 0..self.max_retries {
            let parts_len = lsg.parts.len();
            let part_index = self.random.random_range(0..parts_len);
            let state = lsg.parts[part_index].random_node(lsg, &mut self.random);

            let neighbors: Vec<_> = lsg.product.undirected_neighbors(state);

            let selected = neighbors
                .iter()
                .find(|n| !lsg.contains_state(n) && !self.blacklist.contains(n));

            if let Some(selected) = selected {
                self.last_added = Some(selected.clone());
                return Some(lsg.add_node(selected.clone()));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &LSGSolution) {
        if let Some(last_node) = self.last_added.take() {
            self.blacklist.push(last_node);
        }
    }
}

#[derive(Debug)]
pub struct RandomSCCStrategy {
    pub max_retries: usize,
    pub seed: u64,
    random: StdRng,
    pub blacklist: Vec<MultiGraphState>,
    pub last_added: Vec<MultiGraphState>,
}

impl RandomSCCStrategy {
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

// Idea: chance based on how often a SCC was visited, when long in SCC, then
// maybe more safe?

// Idea: Sub SCC, we look at strongly connected subsets of SCCs

impl ExtensionStrategy for RandomSCCStrategy {
    fn extend<'a>(&mut self, lsg: &LinearSubGraph<'a>, _step: u64) -> Option<LinearSubGraph<'a>> {
        for _ in 0..self.max_retries {
            let paths = lsg
                .parts
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if let LSGPart::Path(path_idx) = p {
                        let path = lsg.path(*path_idx);
                        if path.path.len() > 1 {
                            return Some((i, *path_idx));
                        }
                    }
                    None
                })
                .collect::<Vec<_>>();

            if paths.is_empty() {
                return None;
            }

            for _ in 0..self.max_retries {
                let (part_index, path_idx) = paths[self.random.random_range(0..paths.len())];
                let path = lsg.path(path_idx);

                let state_index = self.random.random_range(0..path.path.state_len());
                let state = &path.path.states[state_index];

                if self.blacklist.contains(state) {
                    continue;
                }

                self.last_added.push(state.clone());
                return Some(lsg.add_scc_around_position(part_index, state_index));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &LSGSolution) {
        for last_node in self.last_added.drain(..) {
            self.blacklist.push(last_node);
        }
    }
}
