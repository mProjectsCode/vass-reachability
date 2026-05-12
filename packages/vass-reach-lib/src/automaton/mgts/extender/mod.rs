use std::fmt::Debug;

use hashbrown::HashSet;

use crate::{
    automaton::{
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        mgts::MGTS,
        path::Path,
        scc::{SCCAlgorithms, SCCDag},
        vass::counter::VASSCounterValuation,
    },
    solver::{SolverStatus, mgts_reach::MGTSReachSolverOptions},
};

mod layout;

use layout::{CandidateSeed, InterpolationLayout};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

/// Builds a large unreachable MGTS between one or more seed-language lower
/// bounds and the full SCCs of the current product approximation.
#[derive(Debug)]
pub struct MGTSExtender<'a> {
    /// The selected unreachable MGTS. This is updated by `run_mgts`.
    pub mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    primary_path: MultiGraphPath,
    auxiliary_paths: Vec<MultiGraphPath>,
    /// The subset of seed paths currently represented by `seed_mgts`.
    selected_path_indices: Vec<usize>,
    /// Reference to the underlying CFG.
    pub product: &'a ImplicitCFGProduct,
    /// Dimension of the CFG.
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    /// Maximum number of refinement steps to perform.
    pub max_refinements: u64,
    /// The best seed-language MGTS found for the selected seed subset.
    seed_mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    /// Interpolation layout for the selected seed subset.
    layout: Option<InterpolationLayout<'a>>,
    /// Optional SCC DAG supplied by a caller that already computed it.
    scc_dag: Option<SCCDag<MultiGraphState, CFGCounterUpdate>>,
}

impl<'a> MGTSExtender<'a> {
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
        Self::from_primary_path(
            path,
            Vec::new(),
            product,
            dimension,
            initial_valuation,
            final_valuation,
            max_refinements,
        )
    }

    /// Creates an extender from one or more seed paths.
    ///
    /// The first path is the primary path that must be covered by any selected
    /// MGTS. Remaining paths are auxiliary paths that may enrich the primary
    /// path when they take the same SCC-DAG route.
    pub fn from_paths(
        paths: Vec<MultiGraphPath>,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        assert!(
            !paths.is_empty(),
            "MGTSExtender requires at least one seed path"
        );

        let mut paths = paths;
        let primary_path = paths.remove(0);

        Self::from_primary_path(
            primary_path,
            paths,
            product,
            dimension,
            initial_valuation,
            final_valuation,
            max_refinements,
        )
    }

    /// Creates an extender from a primary path and auxiliary paths.
    ///
    /// Every selected MGTS must include the primary path. Auxiliary paths are
    /// only used when they take the same SCC-DAG route as the primary path, so
    /// they can add seed nodes without changing the full MGTS shape being cut.
    pub fn from_primary_path(
        primary_path: MultiGraphPath,
        auxiliary_paths: Vec<MultiGraphPath>,
        product: &'a ImplicitCFGProduct,
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        max_refinements: u64,
    ) -> Self {
        let initial_mgts = MGTS::from_path(primary_path.clone(), product, dimension);

        MGTSExtender {
            mgts: initial_mgts.clone(),
            primary_path,
            auxiliary_paths,
            selected_path_indices: vec![0],
            dimension,
            product,
            initial_valuation,
            final_valuation,
            max_refinements,
            seed_mgts: initial_mgts,
            layout: None,
            scc_dag: None,
        }
    }

    /// Reuses a precomputed SCC DAG for route-compatible MGTS layout building.
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

    /// Refines `self.mgts` by searching between the seed-language MGTS and the
    /// full-SCC MGTS induced by the current product.
    pub fn run(&mut self) -> VASSCFG<()> {
        self.run_mgts().to_cfg()
    }

    /// Runs the extender and returns the selected unreachable MGTS.
    ///
    /// The public `run` method still returns a CFG because that is what the
    /// VASS reachability refinement consumes, but tests and future call sites
    /// can use this method to inspect the chosen MGTS directly.
    pub fn run_mgts(&mut self) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
        let _span = tracing::span!(tracing::Level::DEBUG, "MGTSExtender::run_mgts").entered();

        let max_checks_per_phase =
            usize::try_from(self.max_refinements.max(1)).unwrap_or(usize::MAX);
        let mut seed_checks = 0usize;

        let Some(seed) = self.select_initial_seed(max_checks_per_phase, &mut seed_checks) else {
            return self.fallback_to_exact_primary_path();
        };

        let layout = self.install_initial_seed(seed, seed_checks);
        let best = self.seed_mgts.clone();
        let mut checks = 0usize;

        if self.interpolation_is_exhausted(&layout, checks, max_checks_per_phase) {
            return self.finish_with_mgts(best);
        }

        tracing::debug!(
            regions = layout.regions.len(),
            seed_size = best.size(),
            "Starting interpolated MGTS search"
        );

        if let Some(full) = self.try_full_scc_upper_bound(&layout, &mut checks) {
            return self.finish_with_mgts(full);
        }

        let best = self.search_interpolated_regions(&layout, best, checks, max_checks_per_phase);

        self.finish_with_mgts(best)
    }

    /// Keeps the exact primary path when no larger seed-language candidate can
    /// be proved unreachable.
    fn fallback_to_exact_primary_path(&mut self) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
        tracing::debug!("No seed MGTS was proved unreachable; keeping exact first path MGTS");

        let exact = MGTS::from_path(self.primary_path.clone(), self.product, self.dimension);
        let result = self.solve_candidate(&exact);

        debug_assert!(
            matches!(&result.status, SolverStatus::False(_)),
            "Exact primary path MGTS must be unreachable when used as fallback"
        );
        if !matches!(&result.status, SolverStatus::False(_)) {
            tracing::warn!(
                status = ?result.status,
                "Exact primary path MGTS was not proved unreachable during fallback"
            );
        }

        self.finish_with_mgts(exact)
    }

    /// Records the selected seed as the lower bound for interpolation.
    fn install_initial_seed(
        &mut self,
        seed: CandidateSeed<'a>,
        seed_checks: usize,
    ) -> InterpolationLayout<'a> {
        let layout = seed.layout;

        self.selected_path_indices = seed.path_indices;
        self.seed_mgts = seed.seed_mgts;
        self.layout = Some(layout.clone());
        self.mgts = self.seed_mgts.clone();

        tracing::debug!(
            size = self.seed_mgts.size(),
            selected_paths = self.selected_path_indices.len(),
            seed_checks,
            "Seed-language MGTS is unreachable; using it as search lower bound"
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
    ) -> Option<MGTS<'a, MultiGraphState, ImplicitCFGProduct>> {
        let full_mask = vec![true; layout.regions.len()];
        let full = layout.build_candidate(&full_mask);
        let full_result = self.solve_candidate(&full.mgts);
        *checks += 1;

        if matches!(full_result.status, SolverStatus::False(_)) {
            tracing::debug!(
                size = full.mgts.size(),
                checks = *checks,
                "Full-SCC MGTS is unreachable"
            );
            return Some(full.mgts);
        }

        None
    }

    /// Grows the seed candidate by enabling batches of SCC regions and keeping
    /// only the unreachable expansions.
    fn search_interpolated_regions(
        &self,
        layout: &InterpolationLayout<'a>,
        mut best: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
        mut checks: usize,
        max_checks: usize,
    ) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
        let mut accepted = vec![false; layout.regions.len()];
        let mut pending = (0..layout.regions.len()).collect::<Vec<_>>();
        pending.sort_by_key(|region| std::cmp::Reverse(layout.regions[*region].gain()));

        let mut batch_size = pending.len().div_ceil(2).max(1);

        while checks < max_checks && !pending.is_empty() {
            let batch_len = batch_size.min(pending.len()).max(1);
            let batch = pending[..batch_len].to_vec();

            // Try one extra region batch on top of the known-unreachable mask.
            let candidate_mask = mask_with_batch(&accepted, &batch);
            let candidate = layout.build_candidate(&candidate_mask);
            let candidate_result = self.solve_candidate(&candidate.mgts);
            checks += 1;

            match candidate_result.status {
                SolverStatus::False(_) => {
                    tracing::debug!(
                        size = candidate.mgts.size(),
                        enabled_regions = candidate_mask.iter().filter(|enabled| **enabled).count(),
                        checks,
                        "Interpolated candidate is unreachable"
                    );

                    accepted = candidate_mask;
                    best = candidate.mgts;
                    pending.retain(|region| !batch.contains(region));
                    batch_size = pending.len().div_ceil(2).max(1);
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

                    if used_in_batch.is_empty() {
                        if batch_len == 1 {
                            pending.retain(|region| *region != batch[0]);
                        } else {
                            batch_size = batch_len.div_ceil(2).max(1);
                        }
                    } else {
                        pending.retain(|region| !used_in_batch.contains(region));
                        batch_size = pending.len().div_ceil(2).max(1);
                    }
                }
                SolverStatus::Unknown(reason) => {
                    tracing::debug!(
                        ?reason,
                        batch_len,
                        checks,
                        "Interpolated candidate returned unknown"
                    );

                    if batch_len == 1 {
                        pending.retain(|region| *region != batch[0]);
                    } else {
                        batch_size = batch_len.div_ceil(2).max(1);
                    }
                }
            }
        }

        tracing::debug!(
            size = best.size(),
            enabled_regions = accepted.iter().filter(|enabled| **enabled).count(),
            checks,
            max_checks,
            "Finished interpolated MGTS search"
        );

        best
    }

    /// Stores and returns the MGTS chosen by the current search phase.
    fn finish_with_mgts(
        &mut self,
        mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    ) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
        self.mgts = mgts.clone();
        mgts
    }

    /// Finds a large path-compatible seed-language MGTS that is still
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

        candidates.sort_by_key(|candidate| {
            std::cmp::Reverse((candidate.path_indices.len(), candidate.seed_mgts.size()))
        });

        for candidate in candidates {
            if *checks >= max_checks {
                break;
            }

            let result = self.solve_candidate(&candidate.seed_mgts);
            *checks += 1;

            if matches!(result.status, SolverStatus::False(_)) {
                tracing::debug!(
                    selected_paths = candidate.path_indices.len(),
                    size = candidate.seed_mgts.size(),
                    checks,
                    "Selected path subset for initial MGTS"
                );
                return Some(candidate);
            }
        }

        None
    }

    /// Checks whether a candidate MGTS is reachable between the configured
    /// boundary valuations.
    fn solve_candidate(
        &self,
        mgts: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    ) -> crate::solver::mgts_reach::MGTSReachSolverResult {
        MGTSReachSolverOptions::default()
            .to_solver(mgts, &self.initial_valuation, &self.final_valuation)
            .solve()
    }
}

fn mask_with_batch(accepted: &[bool], batch: &[usize]) -> Vec<bool> {
    let mut mask = accepted.to_vec();

    for region in batch {
        mask[*region] = true;
    }

    mask
}
