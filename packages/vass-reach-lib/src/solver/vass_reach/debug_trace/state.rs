use super::seeds::PathSeed;
use crate::{
    automaton::{
        cfg::update::CFGCounterUpdate, implicit_cfg_product::state::MultiGraphState, path::Path,
    },
    utils::now_unix_ms,
};

pub(super) fn state_key(state: &MultiGraphState) -> String {
    state
        .states
        .iter()
        .map(|idx| idx.index().to_string())
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn state_dot_suffix(state: &MultiGraphState) -> String {
    state
        .states
        .iter()
        .map(|idx| idx.index().to_string())
        .collect::<Vec<_>>()
        .join("_")
}

pub(super) fn state_dot_id(state: &MultiGraphState) -> String {
    format!("S_{}", state_dot_suffix(state))
}

pub(super) fn path_to_seed(path: &Path<MultiGraphState, CFGCounterUpdate>) -> PathSeed {
    PathSeed {
        states: path.states.clone(),
        transitions: path.transitions.clone(),
    }
}

pub(super) fn default_run_name() -> String {
    format!("run-{}", now_unix_ms())
}
