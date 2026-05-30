use std::fmt::Debug;

use hashbrown::HashSet;

use crate::{
    automaton::{
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        linear_graph::LinearGraph,
        path::Path,
        scc::{SCCAlgorithms, SCCDag},
        vass::counter::VASSCounterValuation,
    },
    config::{
        LinearGraphConfig, LinearGraphInterpolationStrategy, LinearGraphRegionOrder,
        LinearGraphSeedOrder,
    },
    solver::{SolverStatus, linear_graph_reach::LinearGraphReachSolverOptions},
};

mod layout;

use layout::{CandidateSeed, InterpolationLayout};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

/// Builds a large unreachable LinearGraph between one or more seed-language
/// lower bounds and the full SCCs of the current product approximation.
#[derive(Debug)]
pub struct LinearGraphExtender<'a> {
    primary_path: MultiGraphPath,
    auxiliary_paths: Vec<MultiGraphPath>,
    /// The subset of seed paths currently represented by `seed_linear_graph`.
    selected_path_indices: Vec<usize>,
    /// Reference to the underlying CFG.
    pub product: &'a ImplicitCFGProduct,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// Extender search and solver options.
    options: LinearGraphExtenderOptions,
    /// The best seed-language LinearGraph found for the selected seed subset.
    seed_linear_graph: LinearGraph<'a, MultiGraphState, ImplicitCFGProduct>,
    /// Interpolation layout for the selected seed subset.
    layout: Option<InterpolationLayout<'a>>,
    /// Optional SCC DAG supplied by a caller that already computed it.
    scc_dag: Option<SCCDag<MultiGraphState, CFGCounterUpdate>>,
}

#[derive(Debug, Clone)]
struct LinearGraphExtenderOptions {
    max_seed_checks: usize,
    max_interpolation_steps: usize,
    check_full_scc_upper_bound: bool,
    interpolation_strategy: LinearGraphInterpolationStrategy,
    region_order: LinearGraphRegionOrder,
    seed_order: LinearGraphSeedOrder,
    reach_solver_max_iterations: Option<u32>,
    reach_solver_timeout: Option<std::time::Duration>,
}

impl LinearGraphExtenderOptions {
    fn from_refinement_steps(max_refinements: u64) -> Self {
        let max_checks = refinement_steps_to_usize(max_refinements);

        Self {
            max_seed_checks: max_checks,
            max_interpolation_steps: max_checks,
            check_full_scc_upper_bound: true,
            interpolation_strategy: LinearGraphInterpolationStrategy::AdaptiveBatch,
            region_order: LinearGraphRegionOrder::GainDescending,
            seed_order: LinearGraphSeedOrder::MorePathsThenSize,
            reach_solver_max_iterations: None,
            reach_solver_timeout: None,
        }
    }

    fn from_config(config: &LinearGraphConfig) -> Self {
        let default_checks = refinement_steps_to_usize(*config.get_max_refinement_steps());

        Self {
            max_seed_checks: (*config.get_max_seed_checks()).unwrap_or(default_checks),
            max_interpolation_steps: (*config.get_max_interpolation_steps())
                .unwrap_or(default_checks),
            check_full_scc_upper_bound: *config.get_check_full_scc_upper_bound(),
            interpolation_strategy: *config.get_interpolation_strategy(),
            region_order: *config.get_region_order(),
            seed_order: *config.get_seed_order(),
            reach_solver_max_iterations: *config.get_reach_solver_max_iterations(),
            reach_solver_timeout: *config.get_reach_solver_timeout(),
        }
    }
}

impl<'a> LinearGraphExtender<'a> {
    /// Creates an extender from a single seed path.
    ///
    /// This keeps the older single-path API intact and delegates to
    /// `from_paths`, which owns the multi-seed setup.
    pub fn new(
        path: MultiGraphPath,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        Self::from_primary_path_with_options(
            path,
            Vec::new(),
            product,
            dimension,
            initial_valuation,
            final_valuation,
            LinearGraphExtenderOptions::from_refinement_steps(max_refinements),
        )
    }

    /// Creates an extender from one or more seed paths.
    ///
    /// The first path is the primary path that must be covered by any selected
    /// LinearGraph. Remaining paths are auxiliary paths that may enrich the
    /// primary path when they take the same SCC-DAG route.
    pub fn from_paths(
        paths: Vec<MultiGraphPath>,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        let (primary_path, auxiliary_paths) = split_primary_path(paths);

        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            product,
            dimension,
            initial_valuation,
            final_valuation,
            LinearGraphExtenderOptions::from_refinement_steps(max_refinements),
        )
    }

    /// Creates an extender from a primary path and auxiliary paths.
    ///
    /// Every selected LinearGraph must include the primary path. Auxiliary
    /// paths are only used when they take the same SCC-DAG route as the
    /// primary path, so they can add seed nodes without changing the full
    /// LinearGraph shape being cut.
    pub fn from_primary_path(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            product,
            dimension,
            initial_valuation,
            final_valuation,
            LinearGraphExtenderOptions::from_refinement_steps(max_refinements),
        )
    }

    fn from_primary_path_with_options(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        options: LinearGraphExtenderOptions,
    ) -> Self {
        let initial_linear_graph = LinearGraph::from_path(primary_path.clone(), product, dimension);

        LinearGraphExtender {
            primary_path,
            auxiliary_paths,
            selected_path_indices: vec![0],
            dimension,
            product,
            initial_valuation,
            final_valuation,
            options,
            seed_linear_graph: initial_linear_graph,
            layout: None,
            scc_dag: None,
        }
    }

    /// Reuses a precomputed SCC DAG for route-compatible LinearGraph layout
    /// building.
    pub fn with_scc_dag(mut self, scc_dag: SCCDag<MultiGraphState, CFGCounterUpdate>) -> Self {
        self.scc_dag = Some(scc_dag);
        self
    }

    /// Creates a single-path extender using dimension and boundary valuations
    /// from the implicit product.
    pub fn from_cfg_product(
        path: MultiGraphPath,
        cfg_product: &'a ImplicitCFGProduct,
        max_refinements: u64,
    ) -> Self {
        Self::new(
            path,
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            max_refinements,
        )
    }

    /// Creates a single-path extender using the LinearGraph configuration.
    pub fn from_cfg_product_with_config(
        path: MultiGraphPath,
        cfg_product: &'a ImplicitCFGProduct,
        config: &LinearGraphConfig,
    ) -> Self {
        Self::from_primary_path_with_options(
            path,
            Vec::new(),
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            LinearGraphExtenderOptions::from_config(config),
        )
    }

    /// Creates a multi-path extender using dimension and boundary valuations
    /// from the implicit product.
    pub fn from_cfg_product_paths(
        paths: Vec<MultiGraphPath>,
        cfg_product: &'a ImplicitCFGProduct,
        max_refinements: u64,
    ) -> Self {
        Self::from_paths(
            paths,
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            max_refinements,
        )
    }

    /// Creates a multi-path extender using the LinearGraph configuration.
    pub fn from_cfg_product_paths_with_config(
        paths: Vec<MultiGraphPath>,
        cfg_product: &'a ImplicitCFGProduct,
        config: &LinearGraphConfig,
    ) -> Self {
        let (primary_path, auxiliary_paths) = split_primary_path(paths);

        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            LinearGraphExtenderOptions::from_config(config),
        )
    }

    /// Creates a primary-path extender with auxiliary paths using dimension and
    /// boundary valuations from the implicit product.
    pub fn from_cfg_product_primary_path(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        cfg_product: &'a ImplicitCFGProduct,
        max_refinements: u64,
    ) -> Self {
        Self::from_primary_path(
            primary_path,
            auxiliary_paths,
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            max_refinements,
        )
    }

    /// Creates a primary-path extender with auxiliary paths using the
    /// LinearGraph configuration.
    pub fn from_cfg_product_primary_path_with_config(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        cfg_product: &'a ImplicitCFGProduct,
        config: &LinearGraphConfig,
    ) -> Self {
        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            cfg_product,
            cfg_product.dimension,
            cfg_product.initial_valuation.clone(),
            cfg_product.final_valuation.clone(),
            LinearGraphExtenderOptions::from_config(config),
        )
    }

    /// Refines `self.linear_graph` by searching between the seed-language
    /// LinearGraph and the full-SCC LinearGraph induced by the current
    /// product.
    pub fn run(&mut self) -> VASSCFG<()> {
        self.run_linear_graph().to_cfg()
    }

    /// Runs the extender and returns the selected unreachable LinearGraph.
    ///
    /// The public `run` method still returns a CFG because that is what the
    /// VASS reachability refinement consumes, but tests and future call sites
    /// can use this method to inspect the chosen LinearGraph directly.
    pub fn run_linear_graph(&mut self) -> LinearGraph<'a, MultiGraphState, ImplicitCFGProduct> {
        let _span = tracing::span!(
            tracing::Level::DEBUG,
            "LinearGraphExtender::run_linear_graph"
        )
        .entered();

        let mut seed_checks = 0usize;

        let Some(seed) = self.select_initial_seed(self.options.max_seed_checks, &mut seed_checks)
        else {
            return self.fallback_to_exact_primary_path();
        };

        let layout = self.install_initial_seed(seed, seed_checks);
        let best = self.seed_linear_graph.clone();
        let mut checks = 0usize;

        if self.interpolation_is_exhausted(&layout, checks, self.options.max_interpolation_steps) {
            return best;
        }

        tracing::debug!(
            regions = layout.regions.len(),
            seed_size = best.size(),
            "Starting interpolated LinearGraph search"
        );

        if self.options.check_full_scc_upper_bound
            && checks < self.options.max_interpolation_steps
            && let Some(full) = self.try_full_scc_upper_bound(&layout, &mut checks)
        {
            return full;
        }

        self.search_interpolated_regions(
            &layout,
            best,
            checks,
            self.options.max_interpolation_steps,
        )
    }

    /// Keeps the exact primary path when no larger seed-language candidate can
    /// be proved unreachable.
    fn fallback_to_exact_primary_path(
        &mut self,
    ) -> LinearGraph<'a, MultiGraphState, ImplicitCFGProduct> {
        tracing::debug!(
            "No seed LinearGraph was proved unreachable; keeping exact first path LinearGraph"
        );

        let exact = LinearGraph::from_path(self.primary_path.clone(), self.product, self.dimension);
        let result = self.solve_candidate(&exact);

        debug_assert!(
            matches!(&result.status, SolverStatus::False(_)),
            "Exact primary path LinearGraph must be unreachable when used as fallback"
        );
        if !matches!(&result.status, SolverStatus::False(_)) {
            tracing::warn!(
                status = ?result.status,
                "Exact primary path LinearGraph was not proved unreachable during fallback"
            );
        }

        exact
    }

    /// Records the selected seed as the lower bound for interpolation.
    fn install_initial_seed(
        &mut self,
        seed: CandidateSeed<'a>,
        seed_checks: usize,
    ) -> InterpolationLayout<'a> {
        let layout = seed.layout;

        self.selected_path_indices = seed.path_indices;
        self.seed_linear_graph = seed.seed_linear_graph;
        self.layout = Some(layout.clone());

        tracing::debug!(
            size = self.seed_linear_graph.size(),
            selected_paths = self.selected_path_indices.len(),
            seed_checks,
            "Seed-language LinearGraph is unreachable; using it as search lower bound"
        );

        layout
    }

    /// Returns true when there are no SCC regions left to expand, or the
    /// per-phase solver budget has already been spent.
    fn interpolation_is_exhausted(
        &self,
        layout: &InterpolationLayout<'a>,
        checks: usize,
        max_checks: usize,
    ) -> bool {
        let exhausted = layout.regions.is_empty() || checks >= max_checks;

        if exhausted {
            tracing::debug!(
                regions = layout.regions.len(),
                checks,
                max_checks,
                "No interpolation search needed"
            );
        }

        exhausted
    }

    /// Checks the full-SCC upper bound before doing smaller interpolation
    /// steps. If it is unreachable, no larger candidate exists in this layout.
    fn try_full_scc_upper_bound(
        &self,
        layout: &InterpolationLayout<'a>,
        checks: &mut usize,
    ) -> Option<LinearGraph<'a, MultiGraphState, ImplicitCFGProduct>> {
        let full_mask = vec![true; layout.regions.len()];
        let full = layout.build_candidate(&full_mask);
        let full_result = self.solve_candidate(&full.linear_graph);
        *checks += 1;

        if matches!(full_result.status, SolverStatus::False(_)) {
            tracing::debug!(
                size = full.linear_graph.size(),
                checks = *checks,
                "Full-SCC LinearGraph is unreachable"
            );
            return Some(full.linear_graph);
        }

        None
    }

    /// Grows the seed candidate by enabling batches of SCC regions and keeping
    /// only the unreachable expansions.
    fn search_interpolated_regions(
        &self,
        layout: &InterpolationLayout<'a>,
        mut best: LinearGraph<'a, MultiGraphState, ImplicitCFGProduct>,
        mut checks: usize,
        max_checks: usize,
    ) -> LinearGraph<'a, MultiGraphState, ImplicitCFGProduct> {
        let mut accepted = vec![false; layout.regions.len()];
        let mut pending = self.ordered_regions(layout);
        let mut strategy =
            interpolation_strategy(self.options.interpolation_strategy, pending.len());

        while checks < max_checks && !pending.is_empty() {
            let batch = strategy.next_batch(&pending);
            if batch.is_empty() {
                break;
            }
            let batch_len = batch.len();

            // Try one extra region batch on top of the known-unreachable mask.
            let candidate_mask = mask_with_batch(&accepted, &batch);
            let candidate = layout.build_candidate(&candidate_mask);
            let candidate_result = self.solve_candidate(&candidate.linear_graph);
            checks += 1;

            match candidate_result.status {
                SolverStatus::False(_) => {
                    tracing::debug!(
                        size = candidate.linear_graph.size(),
                        enabled_regions = candidate_mask.iter().filter(|enabled| **enabled).count(),
                        checks,
                        "Interpolated candidate is unreachable"
                    );

                    accepted = candidate_mask;
                    best = candidate.linear_graph;
                    strategy.on_unreachable(&mut pending, &batch);
                }
                SolverStatus::True(solution) => {
                    let used = candidate.used_full_regions(&solution);
                    let used_in_batch = batch
                        .iter()
                        .copied()
                        .filter(|region| used.contains(region))
                        .collect::<HashSet<_>>();

                    tracing::debug!(
                        used_regions = used_in_batch.len(),
                        batch_len,
                        checks,
                        "Interpolated candidate is reachable"
                    );

                    strategy.on_reachable(&mut pending, &batch, &used_in_batch);
                }
                SolverStatus::Unknown(reason) => {
                    tracing::debug!(
                        ?reason,
                        batch_len,
                        checks,
                        "Interpolated candidate returned unknown"
                    );

                    strategy.on_unknown(&mut pending, &batch);
                }
            }
        }

        tracing::debug!(
            size = best.size(),
            enabled_regions = accepted.iter().filter(|enabled| **enabled).count(),
            checks,
            max_checks,
            "Finished interpolated LinearGraph search"
        );

        best
    }

    /// Finds a large path-compatible seed-language LinearGraph that is still
    /// unreachable within this phase's solver-check budget.
    ///
    /// This is deliberately best-effort. Compatible auxiliary paths are tried
    /// in size-ranked prefix groups instead of exhaustively enumerating
    /// subsets, since the full search space is exponential.
    fn select_initial_seed(
        &self,
        max_checks: usize,
        checks: &mut usize,
    ) -> Option<CandidateSeed<'a>> {
        let computed_dag;
        let dag = if let Some(dag) = &self.scc_dag {
            dag
        } else {
            computed_dag = self.product.find_scc_dag();
            &computed_dag
        };

        let mut candidates = InterpolationLayout::from_compatible_path_groups(
            &self.primary_path,
            &self.auxiliary_paths,
            self.product,
            self.dimension,
            dag,
        );

        candidates.sort_by(|left, right| {
            let left_path_count = left.path_indices.len();
            let right_path_count = right.path_indices.len();
            let left_size = left.seed_linear_graph.size();
            let right_size = right.seed_linear_graph.size();

            match self.options.seed_order {
                LinearGraphSeedOrder::MorePathsThenSize => right_path_count
                    .cmp(&left_path_count)
                    .then_with(|| right_size.cmp(&left_size)),
                LinearGraphSeedOrder::LargerSeedFirst => right_size
                    .cmp(&left_size)
                    .then_with(|| right_path_count.cmp(&left_path_count)),
                LinearGraphSeedOrder::SmallerSeedFirst => left_size
                    .cmp(&right_size)
                    .then_with(|| right_path_count.cmp(&left_path_count)),
            }
        });

        for candidate in candidates {
            if *checks >= max_checks {
                break;
            }

            let result = self.solve_candidate(&candidate.seed_linear_graph);
            *checks += 1;

            if matches!(result.status, SolverStatus::False(_)) {
                tracing::debug!(
                    selected_paths = candidate.path_indices.len(),
                    size = candidate.seed_linear_graph.size(),
                    checks,
                    "Selected path subset for initial LinearGraph"
                );
                return Some(candidate);
            }
        }

        None
    }

    /// Checks whether a candidate LinearGraph is reachable between the
    /// configured boundary valuations.
    fn solve_candidate(
        &self,
        linear_graph: &LinearGraph<'a, MultiGraphState, ImplicitCFGProduct>,
    ) -> crate::solver::linear_graph_reach::LinearGraphReachSolverResult {
        LinearGraphReachSolverOptions::default()
            .with_optional_iteration_limit(self.options.reach_solver_max_iterations)
            .with_optional_time_limit(self.options.reach_solver_timeout)
            .to_solver(linear_graph, &self.initial_valuation, &self.final_valuation)
            .solve()
    }

    fn ordered_regions(&self, layout: &InterpolationLayout<'a>) -> Vec<usize> {
        let mut pending = (0..layout.regions.len()).collect::<Vec<_>>();

        match self.options.region_order {
            LinearGraphRegionOrder::GainDescending => {
                pending.sort_by_key(|region| std::cmp::Reverse(layout.regions[*region].gain()));
            }
            LinearGraphRegionOrder::GainAscending => {
                pending.sort_by_key(|region| layout.regions[*region].gain());
            }
            LinearGraphRegionOrder::Input => {}
        }

        pending
    }
}

fn mask_with_batch(accepted: &[bool], batch: &[usize]) -> Vec<bool> {
    let mut mask = accepted.to_vec();

    for region in batch {
        mask[*region] = true;
    }

    mask
}

fn split_primary_path(mut paths: Vec<MultiGraphPath>) -> (MultiGraphPath, Vec<MultiGraphPath>) {
    assert!(
        !paths.is_empty(),
        "LinearGraphExtender requires at least one seed path"
    );

    let primary_path = paths.remove(0);
    (primary_path, paths)
}

fn refinement_steps_to_usize(max_refinements: u64) -> usize {
    usize::try_from(max_refinements.max(1)).unwrap_or(usize::MAX)
}

trait InterpolationStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize>;
    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]);
    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    );
    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]);
}

fn interpolation_strategy(
    strategy: LinearGraphInterpolationStrategy,
    pending_len: usize,
) -> Box<dyn InterpolationStrategy> {
    match strategy {
        LinearGraphInterpolationStrategy::AdaptiveBatch => {
            Box::new(AdaptiveBatchStrategy::new(pending_len))
        }
        LinearGraphInterpolationStrategy::Linear => Box::new(LinearInterpolationStrategy),
    }
}

struct AdaptiveBatchStrategy {
    batch_size: usize,
}

impl AdaptiveBatchStrategy {
    fn new(pending_len: usize) -> Self {
        Self {
            batch_size: next_halving_batch_size(pending_len),
        }
    }
}

impl InterpolationStrategy for AdaptiveBatchStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize> {
        let batch_len = self.batch_size.min(pending.len()).max(1);
        pending[..batch_len].to_vec()
    }

    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
        self.batch_size = next_halving_batch_size(pending.len());
    }

    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    ) {
        if used_in_batch.is_empty() {
            if batch.len() == 1 {
                remove_batch(pending, batch);
            } else {
                self.batch_size = next_halving_batch_size(batch.len());
            }
        } else {
            pending.retain(|region| !used_in_batch.contains(region));
            self.batch_size = next_halving_batch_size(pending.len());
        }
    }

    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        if batch.len() == 1 {
            remove_batch(pending, batch);
        } else {
            self.batch_size = next_halving_batch_size(batch.len());
        }
    }
}

struct LinearInterpolationStrategy;

impl InterpolationStrategy for LinearInterpolationStrategy {
    fn next_batch(&mut self, pending: &[usize]) -> Vec<usize> {
        pending.first().copied().into_iter().collect()
    }

    fn on_unreachable(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
    }

    fn on_reachable(
        &mut self,
        pending: &mut Vec<usize>,
        batch: &[usize],
        used_in_batch: &HashSet<usize>,
    ) {
        if used_in_batch.is_empty() {
            remove_batch(pending, batch);
        } else {
            pending.retain(|region| !used_in_batch.contains(region));
        }
    }

    fn on_unknown(&mut self, pending: &mut Vec<usize>, batch: &[usize]) {
        remove_batch(pending, batch);
    }
}

fn remove_batch(pending: &mut Vec<usize>, batch: &[usize]) {
    pending.retain(|region| !batch.contains(region));
}

fn next_halving_batch_size(len: usize) -> usize {
    len.div_ceil(2).max(1)
}
