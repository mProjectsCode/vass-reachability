use std::{
    collections::{BTreeSet, HashMap, HashSet},
    fs,
    path::{Path as FsPath, PathBuf},
    str::FromStr,
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::{
    automaton::{
        Alphabet, TransitionSystem,
        cfg::update::CFGCounterUpdate,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        path::Path,
        scc::SCCDag,
        vass::counter::VASSCounterValuation,
    },
    config::VASSReachConfig,
    utils::{now_unix_ms, sanitize_path_component, write_json_pretty_atomic},
};

const TRACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug)]
pub struct DebugTraceWriter {
    run_dir: PathBuf,
    steps_dir: PathBuf,
}

#[derive(Debug, Serialize)]
struct RunTraceIndex {
    schema_version: u32,
    run_name: String,
    instance_name: Option<String>,
    created_at_unix_ms: u128,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StepTraceSeed {
    pub schema_version: u32,
    pub step: u64,
    #[serde(default)]
    pub initial_valuation: Option<Vec<i32>>,
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
    pub nodes: Vec<Vec<usize>>,
    pub accepting_nodes: Vec<Vec<usize>>,
    #[serde(default)]
    pub internal_edges: Vec<SCCComponentEdgeSeed>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCComponentEdgeSeed {
    pub source: Vec<usize>,
    pub target: Vec<usize>,
    pub transition: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SCCDagEdgeSeed {
    pub target_component: usize,
    pub path: PathSeed,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PathSeed {
    pub states: Vec<Vec<usize>>,
    pub transitions: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DerivedSCCMetadata {
    pub component_sizes: Vec<usize>,
    pub accepting_sizes: Vec<usize>,
    pub cyclic_components: Vec<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryStateSeed {
    pub key: String,
    pub state: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStepSccViewSeed {
    pub component_index: usize,
    pub dot: String,
    pub entries: Vec<BoundaryStateSeed>,
    pub exits: Vec<BoundaryStateSeed>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceStepSccKarpMillerViewSeed {
    pub dot: String,
}

impl DebugTraceWriter {
    pub fn from_config(config: &VASSReachConfig) -> anyhow::Result<Option<Self>> {
        let trace_cfg = config.get_debug_trace();
        if !*trace_cfg.get_enabled() {
            return Ok(None);
        }

        let output_root = trace_cfg
            .get_output_root()
            .as_deref()
            .unwrap_or("debug/vass-reach");
        let root = PathBuf::from(output_root);

        let run_name = trace_cfg
            .get_run_name()
            .as_ref()
            .map(|name| sanitize_path_component(name, "unnamed"))
            .unwrap_or_else(default_run_name);

        let mut run_dir = root.join(run_name.clone());
        let instance_name = trace_cfg
            .get_instance_name()
            .as_ref()
            .map(|name| sanitize_path_component(name, "unnamed"));
        if let Some(instance_name) = &instance_name {
            run_dir = run_dir.join(instance_name);
        }

        let steps_dir = run_dir.join("steps");
        fs::create_dir_all(&steps_dir).with_context(|| {
            format!("failed to create trace directory: {}", steps_dir.display())
        })?;

        let index = RunTraceIndex {
            schema_version: TRACE_SCHEMA_VERSION,
            run_name,
            instance_name,
            created_at_unix_ms: now_unix_ms(),
        };

        write_json_pretty_atomic(&run_dir.join("run.json"), &index)?;

        Ok(Some(Self { run_dir, steps_dir }))
    }

    pub fn write_step_seed(
        &self,
        step: u64,
        initial_valuation: &VASSCounterValuation,
        path: &Path<MultiGraphState, CFGCounterUpdate>,
        dag: &SCCDag<MultiGraphState, CFGCounterUpdate>,
        product: &ImplicitCFGProduct,
        n_reaching: bool,
    ) -> anyhow::Result<()> {
        let file_name = format!("step_{step:06}.json");
        let step_path = self.steps_dir.join(file_name);

        let payload = StepTraceSeed {
            schema_version: TRACE_SCHEMA_VERSION,
            step,
            initial_valuation: Some(initial_valuation.iter().copied().collect()),
            found_path: FoundPathSeed {
                n_reaching,
                path: path_to_seed(path),
            },
            scc_dag: scc_dag_to_seed(dag, product),
        };

        write_json_pretty_atomic(&step_path, &payload)
    }

    pub fn run_dir(&self) -> &FsPath {
        &self.run_dir
    }
}

fn scc_dag_to_seed(
    dag: &SCCDag<MultiGraphState, CFGCounterUpdate>,
    product: &ImplicitCFGProduct,
) -> SCCDagSeed {
    SCCDagSeed {
        root_component: dag.root_component,
        trivial_paths_rolled: dag.trivial_paths_rolled,
        dot: dag.to_graphviz(None, None, true),
        components: dag
            .components
            .iter()
            .map(|component| {
                let component_state_set = component.nodes.iter().cloned().collect::<HashSet<_>>();
                let internal_edges =
                    collect_component_internal_edges(product, &component_state_set);

                SCCComponentSeed {
                    cyclic: component.cyclic,
                    nodes: component.nodes.iter().map(state_to_seed).collect(),
                    accepting_nodes: component
                        .accepting_nodes
                        .iter()
                        .map(state_to_seed)
                        .collect(),
                    internal_edges,
                }
            })
            .collect(),
        edges: dag
            .edges
            .iter()
            .map(|edges| {
                edges
                    .iter()
                    .map(|edge| SCCDagEdgeSeed {
                        target_component: edge.target_component,
                        path: path_to_seed(&edge.path),
                    })
                    .collect()
            })
            .collect(),
    }
}

fn collect_component_internal_edges(
    product: &ImplicitCFGProduct,
    component_states: &HashSet<MultiGraphState>,
) -> Vec<SCCComponentEdgeSeed> {
    let mut edges = Vec::new();

    for source in component_states {
        for transition in product.alphabet() {
            let Some(target) = product.successor(source, transition) else {
                continue;
            };

            if !component_states.contains(&target) {
                continue;
            }

            edges.push(SCCComponentEdgeSeed {
                source: state_to_seed(source),
                target: state_to_seed(&target),
                transition: transition.to_string(),
            });
        }
    }

    edges.sort_by(|left, right| {
        left.source
            .cmp(&right.source)
            .then(left.target.cmp(&right.target))
            .then(left.transition.cmp(&right.transition))
    });
    edges.dedup_by(|left, right| {
        left.source == right.source
            && left.target == right.target
            && left.transition == right.transition
    });

    edges
}

pub fn derive_scc_metadata(seed: &SCCDagSeed) -> DerivedSCCMetadata {
    let component_sizes = seed
        .components
        .iter()
        .map(|component| component.nodes.len())
        .collect();
    let accepting_sizes = seed
        .components
        .iter()
        .map(|component| component.accepting_nodes.len())
        .collect();
    let cyclic_components = seed
        .components
        .iter()
        .map(|component| component.cyclic)
        .collect();

    DerivedSCCMetadata {
        component_sizes,
        accepting_sizes,
        cyclic_components,
    }
}

pub fn derive_scc_component_view(
    seed: &StepTraceSeed,
    component_index: usize,
) -> anyhow::Result<TraceStepSccViewSeed> {
    let (entries, exits) = collect_component_boundaries(seed, component_index)?;
    let dot = build_component_dot(seed, component_index, &entries, &exits)?;

    Ok(TraceStepSccViewSeed {
        component_index,
        dot,
        entries,
        exits,
    })
}

fn build_component_dot(
    seed: &StepTraceSeed,
    component_index: usize,
    entries: &[BoundaryStateSeed],
    exits: &[BoundaryStateSeed],
) -> anyhow::Result<String> {
    let component = seed
        .scc_dag
        .components
        .get(component_index)
        .ok_or_else(|| anyhow::anyhow!("invalid component index"))?;

    let mut states = component.nodes.clone();
    states.sort();
    states.dedup();
    let state_set = states.iter().cloned().collect::<HashSet<_>>();

    let accepting = component
        .accepting_nodes
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let entry_keys = entries
        .iter()
        .map(|entry| entry.key.clone())
        .collect::<HashSet<_>>();
    let exit_keys = exits
        .iter()
        .map(|exit| exit.key.clone())
        .collect::<HashSet<_>>();
    let (entry_connections, exit_connections) = collect_boundary_connections(seed, component_index);

    let transitions = if let Some(component) = seed.scc_dag.components.get(component_index) {
        if component.internal_edges.is_empty() {
            collect_internal_component_transitions(seed, &state_set)?
        } else {
            component
                .internal_edges
                .iter()
                .map(|edge| {
                    Ok((
                        edge.source.clone(),
                        edge.target.clone(),
                        CFGCounterUpdate::from_str(&edge.transition)
                            .with_context(|| {
                                format!(
                                    "invalid CFGCounterUpdate in trace seed: {}",
                                    edge.transition
                                )
                            })?,
                    ))
                })
                .collect::<anyhow::Result<Vec<_>>>()?
        }
    } else {
        collect_internal_component_transitions(seed, &state_set)?
    };

    let mut dot = String::new();
    dot.push_str("digraph scc_component {\n");
    dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
    dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
    dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
    dot.push_str("rankdir=LR;\n");

    for state in states {
        let key = state_key(&state);
        let id = state_dot_id(&state);
        let mut attrs = vec![("label", format!("\"{}\"", key))];

        if accepting.contains(&state) {
            attrs.push(("shape", "doublecircle".to_string()));
        } else {
            attrs.push(("shape", "circle".to_string()));
        }

        if entry_keys.contains(&key) {
            attrs.push(("style", "filled".to_string()));
            attrs.push(("fillcolor", "\"#315f3a\"".to_string()));
        }

        if exit_keys.contains(&key) {
            attrs.push(("penwidth", "2".to_string()));
            attrs.push(("color", "\"#d8902a\"".to_string()));
        }

        dot.push_str(&format!(
            "{} [{}];\n",
            id,
            attrs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    for entry in entries {
        let entry_id = format!("ENTRY_{}", state_dot_suffix(&entry.state));
        let target_id = state_dot_id(&entry.state);
        let label = format_boundary_label("entry", entry_connections.get(&entry.key));
        dot.push_str(&format!(
            "{} [shape=point,label=\"\",color=\"#4caf50\"];\n",
            entry_id
        ));
        dot.push_str(&format!(
            "{} -> {} [style=dashed,color=\"#4caf50\",label=\"{}\"];\n",
            entry_id, target_id, label
        ));
    }

    for exit in exits {
        let source_id = state_dot_id(&exit.state);
        let exit_id = format!("EXIT_{}", state_dot_suffix(&exit.state));
        let label = format_boundary_label("exit", exit_connections.get(&exit.key));
        dot.push_str(&format!(
            "{} [shape=point,label=\"\",color=\"#ff9800\"];\n",
            exit_id
        ));
        dot.push_str(&format!(
            "{} -> {} [style=dashed,color=\"#ff9800\",label=\"{}\"];\n",
            source_id, exit_id, label
        ));
    }

    for (source, target, update) in transitions {
        dot.push_str(&format!(
            "{} -> {} [label=\"{}\"];\n",
            state_dot_id(&source),
            state_dot_id(&target),
            update
        ));
    }

    dot.push_str("}\n");
    Ok(dot)
}

fn collect_component_boundaries(
    seed: &StepTraceSeed,
    component_index: usize,
) -> anyhow::Result<(Vec<BoundaryStateSeed>, Vec<BoundaryStateSeed>)> {
    let component = seed
        .scc_dag
        .components
        .get(component_index)
        .ok_or_else(|| anyhow::anyhow!("invalid component index"))?;

    let state_set = component
        .nodes
        .iter()
        .cloned()
        .collect::<HashSet<Vec<usize>>>();

    let mut entries = Vec::new();
    let mut entry_seen = HashSet::new();
    let mut exits = Vec::new();
    let mut exit_seen = HashSet::new();

    if component_index == seed.scc_dag.root_component
        && let Some(root_state) = seed.found_path.path.states.first()
        && state_set.contains(root_state)
    {
        let key = state_key(root_state);
        if entry_seen.insert(key.clone()) {
            entries.push(BoundaryStateSeed {
                key,
                state: root_state.clone(),
            });
        }
    }

    for (source_component, outgoing) in seed.scc_dag.edges.iter().enumerate() {
        for edge in outgoing {
            if edge.target_component == component_index
                && let Some(entry_state) = edge.path.states.last()
                && state_set.contains(entry_state)
            {
                let key = state_key(entry_state);
                if entry_seen.insert(key.clone()) {
                    entries.push(BoundaryStateSeed {
                        key,
                        state: entry_state.clone(),
                    });
                }
            }

            if source_component == component_index
                && let Some(exit_state) = edge.path.states.first()
                && state_set.contains(exit_state)
            {
                let key = state_key(exit_state);
                if exit_seen.insert(key.clone()) {
                    exits.push(BoundaryStateSeed {
                        key,
                        state: exit_state.clone(),
                    });
                }
            }
        }
    }

    entries.sort_by(|left, right| left.key.cmp(&right.key));
    exits.sort_by(|left, right| left.key.cmp(&right.key));

    Ok((entries, exits))
}

fn collect_boundary_connections(
    seed: &StepTraceSeed,
    component_index: usize,
) -> (
    HashMap<String, BTreeSet<usize>>,
    HashMap<String, BTreeSet<usize>>,
) {
    let mut entry_connections: HashMap<String, BTreeSet<usize>> = HashMap::new();
    let mut exit_connections: HashMap<String, BTreeSet<usize>> = HashMap::new();

    for (source_component, outgoing) in seed.scc_dag.edges.iter().enumerate() {
        for edge in outgoing {
            if edge.target_component == component_index
                && let Some(entry_state) = edge.path.states.last()
            {
                entry_connections
                    .entry(state_key(entry_state))
                    .or_default()
                    .insert(source_component);
            }

            if source_component == component_index
                && let Some(exit_state) = edge.path.states.first()
            {
                exit_connections
                    .entry(state_key(exit_state))
                    .or_default()
                    .insert(edge.target_component);
            }
        }
    }

    (entry_connections, exit_connections)
}

fn format_boundary_label(prefix: &str, connected_components: Option<&BTreeSet<usize>>) -> String {
    let Some(connected_components) = connected_components else {
        return prefix.to_string();
    };

    if connected_components.is_empty() {
        return prefix.to_string();
    }

    let sccs = connected_components
        .iter()
        .map(|component| format!("SCC {}", component))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{} {}", prefix, sccs)
}

fn collect_internal_component_transitions(
    seed: &StepTraceSeed,
    state_set: &HashSet<Vec<usize>>,
) -> anyhow::Result<Vec<(Vec<usize>, Vec<usize>, CFGCounterUpdate)>> {
    let mut transitions = Vec::new();

    collect_internal_transitions_from_path(&seed.found_path.path, state_set, &mut transitions)?;
    for outgoing in seed.scc_dag.edges.iter() {
        for edge in outgoing {
            collect_internal_transitions_from_path(&edge.path, state_set, &mut transitions)?;
        }
    }

    Ok(transitions)
}

fn collect_internal_transitions_from_path(
    path: &PathSeed,
    state_set: &HashSet<Vec<usize>>,
    output: &mut Vec<(Vec<usize>, Vec<usize>, CFGCounterUpdate)>,
) -> anyhow::Result<()> {
    let edge_count = path
        .transitions
        .len()
        .min(path.states.len().saturating_sub(1));
    for i in 0..edge_count {
        let source = path.states[i].clone();
        let target = path.states[i + 1].clone();
        if !state_set.contains(&source) || !state_set.contains(&target) {
            continue;
        }

        let transition = &path.transitions[i];
        let update = CFGCounterUpdate::from_str(transition)
            .with_context(|| format!("invalid CFGCounterUpdate in trace path: {}", transition))?;
        output.push((source, target, update));
    }

    Ok(())
}

fn state_key(state: &[usize]) -> String {
    state
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

fn state_dot_suffix(state: &[usize]) -> String {
    state
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("_")
}

fn state_dot_id(state: &[usize]) -> String {
    format!("S_{}", state_dot_suffix(state))
}

fn path_to_seed(path: &Path<MultiGraphState, CFGCounterUpdate>) -> PathSeed {
    PathSeed {
        states: path.states.iter().map(state_to_seed).collect(),
        transitions: path.transitions.iter().map(ToString::to_string).collect(),
    }
}

fn state_to_seed(state: &MultiGraphState) -> Vec<usize> {
    state.states.iter().map(|index| index.index()).collect()
}

fn default_run_name() -> String {
    format!("run-{}", now_unix_ms())
}
