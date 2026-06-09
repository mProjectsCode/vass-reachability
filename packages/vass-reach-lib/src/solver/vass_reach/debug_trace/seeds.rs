use serde::{Deserialize, Serialize};

use crate::automaton::{
    cfg::update::CFGCounterUpdate, implicit_cfg_product::state::MultiGraphState,
    vass::counter::VASSCounterValuation,
};

pub(super) const TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
pub struct LightTraceSummary {
    pub schema_version: u32,
    pub run_name: String,
    pub instance_name: Option<String>,
    pub created_at_unix_ms: u128,
    pub dimension: usize,
    pub state_count: usize,
    pub transition_count: usize,
    pub initial_valuation: Vec<i32>,
    pub final_valuation: Vec<i32>,
    pub initial_graph_dot: String,
    #[serde(default)]
    pub result: Option<LightTraceResult>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LightTraceResult {
    pub status: String,
    pub reason: Option<String>,
    pub elapsed_ms: u128,
    pub step_count: u64,
    pub mu: Vec<i32>,
    pub forwards_bound: Vec<u32>,
    pub backwards_bound: Vec<u32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepTraceSeed {
    pub schema_version: u32,
    pub step: u64,
    #[serde(default)]
    pub initial_valuation: Option<VASSCounterValuation>,
    pub found_path: FoundPathSeed,
    pub scc_dag: SCCDagSeed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FoundPathSeed {
    pub n_reaching: bool,
    pub path: PathSeed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCDagSeed {
    pub root_component: usize,
    pub trivial_paths_rolled: bool,
    pub dot: String,
    pub components: Vec<SCCComponentSeed>,
    pub edges: Vec<Vec<SCCDagEdgeSeed>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCComponentSeed {
    pub cyclic: bool,
    pub nodes: Vec<MultiGraphState>,
    pub accepting_nodes: Vec<MultiGraphState>,
    #[serde(default)]
    pub internal_edges: Vec<SCCComponentEdgeSeed>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCComponentEdgeSeed {
    pub source: MultiGraphState,
    pub target: MultiGraphState,
    pub transition: CFGCounterUpdate,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCDagEdgeSeed {
    pub target_component: usize,
    pub path: PathSeed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathSeed {
    pub states: Vec<MultiGraphState>,
    pub transitions: Vec<CFGCounterUpdate>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DerivedSCCMetadata {
    pub component_sizes: Vec<usize>,
    pub accepting_sizes: Vec<usize>,
    pub cyclic_components: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStepSccViewSeed {
    pub component_index: usize,
    pub dot: String,
    pub entries: Vec<MultiGraphState>,
    pub exits: Vec<MultiGraphState>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStepSccCounterEffectSetSeed {
    pub component_index: usize,
    pub entry: MultiGraphState,
    pub start_value: i64,
    pub dimension: usize,
    pub total_cycles: usize,
    pub capped: bool,
    pub basic_cycles: Vec<SccCycleCounterEffectSeed>,
    pub effect_set: Vec<SccCounterEffectRepresentativeSeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SccCycleCounterEffectSeed {
    pub states: Vec<MultiGraphState>,
    pub transitions: Vec<CFGCounterUpdate>,
    pub effect: Vec<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SccCounterEffectRepresentativeSeed {
    pub effect: Vec<i64>,
    pub sample_cycle: SccCycleCounterEffectSeed,
}
