use std::{cell::RefCell, collections::VecDeque, fmt::Debug};

use hashbrown::{HashMap, HashSet};
use z3::{Optimize, SatResult, ast::Int};

use crate::{
    automaton::{
        Alphabet, Automaton, InitializedAutomaton, TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{state::MultiGraphState, view::ImplicitCFGProductView},
        linear_graph::LinearGraph,
        path::Path,
        scc::{SCCAlgorithms, SCCDag},
        vass::counter::VASSCounterValuation,
    },
    config::{
        LinearGraphConfig, LinearGraphInterpolationStrategy, LinearGraphRegionOrder,
        LinearGraphSeedOrder,
    },
    solver::{
        SolverStatus,
        linear_graph_reach::{LinearGraphReachSolverOptions, LinearTemplateLowerBound},
    },
};

mod layout;

use layout::{CandidateSeed, InterpolationLayout};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;
type ProductViewLinearGraph<'a> = LinearGraph<'a, MultiGraphState, ImplicitCFGProductView<'a>>;

/// Builds a large unreachable LinearGraph between one or more seed-language
/// lower bounds and the full SCCs of the current product approximation.
#[derive(Debug)]
pub struct LinearGraphExtender<'a> {
    primary_path: MultiGraphPath,
    auxiliary_paths: Vec<MultiGraphPath>,
    /// Reference to the underlying CFG.
    pub product: &'a ImplicitCFGProductView<'a>,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// Extender search and solver options.
    options: LinearGraphExtenderOptions,
    /// Optional SCC DAG supplied by a caller that already computed it.
    scc_dag: Option<SCCDag<MultiGraphState, CFGCounterUpdate>>,
    template_lower_bounds: RefCell<MainCFGTemplateLowerBounds>,
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
        product: &'a ImplicitCFGProductView<'a>,
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
        product: &'a ImplicitCFGProductView<'a>,
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
        product: &'a ImplicitCFGProductView<'a>,
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
        product: &'a ImplicitCFGProductView<'a>,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        options: LinearGraphExtenderOptions,
    ) -> Self {
        let template_lower_bounds =
            main_cfg_template_lower_bounds(product.product.main_cfg(), &initial_valuation);

        LinearGraphExtender {
            primary_path,
            auxiliary_paths,
            dimension,
            product,
            initial_valuation,
            final_valuation,
            options,
            scc_dag: None,
            template_lower_bounds: RefCell::new(template_lower_bounds),
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
    pub fn from_product_view(
        path: MultiGraphPath,
        product_view: &'a ImplicitCFGProductView<'a>,
        max_refinements: u64,
    ) -> Self {
        let (initial_valuation, final_valuation) = product_view_boundary_valuations(product_view);

        Self::from_primary_path_with_options(
            path,
            Vec::new(),
            product_view,
            product_view.dimension(),
            initial_valuation,
            final_valuation,
            LinearGraphExtenderOptions::from_refinement_steps(max_refinements),
        )
    }

    /// Creates a single-path extender using the LinearGraph configuration.
    pub fn from_product_view_with_config(
        path: MultiGraphPath,
        product_view: &'a ImplicitCFGProductView<'a>,
        config: &LinearGraphConfig,
    ) -> Self {
        Self::from_product_view_primary_path_with_config(path, Vec::new(), product_view, config)
    }

    /// Creates a multi-path extender using dimension and boundary valuations
    /// from the implicit product.
    pub fn from_product_view_paths(
        paths: Vec<MultiGraphPath>,
        product_view: &'a ImplicitCFGProductView<'a>,
        max_refinements: u64,
    ) -> Self {
        let (primary_path, auxiliary_paths) = split_primary_path(paths);
        Self::from_product_view_primary_path(
            primary_path,
            auxiliary_paths,
            product_view,
            max_refinements,
        )
    }

    /// Creates a multi-path extender using the LinearGraph configuration.
    pub fn from_product_view_paths_with_config(
        paths: Vec<MultiGraphPath>,
        product_view: &'a ImplicitCFGProductView<'a>,
        config: &LinearGraphConfig,
    ) -> Self {
        let (primary_path, auxiliary_paths) = split_primary_path(paths);
        Self::from_product_view_primary_path_with_config(
            primary_path,
            auxiliary_paths,
            product_view,
            config,
        )
    }

    /// Creates a primary-path extender with auxiliary paths using dimension and
    /// boundary valuations from the implicit product.
    pub fn from_product_view_primary_path(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        product_view: &'a ImplicitCFGProductView<'a>,
        max_refinements: u64,
    ) -> Self {
        let (initial_valuation, final_valuation) = product_view_boundary_valuations(product_view);

        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            product_view,
            product_view.dimension(),
            initial_valuation,
            final_valuation,
            LinearGraphExtenderOptions::from_refinement_steps(max_refinements),
        )
    }

    /// Creates a primary-path extender with auxiliary paths using the
    /// LinearGraph configuration.
    pub fn from_product_view_primary_path_with_config(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        product_view: &'a ImplicitCFGProductView<'a>,
        config: &LinearGraphConfig,
    ) -> Self {
        let (initial_valuation, final_valuation) = product_view_boundary_valuations(product_view);

        Self::from_primary_path_with_options(
            primary_path,
            auxiliary_paths,
            product_view,
            product_view.dimension(),
            initial_valuation,
            final_valuation,
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
    pub fn run_linear_graph(&mut self) -> ProductViewLinearGraph<'a> {
        let _span = tracing::span!(
            tracing::Level::DEBUG,
            "LinearGraphExtender::run_linear_graph"
        )
        .entered();

        let mut seed_checks = 0usize;

        let Some(seed) = self.select_initial_seed(self.options.max_seed_checks, &mut seed_checks)
        else {
            if let Some(repeated_seed) =
                self.select_repeated_path_seed(self.options.max_seed_checks, &mut seed_checks)
            {
                return repeated_seed;
            }
            return self.fallback_to_exact_primary_path();
        };

        let (layout, best) = self.install_initial_seed(seed, seed_checks);
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
    fn fallback_to_exact_primary_path(&mut self) -> ProductViewLinearGraph<'a> {
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

    fn select_repeated_path_seed(
        &self,
        max_checks: usize,
        checks: &mut usize,
    ) -> Option<ProductViewLinearGraph<'a>> {
        let computed_dag;
        let dag = if let Some(dag) = &self.scc_dag {
            dag
        } else {
            computed_dag = self.product.find_scc_dag();
            &computed_dag
        };

        let first_negative_position = self
            .primary_path
            .find_negative_counter_forward(&self.initial_valuation)
            .map(|(_, transition)| transition + 1)
            .unwrap_or(0);
        let mut positions = self
            .primary_path
            .states
            .iter()
            .enumerate()
            .skip(first_negative_position)
            .map(|(position, state)| (position, state.clone()))
            .collect::<Vec<_>>();
        positions.sort_by_key(|(position, _)| {
            let is_control_self_loop = *position < self.primary_path.len()
                && self.primary_path.states[*position].cfg_state(0)
                    == self.primary_path.states[*position + 1].cfg_state(0);
            (!is_control_self_loop, *position)
        });

        let mut repeated_paths = Vec::new();
        for (position, state) in &positions {
            let Some(component) = dag
                .components
                .iter()
                .find(|component| component.nodes.contains(state))
            else {
                continue;
            };
            let allowed = component.nodes.iter().cloned().collect::<HashSet<_>>();
            let preferred = self.primary_path.transitions.get(*position);
            if let Some(cycle) = preferred_rooted_cycle(self.product, state, &allowed, preferred) {
                repeated_paths.push((*position, cycle));
            }
        }

        if !repeated_paths.is_empty() && *checks < max_checks {
            let candidate = LinearGraph::from_path_with_repeats_at(
                self.primary_path.clone(),
                repeated_paths.clone(),
                self.product,
                self.dimension,
            );
            let result = self.solve_candidate(&candidate);
            *checks += 1;

            if matches!(result.status, SolverStatus::False(_)) {
                tracing::debug!(
                    repeat_paths = candidate.repeat_paths.len(),
                    first_negative_position,
                    checks = *checks,
                    "Repeated-path suffix LinearGraph is unreachable"
                );
                return Some(candidate);
            }
        }

        for (position, cycle) in &repeated_paths {
            if *checks >= max_checks {
                break;
            }

            let candidate = LinearGraph::from_path_with_repeat_at(
                self.primary_path.clone(),
                cycle.clone(),
                *position,
                self.product,
                self.dimension,
            );
            let result = self.solve_candidate(&candidate);
            *checks += 1;

            if matches!(result.status, SolverStatus::False(_)) {
                tracing::debug!(
                    repeat_position = *position,
                    repeat_length = cycle.len(),
                    first_negative_position,
                    checks = *checks,
                    "Witness-aligned repeated-path LinearGraph is unreachable"
                );
                return Some(candidate);
            }
        }

        None
    }

    /// Records the selected seed as the lower bound for interpolation.
    fn install_initial_seed(
        &self,
        seed: CandidateSeed<'a>,
        seed_checks: usize,
    ) -> (InterpolationLayout<'a>, ProductViewLinearGraph<'a>) {
        let selected_paths = seed.path_indices.len();
        let size = seed.seed_linear_graph.size();

        tracing::debug!(
            size,
            selected_paths,
            seed_checks,
            "Seed-language LinearGraph is unreachable; using it as search lower bound"
        );

        (seed.layout, seed.seed_linear_graph)
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
    ) -> Option<ProductViewLinearGraph<'a>> {
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
        mut best: ProductViewLinearGraph<'a>,
        mut checks: usize,
        max_checks: usize,
    ) -> ProductViewLinearGraph<'a> {
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
        linear_graph: &ProductViewLinearGraph<'a>,
    ) -> crate::solver::linear_graph_reach::LinearGraphReachSolverResult {
        const MAX_SYNTHESIS_STEPS: usize = 8;

        for synthesis_step in 0..=MAX_SYNTHESIS_STEPS {
            let result = self.solve_candidate_once(linear_graph);
            let Some(solution) = result.get_solution() else {
                return result;
            };

            if solution.build_run(linear_graph, true).is_some()
                || synthesis_step == MAX_SYNTHESIS_STEPS
            {
                return result;
            }

            let model_boundaries = solution.boundary_valuations(linear_graph);
            let Some((template, analysis)) =
                self.synthesize_template_excluding_boundaries(&model_boundaries)
            else {
                return result;
            };

            tracing::debug!(
                coefficients = ?template.coefficients,
                synthesis_step,
                "Synthesized relational lower-bound template from spurious LinearGraph model"
            );
            *self.template_lower_bounds.borrow_mut() = analysis;
        }

        unreachable!("template synthesis loop always returns")
    }

    fn solve_candidate_once(
        &self,
        linear_graph: &ProductViewLinearGraph<'a>,
    ) -> crate::solver::linear_graph_reach::LinearGraphReachSolverResult {
        let template_lower_bounds = self.template_lower_bounds.borrow();
        let boundary_lower_bounds = linear_graph_boundary_template_lower_bounds(
            linear_graph,
            &template_lower_bounds,
            self.product.product.main_cfg_index(),
        );

        LinearGraphReachSolverOptions::default()
            .with_optional_iteration_limit(self.options.reach_solver_max_iterations)
            .with_optional_time_limit(self.options.reach_solver_timeout)
            .into_solver_with_boundary_lower_bounds(
                linear_graph,
                &self.initial_valuation,
                &self.final_valuation,
                boundary_lower_bounds,
            )
            .solve()
    }

    fn synthesize_template_excluding_boundaries(
        &self,
        model_boundaries: &[(MultiGraphState, VASSCounterValuation)],
    ) -> Option<(LinearTemplate, MainCFGTemplateLowerBounds)> {
        const MAX_COEFFICIENT: i32 = 2;
        const MAX_CANDIDATES: usize = 256;

        let main_boundaries = model_boundaries
            .iter()
            .map(|(state, valuation)| {
                (
                    state.cfg_state(self.product.product.main_cfg_index()),
                    valuation.clone(),
                )
            })
            .collect::<Vec<_>>();
        synthesize_template_for_boundaries(
            self.product.product.main_cfg(),
            &self.initial_valuation,
            &self.template_lower_bounds.borrow(),
            &main_boundaries,
            MAX_COEFFICIENT,
            MAX_CANDIDATES,
        )
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

fn product_view_boundary_valuations(
    product_view: &ImplicitCFGProductView<'_>,
) -> (VASSCounterValuation, VASSCounterValuation) {
    (
        product_view.product.initial_valuation.clone(),
        product_view.product.final_valuation.clone(),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LinearTemplate {
    coefficients: Box<[i32]>,
}

#[derive(Debug, Clone)]
struct MainCFGTemplateLowerBounds {
    templates: Vec<LinearTemplate>,
    state_bounds: Vec<Option<Box<[i32]>>>,
}

fn linear_templates(dimension: usize) -> Vec<LinearTemplate> {
    let mut supports = (0..dimension)
        .map(|counter| vec![counter])
        .collect::<Vec<_>>();

    for left in 0..dimension {
        for right in left + 1..dimension {
            supports.push(vec![left, right]);
        }
    }

    if dimension > 2 {
        supports.push((0..dimension).collect());
    }

    supports
        .into_iter()
        .map(|support| {
            let mut coefficients = vec![0; dimension];
            for counter in &support {
                coefficients[*counter] = 1;
            }
            LinearTemplate {
                coefficients: coefficients.into_boxed_slice(),
            }
        })
        .collect()
}

fn template_value(template: &LinearTemplate, valuation: &VASSCounterValuation) -> i32 {
    template
        .coefficients
        .iter()
        .zip(valuation.iter())
        .map(|(coefficient, value)| coefficient * value)
        .sum()
}

fn main_cfg_template_lower_bounds(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
) -> MainCFGTemplateLowerBounds {
    analyze_templates(
        cfg,
        initial_valuation,
        linear_templates(initial_valuation.dimension()),
    )
}

fn analyze_templates(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: Vec<LinearTemplate>,
) -> MainCFGTemplateLowerBounds {
    // Clamping makes the abstract domain finite and only weakens each lower bound.
    let cap = i32::try_from(cfg.node_count()).unwrap_or(i32::MAX);
    let initial_bound = templates
        .iter()
        .map(|template| template_value(template, initial_valuation).clamp(0, cap))
        .collect::<Vec<_>>()
        .into_boxed_slice();
    let mut bounds = vec![None; cfg.node_count()];
    let initial = cfg.get_initial();
    bounds[initial.index()] = Some(initial_bound);

    let mut queue = VecDeque::from([initial]);
    let mut queued = vec![false; cfg.node_count()];
    queued[initial.index()] = true;

    while let Some(source) = queue.pop_front() {
        queued[source.index()] = false;
        let source_bound = bounds[source.index()]
            .as_ref()
            .expect("queued states have a lower bound")
            .clone();

        for update in cfg.alphabet() {
            let Some(target) = cfg.successor(&source, update) else {
                continue;
            };

            let mut candidate = source_bound.clone();

            for template_index in 0..templates.len() {
                candidate[template_index] = exact_successor_template_bound(
                    &templates,
                    &source_bound,
                    update,
                    template_index,
                    cap,
                );
            }
            let changed = if let Some(current) = &mut bounds[target.index()] {
                let previous = current.clone();
                for (current_bound, candidate_bound) in current.iter_mut().zip(candidate.iter()) {
                    *current_bound = (*current_bound).min(*candidate_bound);
                }
                *current != previous
            } else {
                bounds[target.index()] = Some(candidate);
                true
            };

            if changed && !queued[target.index()] {
                queued[target.index()] = true;
                queue.push_back(target);
            }
        }
    }

    MainCFGTemplateLowerBounds {
        templates,
        state_bounds: bounds,
    }
}

fn exact_successor_template_bound(
    templates: &[LinearTemplate],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
) -> i32 {
    let optimizer = Optimize::new();
    let counters = (0..templates[objective_index].coefficients.len())
        .map(|counter| Int::new_const(format!("template_transfer_c{counter}")))
        .collect::<Vec<_>>();

    for counter in &counters {
        optimizer.assert(counter.ge(Int::from_i64(0)));
    }
    if update.op() < 0 {
        optimizer.assert(counters[update.counter().to_usize()].ge(Int::from_i64(1)));
    }

    for (template, bound) in templates.iter().zip(source_bounds.iter()) {
        optimizer.assert(template_expression(template, &counters).ge(Int::from_i64(*bound as i64)));
    }

    let objective_template = &templates[objective_index];
    let objective = template_expression(objective_template, &counters)
        + Int::from_i64(
            (objective_template.coefficients[update.counter().to_usize()] * update.op()) as i64,
        );
    optimizer.minimize(&objective);

    match optimizer.check(&[]) {
        SatResult::Sat => optimizer
            .get_model()
            .and_then(|model| model.eval(&objective, true))
            .and_then(|value| value.as_i64())
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or(0)
            .clamp(0, cap),
        SatResult::Unsat | SatResult::Unknown => 0,
    }
}

fn template_expression(template: &LinearTemplate, counters: &[Int]) -> Int {
    counters
        .iter()
        .zip(template.coefficients.iter())
        .filter(|(_, coefficient)| **coefficient != 0)
        .fold(Int::from_i64(0), |sum, (counter, coefficient)| {
            sum + counter * Int::from_i64(*coefficient as i64)
        })
}

fn candidate_templates(
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &[LinearTemplate],
) -> Vec<LinearTemplate> {
    fn enumerate(
        position: usize,
        coefficients: &mut [i32],
        max_coefficient: i32,
        max_candidates: usize,
        existing: &[LinearTemplate],
        result: &mut Vec<LinearTemplate>,
    ) {
        if result.len() >= max_candidates {
            return;
        }
        if position == coefficients.len() {
            let support = coefficients
                .iter()
                .enumerate()
                .filter_map(|(counter, coefficient)| (*coefficient != 0).then_some(counter))
                .collect::<Vec<_>>();
            if support.len() < 2
                || existing
                    .iter()
                    .any(|template| template.coefficients.as_ref() == coefficients)
            {
                return;
            }
            result.push(LinearTemplate {
                coefficients: coefficients.to_vec().into_boxed_slice(),
            });
            return;
        }

        for coefficient in 0..=max_coefficient {
            coefficients[position] = coefficient;
            enumerate(
                position + 1,
                coefficients,
                max_coefficient,
                max_candidates,
                existing,
                result,
            );
        }
    }

    let mut result = Vec::new();
    enumerate(
        0,
        &mut vec![0; dimension],
        max_coefficient,
        max_candidates,
        existing,
        &mut result,
    );
    result
}

fn synthesize_template_for_boundaries(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current: &MainCFGTemplateLowerBounds,
    model_boundaries: &[(petgraph::graph::NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
) -> Option<(LinearTemplate, MainCFGTemplateLowerBounds)> {
    let candidates = candidate_templates(
        initial_valuation.dimension(),
        max_coefficient,
        max_candidates,
        &current.templates,
    );

    for template in candidates {
        let mut templates = current.templates.clone();
        templates.push(template.clone());
        let analysis = analyze_templates(cfg, initial_valuation, templates);
        let template_index = analysis.templates.len() - 1;

        let excludes_model = model_boundaries.iter().any(|(state, valuation)| {
            let Some(bounds) = &analysis.state_bounds[state.index()] else {
                return false;
            };
            template_value(&template, valuation) < bounds[template_index]
        });

        if excludes_model {
            return Some((template, analysis));
        }
    }

    None
}

fn linear_graph_boundary_template_lower_bounds(
    linear_graph: &ProductViewLinearGraph<'_>,
    main_bounds: &MainCFGTemplateLowerBounds,
    main_cfg_index: usize,
) -> HashMap<MultiGraphState, Vec<LinearTemplateLowerBound>> {
    linear_graph
        .iter_parts()
        .flat_map(|part| part.iter_nodes(linear_graph))
        .filter_map(|state| {
            main_bounds.state_bounds[state.cfg_state(main_cfg_index).index()]
                .as_ref()
                .map(|bounds| {
                    let lower_bounds = main_bounds
                        .templates
                        .iter()
                        .zip(bounds.iter())
                        .filter(|(_, bound)| **bound > 0)
                        .map(|(template, bound)| LinearTemplateLowerBound {
                            coefficients: template.coefficients.clone(),
                            bound: *bound,
                        })
                        .collect();
                    (state.clone(), lower_bounds)
                })
        })
        .collect()
}

fn refinement_steps_to_usize(max_refinements: u64) -> usize {
    usize::try_from(max_refinements.max(1)).unwrap_or(usize::MAX)
}

fn preferred_rooted_cycle(
    product: &ImplicitCFGProductView<'_>,
    root: &MultiGraphState,
    allowed: &HashSet<MultiGraphState>,
    preferred: Option<&CFGCounterUpdate>,
) -> Option<MultiGraphPath> {
    let mut first_letters = product.alphabet().iter().collect::<Vec<_>>();
    first_letters.sort_by_key(|letter| preferred != Some(*letter));
    let mut fallback = None;

    for first in first_letters {
        let Some(target) = product.successor(root, first) else {
            continue;
        };
        if !allowed.contains(&target) {
            continue;
        }

        let mut first_path = MultiGraphPath::new(root.clone());
        first_path.add(*first, target.clone());
        let cycle = if &target == root {
            Some(first_path)
        } else {
            shortest_path_to_root(product, first_path, root, allowed)
        };

        let Some(cycle) = cycle else {
            continue;
        };
        let nonzero_effect = cycle
            .transitions
            .iter()
            .fold(vec![0i32; product.dimension()], |mut effect, update| {
                effect[update.counter().to_usize()] += update.op();
                effect
            })
            .into_iter()
            .any(|effect| effect != 0);

        if nonzero_effect {
            return Some(cycle);
        }
        fallback.get_or_insert(cycle);
    }

    fallback
}

fn shortest_path_to_root(
    product: &ImplicitCFGProductView<'_>,
    initial_path: MultiGraphPath,
    root: &MultiGraphState,
    allowed: &HashSet<MultiGraphState>,
) -> Option<MultiGraphPath> {
    let mut queue = VecDeque::from([initial_path.clone()]);
    let mut visited = HashSet::new();
    visited.insert(initial_path.end().clone());

    while let Some(path) = queue.pop_front() {
        for letter in product.alphabet() {
            let Some(target) = product.successor(path.end(), letter) else {
                continue;
            };
            if !allowed.contains(&target) {
                continue;
            }

            let mut next = path.clone();
            next.add(*letter, target.clone());
            if &target == root {
                return Some(next);
            }

            if visited.insert(target) {
                queue.push_back(next);
            }
        }
    }

    None
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

#[cfg(test)]
mod tests {
    use super::{
        LinearTemplate, analyze_templates, candidate_templates, exact_successor_template_bound,
        main_cfg_template_lower_bounds, synthesize_template_for_boundaries,
    };
    use crate::automaton::{
        ModifiableAutomaton,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::node::DfaNode,
    };

    #[test]
    fn lower_bounds_preserve_a_mandatory_increment() {
        let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
        let initial = cfg.add_node(DfaNode::non_accepting(()));
        let accepting = cfg.add_node(DfaNode::accepting(()));
        cfg.set_initial(initial);
        cfg.add_edge(&initial, &accepting, CFGCounterUpdate::new(0, true));
        cfg.add_edge(&accepting, &initial, CFGCounterUpdate::new(0, false));

        let bounds = main_cfg_template_lower_bounds(&cfg, &vec![0].into());

        assert_eq!(
            bounds.state_bounds[initial.index()].as_deref(),
            Some(&[0][..])
        );
        assert_eq!(
            bounds.state_bounds[accepting.index()].as_deref(),
            Some(&[1][..])
        );
    }

    #[test]
    fn lower_bounds_are_weakened_by_decrement_cycles() {
        let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
        let initial = cfg.add_node(DfaNode::accepting(()));
        cfg.set_initial(initial);
        cfg.add_edge(&initial, &initial, CFGCounterUpdate::new(0, false));

        let bounds = main_cfg_template_lower_bounds(&cfg, &vec![100].into());

        assert_eq!(
            bounds.state_bounds[initial.index()].as_deref(),
            Some(&[0][..])
        );
    }

    #[test]
    fn template_lower_bounds_preserve_guarded_nonzero_sum() {
        let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
        let initial = cfg.add_node(DfaNode::accepting(()));
        let first_decrement = cfg.add_node(DfaNode::non_accepting(()));
        let second_decrement = cfg.add_node(DfaNode::non_accepting(()));
        let transfer = cfg.add_node(DfaNode::non_accepting(()));
        cfg.set_initial(initial);

        cfg.add_edge(&initial, &first_decrement, CFGCounterUpdate::new(0, true));
        cfg.add_edge(
            &first_decrement,
            &second_decrement,
            CFGCounterUpdate::new(1, false),
        );
        cfg.add_edge(&second_decrement, &initial, CFGCounterUpdate::new(1, false));
        cfg.add_edge(&initial, &transfer, CFGCounterUpdate::new(0, false));
        cfg.add_edge(&transfer, &initial, CFGCounterUpdate::new(1, true));

        let bounds = main_cfg_template_lower_bounds(&cfg, &vec![1, 0].into());
        let sum_template = bounds
            .templates
            .iter()
            .position(|template| template.coefficients.as_ref() == [1, 1])
            .unwrap();

        assert_eq!(
            bounds.state_bounds[initial.index()].as_ref().unwrap()[sum_template],
            1
        );
    }

    #[test]
    fn exact_transfer_combines_relational_constraints() {
        let templates = super::linear_templates(3);
        let source_bounds = vec![0, 0, 0, 2, 2, 2, 0];
        let all_counters = templates.len() - 1;
        let bound = exact_successor_template_bound(
            &templates,
            &source_bounds,
            &CFGCounterUpdate::new(0, true),
            all_counters,
            10,
        );

        assert_eq!(bound, 4);
    }

    #[test]
    fn candidate_generation_includes_weighted_templates() {
        let existing = super::linear_templates(2);
        let candidates = candidate_templates(2, 2, 32, &existing);

        assert!(candidates.iter().any(|template| {
            template.coefficients.as_ref() == [2, 1] || template.coefficients.as_ref() == [1, 2]
        }));
    }

    #[test]
    fn weighted_template_proves_a_non_default_invariant() {
        let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
        let initial = cfg.add_node(DfaNode::accepting(()));
        let dec_c0 = cfg.add_node(DfaNode::non_accepting(()));
        let first_inc_c1 = cfg.add_node(DfaNode::non_accepting(()));
        let inc_c0 = cfg.add_node(DfaNode::non_accepting(()));
        let first_dec_c1 = cfg.add_node(DfaNode::non_accepting(()));
        cfg.set_initial(initial);

        cfg.add_edge(&initial, &dec_c0, CFGCounterUpdate::new(0, false));
        cfg.add_edge(&dec_c0, &first_inc_c1, CFGCounterUpdate::new(1, true));
        cfg.add_edge(&first_inc_c1, &initial, CFGCounterUpdate::new(1, true));
        cfg.add_edge(&initial, &inc_c0, CFGCounterUpdate::new(0, true));
        cfg.add_edge(&inc_c0, &first_dec_c1, CFGCounterUpdate::new(1, false));
        cfg.add_edge(&first_dec_c1, &initial, CFGCounterUpdate::new(1, false));

        let mut templates = super::linear_templates(2);
        templates.push(LinearTemplate {
            coefficients: vec![2, 1].into_boxed_slice(),
        });
        let analysis = analyze_templates(&cfg, &vec![1, 0].into(), templates);
        let weighted = analysis.templates.len() - 1;

        assert_eq!(
            analysis.state_bounds[initial.index()].as_ref().unwrap()[weighted],
            2
        );
    }

    #[test]
    fn synthesis_discovers_a_weighted_separating_template() {
        let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(2));
        let initial = cfg.add_node(DfaNode::accepting(()));
        let dec_c0 = cfg.add_node(DfaNode::non_accepting(()));
        let first_inc_c1 = cfg.add_node(DfaNode::non_accepting(()));
        let inc_c0 = cfg.add_node(DfaNode::non_accepting(()));
        let first_dec_c1 = cfg.add_node(DfaNode::non_accepting(()));
        cfg.set_initial(initial);

        cfg.add_edge(&initial, &dec_c0, CFGCounterUpdate::new(0, false));
        cfg.add_edge(&dec_c0, &first_inc_c1, CFGCounterUpdate::new(1, true));
        cfg.add_edge(&first_inc_c1, &initial, CFGCounterUpdate::new(1, true));
        cfg.add_edge(&initial, &inc_c0, CFGCounterUpdate::new(0, true));
        cfg.add_edge(&inc_c0, &first_dec_c1, CFGCounterUpdate::new(1, false));
        cfg.add_edge(&first_dec_c1, &initial, CFGCounterUpdate::new(1, false));

        let initial_valuation = vec![1, 0].into();
        let current = main_cfg_template_lower_bounds(&cfg, &initial_valuation);
        let (template, _) = synthesize_template_for_boundaries(
            &cfg,
            &initial_valuation,
            &current,
            &[(initial, vec![0, 1].into())],
            2,
            32,
        )
        .unwrap();

        assert!(
            template.coefficients.as_ref() == [2, 1] || template.coefficients.as_ref() == [1, 2]
        );
    }
}
