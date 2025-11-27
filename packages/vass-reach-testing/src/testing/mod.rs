use std::{path, process::Command};

use anyhow::Context;
use hashbrown::HashMap;
use rayon::{
    ThreadPoolBuilder,
    iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator},
};
use serde::{Deserialize, Serialize};
use vass_reach_lib::{logger::Logger, solver::SerializableSolverResult};

use crate::{
    Args,
    config::{Test, TestRunConfig, load_tool_config},
    tools::{Tool, ToolWrapper, kreach::KReachTool, vass_reach::VASSReachTool},
};

pub fn test(logger: &Logger, args: &Args) -> anyhow::Result<()> {
    let Some(folder) = &args.folder else {
        anyhow::bail!("missing required folder argument");
    };
    let test = Test::canonicalize(folder)
        .with_context(|| format!("failed to canonicalize: {}", folder))?;
    let config = test
        .test_config()
        .with_context(|| format!("failed to read context file at: {}", test.path.display()))?;

    logger.info("Loading tool configuration...");

    let tool_config = load_tool_config().context("failed to load tool config")?;

    let tools: Vec<ToolWrapper> = vec![
        VASSReachTool::new(&tool_config, &config, test.path.clone()).into(),
        KReachTool::new(&tool_config, &config).into(),
    ];

    logger.info("Resetting systemd scopes...");

    for tool_config in &config.runs {
        let Some(tool) = tools.iter().find(|tool| tool.name() == &tool_config.tool) else {
            continue;
        };

        Command::new("systemctl")
            .args(&["--user", "reset-failed"])
            .status()
            .context("failed to reset systemd runs via systemctl")?;

        logger.info(&format!("Building tool: {}", tool.name()));

        tool.build()
            .with_context(|| format!("failed to build tool: {}", tool.name()))?;

        logger.info(&format!("Testing tool: {}", tool.name()));

        tool.test()
            .with_context(|| format!("failed to run test for tool: {}", tool.name()))?;

        logger.info(&format!("Running tool: {}", tool.name()));

        let results = run_tool_on_folder(logger, &test.instances_folder(), tool, tool_config)?;

        test.write_results(tool, results, tool_config)
            .with_context(|| {
                format!(
                    "failed to run instances in folder for tool: {}",
                    tool.name()
                )
            })?;

        logger.info(&format!(
            "Persisted test results to folder: {}",
            &test.results_folder().display()
        ));
    }

    Ok(())
}

fn run_tool_on_folder<T: Tool + Send + Sync>(
    logger: &Logger,
    folder: &path::Path,
    tool: &T,
    config: &TestRunConfig,
) -> anyhow::Result<HashMap<String, SolverResultStatistic>> {
    let files = std::fs::read_dir(folder)
        .with_context(|| format!("failed to read dir: {}", folder.display()))?;
    let files = files
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("failed to read dir: {}", folder.display()))?;

    let thread_pol = ThreadPoolBuilder::new()
        .num_threads(config.max_parallel as usize)
        .build()
        .expect("Failed to build thread pool");

    let results = thread_pol.install(|| {
        files
            .par_iter()
            .enumerate()
            .map(|(i, file)| {
                let result = if file.path().extension().and_then(|s| s.to_str()) == Some("spec") {
                    println!(
                        "Processing file {}/{}: {}",
                        i,
                        files.len(),
                        file.path().display()
                    );

                    let start_time = std::time::Instant::now();

                    let result = tool.run_on_file(&file.path(), config);

                    let duration = start_time.elapsed().as_millis();

                    match result {
                        Ok(result) => SolverResultStatistic::new(result, duration),
                        Err(e) => {
                            logger.warn(&format!(
                                "Tool {} crashed on file {}: {}",
                                tool.name(),
                                file.path().display(),
                                e
                            ));

                            SolverResultStatistic::new(
                                SolverRunResult::Crash(e.to_string()),
                                duration,
                            )
                        }
                    }
                } else {
                    SolverResultStatistic::new(
                        SolverRunResult::Crash("Not a .spec file".to_string()),
                        0,
                    )
                };

                let file_path = file.path().to_str().unwrap().to_string();

                (file_path, result)
            })
            .collect::<Vec<_>>()
    });

    Ok(results.into_iter().collect())
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
