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
        modulo: ModuloConfig (Option<PartialModuloConfig> = ModuloConfig::default()),
        lts: LTSConfig (Option<PartialLTSConfig> = LTSConfig::default()),
        mgts: MGTSConfig (Option<PartialMGTSConfig> = MGTSConfig::default()),
    }
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
pub enum ExtensionStrategyConfig {
    Random,
    RandomSCC,
    CompletePartialSCC,
}

config! {
    pub struct MGTSConfig {
        enabled: bool = true,
        max_refinement_steps: u64 = 10,
        strategy: ExtensionStrategyConfig = ExtensionStrategyConfig::CompletePartialSCC,
    }
}

config! {
    pub struct VASSZReachConfig {
        timeout: Option<std::time::Duration> = None,
        max_iterations: Option<u64> = None,
    }
}
