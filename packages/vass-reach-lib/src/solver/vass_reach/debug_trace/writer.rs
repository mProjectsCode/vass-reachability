use std::{
    collections::HashSet,
    fs,
    path::{Path as FsPath, PathBuf},
};

use anyhow::Context;
use serde::Serialize;

use super::{
    seeds::{
        FoundPathSeed, LightTraceResult, LightTraceSummary, SCCComponentEdgeSeed, SCCComponentSeed,
        SCCDagEdgeSeed, SCCDagSeed, StepTraceSeed, TRACE_SCHEMA_VERSION,
    },
    state::{default_run_name, path_to_seed},
};
use crate::{
    automaton::{
        Alphabet, TransitionSystem,
        algorithms::EdgeAutomatonAlgorithms,
        cfg::update::CFGCounterUpdate,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        path::Path,
        scc::SCCDag,
        vass::{counter::VASSCounterValuation, initialized::InitializedVASS},
    },
    config::{DebugTraceLevel, VASSReachConfig},
    solver::vass_reach::{VASSReachSolverResult, VASSReachSolverStatistics},
    utils::{now_unix_ms, sanitize_path_component, write_json_pretty_atomic},
};

#[derive(Debug)]
pub struct DebugTraceWriter {
    run_dir: PathBuf,
    steps_dir: Option<PathBuf>,
    light_summary: Option<LightTraceSummary>,
}

#[derive(Debug, Serialize)]
struct RunTraceIndex {
    schema_version: u32,
    run_name: String,
    instance_name: Option<String>,
    created_at_unix_ms: u128,
}

impl DebugTraceWriter {
    pub fn from_config<N, E>(
        config: &VASSReachConfig,
        ivass: &InitializedVASS<N, E>,
    ) -> anyhow::Result<Option<Self>>
    where
        N: crate::automaton::AutomatonNode,
        E: crate::automaton::AutomatonEdge + crate::automaton::FromLetter,
    {
        let trace_cfg = config.get_debug_trace();
        if !*trace_cfg.get_enabled() || *trace_cfg.get_level() == DebugTraceLevel::Disabled {
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

        fs::create_dir_all(&run_dir)
            .with_context(|| format!("failed to create trace directory: {}", run_dir.display()))?;

        let index = RunTraceIndex {
            schema_version: TRACE_SCHEMA_VERSION,
            run_name,
            instance_name,
            created_at_unix_ms: now_unix_ms(),
        };

        write_json_pretty_atomic(&run_dir.join("run.json"), &index)?;

        let (steps_dir, light_summary) = match trace_cfg.get_level() {
            DebugTraceLevel::Full => {
                let steps_dir = run_dir.join("steps");
                fs::create_dir_all(&steps_dir).with_context(|| {
                    format!("failed to create trace directory: {}", steps_dir.display())
                })?;
                (Some(steps_dir), None)
            }
            DebugTraceLevel::Light => {
                let summary = LightTraceSummary {
                    schema_version: TRACE_SCHEMA_VERSION,
                    run_name: index.run_name,
                    instance_name: index.instance_name,
                    created_at_unix_ms: index.created_at_unix_ms,
                    dimension: ivass.dimension(),
                    state_count: ivass.state_count(),
                    transition_count: ivass.transition_count(),
                    initial_valuation: ivass.initial_valuation.iter().copied().collect(),
                    final_valuation: ivass.final_valuation.iter().copied().collect(),
                    initial_graph_dot: ivass.to_graphviz(None, None),
                    result: None,
                };
                write_json_pretty_atomic(&run_dir.join("summary.json"), &summary)?;
                (None, Some(summary))
            }
            DebugTraceLevel::Disabled => unreachable!("disabled traces return before setup"),
        };

        Ok(Some(Self {
            run_dir,
            steps_dir,
            light_summary,
        }))
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
        let Some(steps_dir) = &self.steps_dir else {
            return Ok(());
        };
        let file_name = format!("step_{step:06}.json");
        let step_path = steps_dir.join(file_name);

        let payload = StepTraceSeed {
            schema_version: TRACE_SCHEMA_VERSION,
            step,
            initial_valuation: Some(initial_valuation.clone()),
            found_path: FoundPathSeed {
                n_reaching,
                path: path_to_seed(path),
            },
            scc_dag: scc_dag_to_seed(dag, product),
        };

        write_json_pretty_atomic(&step_path, &payload)
    }

    pub fn write_light_result(&mut self, result: &VASSReachSolverResult) -> anyhow::Result<()> {
        let Some(summary) = &mut self.light_summary else {
            return Ok(());
        };

        summary.result = Some(light_result(&result.status, &result.statistics));
        write_json_pretty_atomic(&self.run_dir.join("summary.json"), summary)
    }

    pub fn run_dir(&self) -> &FsPath {
        &self.run_dir
    }
}

fn light_result(
    status: &crate::solver::vass_reach::VASSReachSolverStatus,
    statistics: &VASSReachSolverStatistics,
) -> LightTraceResult {
    let (status, reason) = match status {
        crate::solver::SolverStatus::True(()) => ("reachable", None),
        crate::solver::SolverStatus::False(()) => ("unreachable", None),
        crate::solver::SolverStatus::Unknown(reason) => ("unknown", Some(format!("{reason:?}"))),
    };

    LightTraceResult {
        status: status.to_string(),
        reason,
        elapsed_ms: statistics.time.as_millis(),
        step_count: statistics.step_count,
        mu: statistics.mu.to_vec(),
        forwards_bound: statistics.forwards_bound.to_vec(),
        backwards_bound: statistics.backwards_bound.to_vec(),
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
                    nodes: component.nodes.clone(),
                    accepting_nodes: component.accepting_nodes.clone(),
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
                source: source.clone(),
                target: target.clone(),
                transition: *transition,
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
