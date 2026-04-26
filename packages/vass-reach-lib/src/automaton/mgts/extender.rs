use std::fmt::Debug;

use hashbrown::HashSet;
use rand::{RngExt, SeedableRng, rngs::StdRng};

use crate::{
    automaton::{
        TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        mgts::{
            MGTS,
            part::{MGTSPart, MarkedGraph},
        },
        path::Path,
        vass::counter::VASSCounterValuation,
    },
    config::ExtensionStrategyConfig,
    solver::{
        SolverStatus,
        mgts_reach::{MGTSReachSolverOptions, MGTSSolution},
    },
};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

/// Struct to iteratively extend an MGTS by adding nodes chosen
/// by a `NodeChooser`, while keeping the MGTS unreachable.
#[derive(Debug)]
pub struct MGTSExtender<'a> {
    /// The current MGTS being extended.
    pub mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    /// The previous MGTS before the last extension.
    /// This is used to backtrack if the current MGTS becomes reachable.
    pub old_mgts: Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>>,
    /// Reference to the underlying CFG.
    pub product: &'a ImplicitCFGProduct,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// The strategy used to extend the MGTS.
    pub strategy: ExtensionStrategyEnum,
    /// Maximum number of refinement steps to perform.
    pub max_refinements: u64,
}

impl<'a> MGTSExtender<'a> {
    pub fn new(
        path: MultiGraphPath,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        strategy: ExtensionStrategyEnum,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        let mgts = MGTS::from_path_roll_up(path, product, dimension);

        MGTSExtender {
            old_mgts: None,
            mgts,
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

    /// Refines `self.mgts` by trying to extend it using the `node_chooser`
    /// until no more nodes can be added or the maximum number of refinements is
    /// reached.
    pub fn run(&mut self) -> VASSCFG<()> {
        tracing::span!(tracing::Level::DEBUG, "MGTSExtender::run");

        let mut refinement_step = 0;

        loop {
            let solver_result = MGTSReachSolverOptions::default()
                .to_solver(&self.mgts, &self.initial_valuation, &self.final_valuation)
                .solve();

            match &solver_result.status {
                SolverStatus::True(solution) => {
                    tracing::debug!(
                        "MGTSExtender step {}: MGTS became reachable, rolling back",
                        refinement_step
                    );

                    // we became reachable, so we need to remove the last extension
                    let old = self.old_mgts.take();
                    self.mgts = old.expect("MGTSExtender: Something went wrong, we became reachable but have no old MGTS to backtrack to. Maybe the initial MGTS was already reachable?");

                    self.strategy.on_rollback(solution);
                }
                SolverStatus::False(_) => {
                    tracing::debug!(
                        "MGTSExtender step {}: MGTS is still unreachable, trying to extend",
                        refinement_step
                    );

                    // we are still unreachable, so we can try to extend the MGTS
                    if let Some(extended) = self.strategy.extend(&self.mgts, refinement_step) {
                        tracing::debug!(
                            "MGTSExtender step {}: Strategy provided extended MGTS with {} states",
                            refinement_step,
                            extended.size()
                        );

                        self.old_mgts = Some(std::mem::replace(&mut self.mgts, extended));
                    } else {
                        tracing::debug!(
                            "MGTSExtender step {}: Strategy did not provide extended MGTS, stopping",
                            refinement_step
                        );

                        // No more nodes to extend, we can stop
                        break;
                    }
                }
                SolverStatus::Unknown(_) => {
                    tracing::debug!(
                        "MGTSExtender step {}: Solver returned unknown, stopping",
                        refinement_step
                    );

                    // Solver returned unknown, we just stop here
                    break;
                }
            }

            refinement_step += 1;
            if refinement_step >= self.max_refinements {
                tracing::debug!(
                    "MGTSExtender step {}: Reached max refinement steps, stopping",
                    refinement_step
                );
                break;
            }
        }

        self.mgts.to_cfg()
    }
}

pub trait ExtensionStrategy {
    fn extend<'a>(
        &mut self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        step: u64,
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>>;

    fn on_rollback(&mut self, solution: &MGTSSolution);
}

#[derive(Debug)]
pub enum ExtensionStrategyEnum {
    RandomNode(RandomNodeStrategy),
    RandomSCC(RandomSCCStrategy),
    CompletePartialSCC(CompletePartialSCCStrategy),
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
            ExtensionStrategyConfig::CompletePartialSCC => {
                ExtensionStrategyEnum::CompletePartialSCC(CompletePartialSCCStrategy::new())
            }
        }
    }

    pub fn extend<'a>(
        &mut self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        step: u64,
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>> {
        match self {
            ExtensionStrategyEnum::RandomNode(strategy) => strategy.extend(mgts, step),
            ExtensionStrategyEnum::RandomSCC(strategy) => strategy.extend(mgts, step),
            ExtensionStrategyEnum::CompletePartialSCC(strategy) => strategy.extend(mgts, step),
        }
    }

    pub fn on_rollback(&mut self, solution: &MGTSSolution) {
        match self {
            ExtensionStrategyEnum::RandomNode(strategy) => strategy.on_rollback(solution),
            ExtensionStrategyEnum::RandomSCC(strategy) => strategy.on_rollback(solution),
            ExtensionStrategyEnum::CompletePartialSCC(strategy) => strategy.on_rollback(solution),
        }
    }
}

#[derive(Debug, Default)]
pub struct CompletePartialSCCStrategy;

impl CompletePartialSCCStrategy {
    pub fn new() -> Self {
        Self
    }
}

impl ExtensionStrategy for CompletePartialSCCStrategy {
    fn extend<'a>(
        &mut self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        _step: u64,
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>> {
        for part in &mgts.sequence {
            let MGTSPart::Graph(graph_idx) = part else {
                continue;
            };

            let graph = mgts.graph(*graph_idx);
            let start = graph.product_start().clone();
            let end = graph.product_end().clone();

            let full_scc_set = mgts.automaton.find_scc_surrounding(start.clone());
            tracing::debug!(
                "CompletePartialSCCStrategy: Found SCC of size {} around node {:?}",
                full_scc_set.len(),
                start
            );
            let mut full_scc = full_scc_set.iter().cloned().collect::<Vec<_>>();
            full_scc.sort_unstable();
            let current_set = graph
                .graph
                .node_weights()
                .cloned()
                .collect::<HashSet<MultiGraphState>>();

            // Only treat this as a partial SCC when the graph is a true SCC
            // subset. Other graph shapes are left untouched by this strategy.
            if !current_set.is_subset(&full_scc_set) {
                continue;
            }

            if current_set == full_scc_set {
                continue;
            }

            let mut extended = mgts.clone();
            extended.graphs[*graph_idx] =
                MarkedGraph::from_subset(mgts.automaton, &full_scc, start, end);
            extended.assert_consistent();

            return Some(extended);
        }

        None
    }

    fn on_rollback(&mut self, _solution: &MGTSSolution) {}
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
    fn extend<'a>(
        &mut self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        _step: u64,
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>> {
        for _ in 0..self.max_retries {
            let parts_len = mgts.sequence.len();
            let part_index = self.random.random_range(0..parts_len);
            let state = mgts.sequence[part_index].random_node(mgts, &mut self.random);

            let neighbors: Vec<_> = mgts.automaton.undirected_neighbors(state);

            let selected = neighbors
                .iter()
                .find(|n| !mgts.contains_state(n) && !self.blacklist.contains(n));

            if let Some(selected) = selected {
                self.last_added = Some(selected.clone());
                return Some(mgts.add_node(selected.clone()));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &MGTSSolution) {
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

impl ExtensionStrategy for RandomSCCStrategy {
    fn extend<'a>(
        &mut self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        _step: u64,
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>> {
        for _ in 0..self.max_retries {
            let paths = mgts
                .sequence
                .iter()
                .enumerate()
                .filter_map(|(i, p)| {
                    if let MGTSPart::Path(path_idx) = p {
                        let path = mgts.path(*path_idx);
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
                let path = mgts.path(path_idx);

                let state_index = self.random.random_range(0..path.path.state_len());
                let state = &path.path.states[state_index];

                if self.blacklist.contains(state) {
                    continue;
                }

                self.last_added.push(state.clone());
                return Some(mgts.add_scc_around_position(part_index, state_index));
            }
        }

        None
    }

    fn on_rollback(&mut self, _solution: &MGTSSolution) {
        for last_node in self.last_added.drain(..) {
            self.blacklist.push(last_node);
        }
    }
}
