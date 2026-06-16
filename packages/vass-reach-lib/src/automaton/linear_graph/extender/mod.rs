use std::{cell::RefCell, fmt::Debug};

use hashbrown::HashSet;

use crate::{
    automaton::{
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{state::MultiGraphState, view::ImplicitCFGProductView},
        linear_graph::LinearGraph,
        path::Path,
        scc::{SCCAlgorithms, SCCDag},
        vass::counter::VASSCounterValuation,
    },
    config::{LinearGraphConfig, LinearGraphRegionOrder, LinearGraphSeedOrder},
    solver::{SolverStatus, linear_graph_reach::LinearGraphReachSolverOptions},
};

mod cycles;
mod layout;
mod options;
mod strategy;
mod templates;
#[doc(hidden)]
pub mod template_testing {
    pub use super::templates::testing::*;
}

use cycles::preferred_rooted_cycle;
use layout::{CandidateSeed, InterpolationLayout};
use options::LinearGraphExtenderOptions;
use strategy::interpolation_strategy;
use templates::{
    LinearTemplate, MainCFGTemplateLowerBounds, linear_graph_boundary_template_lower_bounds,
    main_cfg_template_lower_bounds, synthesize_template_for_boundaries,
};

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
