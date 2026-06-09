use serde::{Deserialize, Serialize};
use vass_reach_macros::config;

pub trait IntoOr<T> {
    fn into_or(self, or: T) -> T;
}

impl<T> IntoOr<Option<T>> for Option<T> {
    fn into_or(self, or: Option<T>) -> Option<T> {
        match self {
            Some(t) => Some(t),
            None => or,
        }
    }
}

impl<T> IntoOr<T> for Option<T> {
    fn into_or(self, or: T) -> T {
        self.unwrap_or(or)
    }
}

config! {
    pub struct VASSReachConfig {
        timeout: Option<std::time::Duration> = None,
        max_iterations: Option<u64> = None,
        consider_modulo_for_pumping: bool = false,
        bounded_counting_enabled: bool = true,
        preprocessing: PreprocessingConfig (Option<PartialPreprocessingConfig> = PreprocessingConfig::default()),
        modulo: ModuloConfig (Option<PartialModuloConfig> = ModuloConfig::default()),
        lts: LTSConfig (Option<PartialLTSConfig> = LTSConfig::default()),
        linear_graph: LinearGraphConfig (Option<PartialLinearGraphConfig> = LinearGraphConfig::default()),
        debug_trace: DebugTraceConfig (Option<PartialDebugTraceConfig> = DebugTraceConfig::default()),
    }
}

config! {
    pub struct PreprocessingConfig {
        enabled: bool = false,
        z_reach_precheck_enabled: bool = false,
        max_linear_graph_candidates: usize = 256,
    }
}

config! {
    pub struct DebugTraceConfig {
        enabled: bool = false,
        level: DebugTraceLevel = DebugTraceLevel::Full,
        output_root: Option<String> = None,
        run_name: Option<String> = None,
        instance_name: Option<String> = None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DebugTraceLevel {
    Disabled,
    Light,
    Full,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum ModuloMode {
    Increment,
    LeastCommonMultiple,
}

config! {
    pub struct ModuloConfig {
        mode: ModuloMode = ModuloMode::LeastCommonMultiple,
    }
}

config! {
    pub struct LTSConfig {
        enabled: bool = true,
        relaxed_enabled: bool = true,
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LinearGraphInterpolationStrategy {
    AdaptiveBatch,
    Linear,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LinearGraphRegionOrder {
    GainDescending,
    GainAscending,
    Input,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum LinearGraphSeedOrder {
    MorePathsThenSize,
    LargerSeedFirst,
    SmallerSeedFirst,
}

config! {
    pub struct LinearGraphConfig {
        enabled: bool = true,
        multiple_starting_paths_enabled: bool = false,
        extra_auxiliary_paths: usize = 0,
        max_refinement_steps: u64 = 10,
        max_seed_checks: Option<usize> = None,
        max_interpolation_steps: Option<usize> = None,
        check_full_scc_upper_bound: bool = true,
        interpolation_strategy: LinearGraphInterpolationStrategy = LinearGraphInterpolationStrategy::AdaptiveBatch,
        region_order: LinearGraphRegionOrder = LinearGraphRegionOrder::GainDescending,
        seed_order: LinearGraphSeedOrder = LinearGraphSeedOrder::MorePathsThenSize,
        reach_solver_max_iterations: Option<u32> = None,
        reach_solver_timeout: Option<std::time::Duration> = None,
    }
}

config! {
    pub struct VASSZReachConfig {
        timeout: Option<std::time::Duration> = None,
        max_iterations: Option<u64> = None,
    }
}
