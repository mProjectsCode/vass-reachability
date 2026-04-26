use std::{path, process::Command};

use anyhow::Context;
use hashbrown::HashMap;
use rayon::{
    ThreadPoolBuilder,
    iter::{IntoParallelRefIterator, ParallelIterator},
};
use serde::{Deserialize, Serialize};
use vass_reach_lib::solver::SerializableSolverResult;

use crate::{
    Args,
    config::{Test, TestRunConfig, load_tool_config},
    tools::{Tool, ToolWrapper, kreach::KReachTool, vass_reach::VASSReachTool},
};

pub fn test(args: &Args) -> anyhow::Result<()> {
    let Some(folder) = &args.folder else {
        anyhow::bail!("missing required folder argument");
    };
    let test = Test::canonicalize(folder)
        .with_context(|| format!("failed to canonicalize: {}", folder))?;
    let config = test
        .test_config()
        .with_context(|| format!("failed to read context file at: {}", test.path.display()))?;

    tracing::info!("Loading tool configuration...");

    let tool_config = load_tool_config().context("failed to load tool config")?;

    let tools: Vec<ToolWrapper> = vec![
        VASSReachTool::new(&tool_config, &config, test.path.clone()).into(),
        KReachTool::new(&tool_config, &config).into(),
    ];

    tracing::info!("Resetting systemd scopes...");

    for tool_config in &config.runs {
        let Some(tool) = tools.iter().find(|tool| tool.name() == tool_config.tool) else {
            continue;
        };

        Command::new("systemctl")
            .args(["--user", "reset-failed"])
            .status()
            .context("failed to reset systemd runs via systemctl")?;

        tracing::info!("Building tool: {}", tool.name());

        tool.build()
            .with_context(|| format!("failed to build tool: {}", tool.name()))?;

        tracing::info!("Testing tool: {}", tool.name());

        tool.test()
            .with_context(|| format!("failed to run test for tool: {}", tool.name()))?;

        tracing::info!("Running tool: {}", tool.name());

        let instance_files = resolve_instance_files(&test, tool)?;
        let results = run_tool_on_folder(&instance_files, tool, tool_config)?;

        test.write_results(tool, results, tool_config)
            .with_context(|| {
                format!(
                    "failed to run instances in folder for tool: {}",
                    tool.name()
                )
            })?;

        tracing::info!(
            "Persisted test results to folder: {}",
            &test.results_folder().display()
        );
    }

    Ok(())
}

fn run_tool_on_folder<T: Tool + Send + Sync>(
    files: &[path::PathBuf],
    tool: &T,
    config: &TestRunConfig,
) -> anyhow::Result<HashMap<String, SolverResultStatistic>> {
    let thread_pol = ThreadPoolBuilder::new()
        .num_threads(config.max_parallel as usize)
        .build()
        .expect("Failed to build thread pool");

    let counter = std::sync::atomic::AtomicUsize::new(1);

    let results = thread_pol.install(|| {
        files
            .par_iter()
            .map(|file| {
                let i = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let result = if tool.supports_instance_file(file) {
                    println!("Processing file {}/{}: {}", i, files.len(), file.display());

                    let start_time = std::time::Instant::now();

                    let result = tool.run_on_file(file, config);

                    let duration = start_time.elapsed().as_millis();

                    match result {
                        Ok(result) => SolverResultStatistic::new(result, duration),
                        Err(e) => {
                            tracing::warn!(
                                "Tool {} crashed on file {}: {}",
                                tool.name(),
                                file.display(),
                                e
                            );

                            SolverResultStatistic::new(
                                SolverRunResult::Crash(e.to_string()),
                                duration,
                            )
                        }
                    }
                } else {
                    SolverResultStatistic::new(
                        SolverRunResult::Crash("Unsupported instance file for tool".to_string()),
                        0,
                    )
                };

                let file_path = file.to_str().unwrap().to_string();

                (file_path, result)
            })
            .collect::<Vec<_>>()
    });

    Ok(results.into_iter().collect())
}

fn resolve_instance_files(test: &Test, tool: &impl Tool) -> anyhow::Result<Vec<path::PathBuf>> {
    let instance_config = test.instance_config()?;
    let instances_folder = test.instances_folder();

    let mut files = if !instance_config.hand_picked_instances.is_empty() {
        instance_config
            .hand_picked_instances
            .iter()
            .map(|relative| instances_folder.join(relative))
            .collect::<Vec<_>>()
    } else {
        std::fs::read_dir(&instances_folder)
            .with_context(|| format!("failed to read dir: {}", instances_folder.display()))?
            .filter_map(|entry| entry.ok().map(|entry| entry.path()))
            .collect::<Vec<_>>()
    };

    files.retain(|path| path.is_file() && tool.supports_instance_file(path));
    files.sort();

    if files.is_empty() {
        anyhow::bail!(
            "no supported instance files found for tool {} in {}",
            tool.name(),
            instances_folder.display()
        );
    }

    Ok(files)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SolverRunResult {
    Success(SerializableSolverResult<()>),
    Crash(String),
    OOM,
    Timeout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverResultStatistic {
    pub result: SolverRunResult,
    pub ms_taken: u128,
}

impl SolverResultStatistic {
    pub fn new(result: SolverRunResult, ms_taken: u128) -> Self {
        SolverResultStatistic { result, ms_taken }
    }
}
