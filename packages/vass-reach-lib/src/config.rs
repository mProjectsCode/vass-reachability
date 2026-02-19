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
        modulo: ModuloConfig (Option<PartialModuloConfig> = ModuloConfig::default()),
        lts: LTSConfig (Option<PartialLTSConfig> = LTSConfig::default()),
        lsg: LSGConfig (Option<PartialLSGConfig> = LSGConfig::default()),
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
}

config! {
    pub struct LSGConfig {
        enabled: bool = true,
        max_refinement_steps: u64 = 5,
        strategy: ExtensionStrategyConfig = ExtensionStrategyConfig::RandomSCC,
    }
}

config! {
    pub struct VASSZReachConfig {
        timeout: Option<std::time::Duration> = None,
        max_iterations: Option<u64> = None,
    }
}
