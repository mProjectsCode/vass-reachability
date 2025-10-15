use std::{fs::canonicalize, path, process::Command};

use vass_reach_lib::solver::{
    SerializableSolverResult, SerializableSolverStatus, SolverStatus,
    vass_reach::VASSReachSolverStatistics,
};

use crate::random::{
    RandomOptions, persist_multiple_to_file, petri_net::generate_random_petri_net,
    vass::generate_radom_vass,
};

pub mod random;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    build_vass_reach()?;
    run_vass_reach_solver()?;

    let random_vass = generate_random_petri_net(RandomOptions::new(1, 1000), 3, 3, 3);
    let folder = canonicalize("./test_data")?;
    let folder = folder.join("petri_nets_3_3_3");

    persist_multiple_to_file(&random_vass, folder.as_path(), "net")?;

    let results = run_vass_solver_on_folder(folder.as_path())?;

    let successful_runs = results
        .iter()
        .filter(|r| matches!(r, SolverRunResult::Success(_)))
        .count();
    let crashes = results
        .iter()
        .filter(|r| matches!(r, SolverRunResult::Crash(_)))
        .count();
    let timeouts = results
        .iter()
        .filter(|r| matches!(r, SolverRunResult::Timeout))
        .count();

    println!("Successful runs: {}", successful_runs);
    println!("Timeout Kills: {}", timeouts);
    println!("Crashes: {}", crashes);
    println!();

    let reachable_runs = results.iter().filter(|r| matches!(r, SolverRunResult::Success(res) if matches!(res.status, SerializableSolverStatus::True))).collect::<Vec<_>>();
    let unreachable_runs = results.iter().filter(|r| matches!(r, SolverRunResult::Success(res) if matches!(res.status, SerializableSolverStatus::False))).collect::<Vec<_>>();
    let unknown_runs = results.iter().filter(|r| matches!(r, SolverRunResult::Success(res) if matches!(res.status, SerializableSolverStatus::Unknown))).collect::<Vec<_>>();

    println!("Reachable: {}", reachable_runs.len());
    println!(
        "Stats: {:#?}",
        ResultStatistics::from_results(&reachable_runs)
    );
    println!("Unreachable: {}", unreachable_runs.len());
    println!(
        "Stats: {:#?}",
        ResultStatistics::from_results(&unreachable_runs)
    );
    println!("Unknown: {}", unknown_runs.len());
    println!(
        "Stats: {:#?}",
        ResultStatistics::from_results(&unknown_runs)
    );

    Ok(())
}

fn build_vass_reach() -> Result<(), Box<dyn std::error::Error>> {
    println!("Building VASS Reachability Solver...");

    println!("Current dir: {}", canonicalize(".")?.display());
    println!(
        "VASS Reachability dir: {}",
        canonicalize("../vass-reach")?.display()
    );

    Command::new("cargo")
        .args(&["build", "--release"])
        .current_dir(canonicalize("../vass-reach")?)
        .status()?;

    println!("Successfully built VASS Reachability Solver.");

    Ok(())
}

fn run_vass_reach_solver() -> Result<(), Box<dyn std::error::Error>> {
    println!("Running VASS Reachability Solver...");

    let status = Command::new("../vass-reach/target/release/vass-reach")
        .args(&["--help"])
        .current_dir(canonicalize(".")?)
        .status()?;

    if status.success() {
        println!("VASS Reachability Solver ran successfully.");
    } else {
        println!("VASS Reachability Solver failed to run.");
    }

    Ok(())
}

fn run_vass_solver_on_folder(
    folder: &path::Path,
) -> Result<Vec<SolverRunResult>, Box<dyn std::error::Error>> {
    println!(
        "Running VASS Reachability Solver on folder: {}",
        folder.display()
    );

    let files = std::fs::read_dir(folder)?;
    Ok(files
        .map(|file| {
            let file = file?;
            if file.path().extension().and_then(|s| s.to_str()) == Some("json") {
                println!("Processing file: {}", file.path().display());
                let output = Command::new("../vass-reach/target/release/vass-reach")
                    .args(&["-t=30", file.path().to_str().unwrap()])
                    .current_dir(canonicalize(".")?)
                    .output()?;

                if output.status.success() {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    let res: SerializableSolverResult<VASSReachSolverStatistics> =
                        serde_json::from_str(&stdout)?;
                    Ok(SolverRunResult::Success(res))
                } else {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    Ok(SolverRunResult::Crash(stderr.to_string()))
                }
            } else {
                Ok(SolverRunResult::Crash("Not a JSON file".to_string()))
            }
        })
        .map(
            |r: Result<SolverRunResult, Box<dyn std::error::Error>>| match r {
                Ok(res) => res,
                Err(e) => SolverRunResult::Crash(e.to_string()),
            },
        )
        .collect())
}

#[derive(Debug)]
pub enum SolverRunResult {
    Success(SerializableSolverResult<VASSReachSolverStatistics>),
    Crash(String),
    Timeout,
}

#[derive(Debug)]
pub struct ResultStatistics {
    pub max_steps: u32,
    pub min_steps: u32,
    pub avg_steps: f64,
    pub min_seconds: f64,
    pub max_seconds: f64,
    pub avg_seconds: f64,
}

impl ResultStatistics {
    pub fn from_results(results: &[&SolverRunResult]) -> Self {
        let mut steps = vec![];
        let mut seconds = vec![];

        for result in results {
            if let SolverRunResult::Success(res) = result {
                steps.push(res.statistics.step_count);
                seconds.push(res.statistics.time.as_secs_f64());
            }
        }

        let max_steps = *steps.iter().max().unwrap_or(&0);
        let min_steps = *steps.iter().min().unwrap_or(&0);
        let avg_steps = if !steps.is_empty() {
            steps.iter().sum::<u32>() as f64 / steps.len() as f64
        } else {
            0.0
        };

        let max_seconds = *seconds
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);
        let min_seconds = *seconds
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap_or(&0.0);
        let avg_seconds = if !seconds.is_empty() {
            seconds.iter().sum::<f64>() / seconds.len() as f64
        } else {
            0.0
        };

        ResultStatistics {
            max_steps,
            min_steps,
            avg_steps,
            min_seconds,
            max_seconds,
            avg_seconds,
        }
    }
}
