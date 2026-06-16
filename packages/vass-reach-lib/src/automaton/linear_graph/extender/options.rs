use std::time::Duration;

use crate::config::{
    LinearGraphConfig, LinearGraphInterpolationStrategy, LinearGraphRegionOrder,
    LinearGraphSeedOrder,
};

#[derive(Debug, Clone)]
pub(super) struct LinearGraphExtenderOptions {
    pub(super) max_seed_checks: usize,
    pub(super) max_interpolation_steps: usize,
    pub(super) check_full_scc_upper_bound: bool,
    pub(super) interpolation_strategy: LinearGraphInterpolationStrategy,
    pub(super) region_order: LinearGraphRegionOrder,
    pub(super) seed_order: LinearGraphSeedOrder,
    pub(super) reach_solver_max_iterations: Option<u32>,
    pub(super) reach_solver_timeout: Option<Duration>,
}

impl LinearGraphExtenderOptions {
    pub(super) fn from_refinement_steps(max_refinements: u64) -> Self {
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

    pub(super) fn from_config(config: &LinearGraphConfig) -> Self {
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

fn refinement_steps_to_usize(max_refinements: u64) -> usize {
    usize::try_from(max_refinements.max(1)).unwrap_or(usize::MAX)
}
