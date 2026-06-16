use std::{collections::BTreeSet, fs, path::Path, time::Duration};

use rand::{RngExt, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use vass_reach_lib::{
    automaton::{algorithms::EdgeAutomatonAlgorithms, vass::initialized::InitializedVASS},
    config::{PreprocessingConfig, VASSReachConfig},
    solver::{
        SolverStatus,
        vass_reach::{VASSReachSolver, VASSReachSolverError, VASSReachSolverStatistics},
    },
    utils::write_json_pretty_atomic,
};

use crate::{
    Args,
    config::{InstanceConfig, Test},
    random::{RandomOptions, vass::generate_random_vass_in_ranges},
};

const SEARCH_RUN_NAME: &str = "hard-instance-search";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardCandidateSummary {
    pub schema_version: u32,
    pub run_name: String,
    pub instance_name: String,
    pub seed: String,
    pub result_reason: String,
    pub repetitions: usize,
    pub elapsed_ms: Vec<u128>,
    pub step_counts: Vec<u64>,
    pub dimension: usize,
    pub state_count: usize,
    pub transition_count: usize,
    pub initial_valuation: Vec<i32>,
    pub final_valuation: Vec<i32>,
    pub max_update_magnitude: i32,
    pub initial_graph_dot: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SearchManifest {
    seed: u64,
    generated: usize,
    retained: Vec<HardCandidateSummary>,
}

#[derive(Debug)]
struct HardnessObservation {
    reason: VASSReachSolverError,
    statistics: VASSReachSolverStatistics,
}

pub fn search(args: &Args) -> anyhow::Result<()> {
    let test = test_from_args(args)?;
    let config = test.instance_config()?;
    validate_search_config(&config)?;
    if config.num_instances == 0 {
        anyhow::bail!("num_instances must be greater than zero");
    }

    let instances_dir = test.instances_folder();
    let summaries_dir = test
        .path
        .join("light-traces")
        .join("vass-reach")
        .join(SEARCH_RUN_NAME);
    fs::create_dir_all(&instances_dir)?;
    fs::create_dir_all(&summaries_dir)?;

    let mut seed_rng = StdRng::seed_from_u64(config.seed);
    let mut retained = Vec::new();
    for index in 0..config.num_instances {
        let seed = seed_rng.random::<u64>();
        let mut generated = generate_random_vass_in_ranges(
            RandomOptions::new(seed, 1),
            config.vass_counters,
            config.vass_states,
            config.vass_transitions,
            config.vass_updates,
            config.vass_valuations,
        )?;
        let instance = generated.pop().expect("requested one generated VASS");
        let Some(observations) = repeat_hardness(&instance, &config) else {
            continue;
        };

        let name = format!("hard_{index:06}_seed_{seed}");
        let instance_path = instances_dir.join(format!("{name}.vass.json"));
        instance.to_json_file(path_str(&instance_path)?)?;
        let summary = candidate_summary(&instance, name.clone(), seed, &observations);
        let summary_dir = summaries_dir.join(&name);
        fs::create_dir_all(&summary_dir)?;
        write_json_pretty_atomic(&summary_dir.join("summary.json"), &summary)?;
        retained.push(summary);
    }

    retained.sort_by_key(|candidate| {
        (
            candidate.dimension,
            candidate.transition_count,
            candidate.state_count,
            candidate.max_update_magnitude,
        )
    });
    write_json_pretty_atomic(
        &test.path.join("search-results.json"),
        &SearchManifest {
            seed: config.seed,
            generated: config.num_instances,
            retained,
        },
    )?;
    Ok(())
}

fn test_from_args(args: &Args) -> anyhow::Result<Test> {
    let folder = args
        .folder
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("missing required folder argument"))?;
    Test::canonicalize(folder)
}

fn validate_search_config(config: &InstanceConfig) -> anyhow::Result<()> {
    if config.search_repetitions == 0 {
        anyhow::bail!("search_repetitions must be greater than zero");
    }
    Ok(())
}

fn solver_config(config: &InstanceConfig) -> VASSReachConfig {
    VASSReachConfig::default()
        .with_timeout(Some(Duration::from_millis(config.search_timeout_ms)))
        .with_max_iterations(Some(config.search_max_iterations))
        .with_bounded_counting_enabled(false)
        .with_preprocessing(PreprocessingConfig::default().with_enabled(false))
}

fn observe_hardness(
    instance: &InitializedVASS<usize, usize>,
    config: &InstanceConfig,
) -> Option<HardnessObservation> {
    let result = VASSReachSolver::new(instance, solver_config(config)).solve();
    match result.status {
        SolverStatus::Unknown(reason) => Some(HardnessObservation {
            reason,
            statistics: result.statistics,
        }),
        SolverStatus::True(()) | SolverStatus::False(()) => None,
    }
}

fn repeat_hardness(
    instance: &InitializedVASS<usize, usize>,
    config: &InstanceConfig,
) -> Option<Vec<HardnessObservation>> {
    let mut observations = Vec::with_capacity(config.search_repetitions);
    for _ in 0..config.search_repetitions {
        observations.push(observe_hardness(instance, config)?);
    }
    Some(observations)
}

fn candidate_summary(
    instance: &InitializedVASS<usize, usize>,
    name: String,
    seed: u64,
    observations: &[HardnessObservation],
) -> HardCandidateSummary {
    let max_update_magnitude = instance
        .vass
        .graph
        .edge_weights()
        .flat_map(|edge| edge.update.iter())
        .map(|value| value.abs())
        .max()
        .unwrap_or(0);
    let reasons = observations
        .iter()
        .map(|observation| format!("{:?}", observation.reason))
        .collect::<BTreeSet<_>>();
    HardCandidateSummary {
        schema_version: 1,
        run_name: SEARCH_RUN_NAME.to_string(),
        instance_name: name,
        seed: seed.to_string(),
        result_reason: reasons.into_iter().collect::<Vec<_>>().join(", "),
        repetitions: observations.len(),
        elapsed_ms: observations
            .iter()
            .map(|observation| observation.statistics.time.as_millis())
            .collect(),
        step_counts: observations
            .iter()
            .map(|observation| observation.statistics.step_count)
            .collect(),
        dimension: instance.dimension(),
        state_count: instance.state_count(),
        transition_count: instance.transition_count(),
        initial_valuation: instance.initial_valuation.iter().copied().collect(),
        final_valuation: instance.final_valuation.iter().copied().collect(),
        max_update_magnitude,
        initial_graph_dot: instance.to_graphviz(None, None),
    }
}

fn path_str(path: &Path) -> anyhow::Result<&str> {
    path.to_str()
        .ok_or_else(|| anyhow::anyhow!("path is not valid UTF-8: {}", path.display()))
}
