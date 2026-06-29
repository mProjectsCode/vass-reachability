use std::time::Duration;

use crate::config::{
    LinearGraphConfig, LinearGraphInterpolationStrategy, LinearGraphRegionOrder,
    LinearGraphSeedOrder, LinearGraphTemplateFamily,
};

#[derive(Debug, Clone)]
pub(super) struct LinearGraphExtenderOptions {
    pub(super) overall_time_limit: Option<Duration>,
    pub(super) max_seed_checks: usize,
    pub(super) max_interpolation_steps: usize,
    pub(super) check_full_scc_upper_bound: bool,
    pub(super) interpolation_strategy: LinearGraphInterpolationStrategy,
    pub(super) region_order: LinearGraphRegionOrder,
    pub(super) seed_order: LinearGraphSeedOrder,
    pub(super) reach_solver_max_iterations: Option<u32>,
    pub(super) reach_solver_timeout: Option<Duration>,
    pub(super) template_exact_transfer_enabled: bool,
    pub(super) template_exact_transfer_max_templates: usize,
    pub(super) template_synthesis_enabled: bool,
    pub(super) template_synthesis_max_coefficient: i32,
    pub(super) template_synthesis_candidate_limit: usize,
    pub(super) template_synthesis_round_limit: usize,
    pub(super) initial_template_families: Vec<LinearGraphTemplateFamily>,
}

impl LinearGraphExtenderOptions {
    pub(super) fn from_refinement_steps(max_refinements: u64) -> Self {
        let max_checks = refinement_steps_to_usize(max_refinements);

        Self {
            overall_time_limit: None,
            max_seed_checks: max_checks,
            max_interpolation_steps: max_checks,
            check_full_scc_upper_bound: true,
            interpolation_strategy: LinearGraphInterpolationStrategy::AdaptiveBatch,
            region_order: LinearGraphRegionOrder::GainDescending,
            seed_order: LinearGraphSeedOrder::MorePathsThenSize,
            reach_solver_max_iterations: None,
            reach_solver_timeout: None,
            template_exact_transfer_enabled: true,
            template_exact_transfer_max_templates: 8,
            template_synthesis_enabled: true,
            template_synthesis_max_coefficient: 2,
            template_synthesis_candidate_limit: 256,
            template_synthesis_round_limit: 8,
            initial_template_families: vec![
                LinearGraphTemplateFamily::Singleton,
                LinearGraphTemplateFamily::Pair,
                LinearGraphTemplateFamily::All,
            ],
        }
    }

    pub(super) fn from_config(config: &LinearGraphConfig) -> Self {
        let default_checks = refinement_steps_to_usize(*config.get_max_refinement_steps());

        Self {
            overall_time_limit: None,
            max_seed_checks: (*config.get_max_seed_checks()).unwrap_or(default_checks),
            max_interpolation_steps: (*config.get_max_interpolation_steps())
                .unwrap_or(default_checks),
            check_full_scc_upper_bound: *config.get_check_full_scc_upper_bound(),
            interpolation_strategy: *config.get_interpolation_strategy(),
            region_order: *config.get_region_order(),
            seed_order: *config.get_seed_order(),
            reach_solver_max_iterations: *config.get_reach_solver_max_iterations(),
            reach_solver_timeout: *config.get_reach_solver_timeout(),
            template_exact_transfer_enabled: *config.get_template_exact_transfer_enabled(),
            template_exact_transfer_max_templates: *config
                .get_template_exact_transfer_max_templates(),
            template_synthesis_enabled: *config.get_template_synthesis_enabled(),
            template_synthesis_max_coefficient: *config
                .get_template_synthesis_max_coefficient()
                .max(&0),
            template_synthesis_candidate_limit: *config.get_template_synthesis_candidate_limit(),
            template_synthesis_round_limit: *config.get_template_synthesis_round_limit(),
            initial_template_families: config.get_initial_template_families().clone(),
        }
    }
}

fn refinement_steps_to_usize(max_refinements: u64) -> usize {
    usize::try_from(max_refinements.max(1)).unwrap_or(usize::MAX)
}
