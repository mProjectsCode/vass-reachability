use std::{fmt::Display, fs, str::FromStr};

use anyhow::Context;
use clap::Parser;
use vass_reach_lib::logger::{LogLevel, Logger};

use crate::{generation::generate, testing::test, visualization::visualize};

pub mod config;
pub mod generation;
pub mod process_watcher;
pub mod random;
pub mod testing;
pub mod tools;
pub mod visualization;

#[derive(Parser, Debug)]
#[command(name = "VASS Reachability Solver Tester")]
#[command(version = "0.1")]
#[command(about = "Test different VASS reach solvers", long_about = None)]
pub struct Args {
    folder: Option<String>,

    #[arg(short, long, default_value_t = Mode::Test)]
    mode: Mode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    Test,
    Generate,
    Visualize,
}

impl FromStr for Mode {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "test" => Ok(Mode::Test),
            "generate" | "gen" => Ok(Mode::Generate),
            "visualize" | "vis" => Ok(Mode::Visualize),
            _ => Err(anyhow::anyhow!("Invalid mode: {}", s)),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::Test => write!(f, "Test"),
            Mode::Generate => write!(f, "Generate"),
            Mode::Visualize => write!(f, "Visualize"),
        }
    }
}

fn main() {
    let logger = Logger::new(LogLevel::Info, "tester".to_string(), None);
    let res = run(&logger);
    match &res {
        Ok(_) => logger.info("Tester completed successfully."),
        Err(e) => logger.error(&format!("Tester failed with error: {}", e)),
    }
}

fn run(logger: &Logger) -> anyhow::Result<()> {
    logger.info(&format!(
        "Running from: {}",
        fs::canonicalize(".").context("failed to canonicalize cwd")?.display()
    ));

    let args = Args::parse();

    match &args.mode {
        Mode::Generate => generate(logger, &args),
        Mode::Test => test(logger, &args),
        Mode::Visualize => visualize(logger, &args),
    }.with_context(|| format!("failed in mode: {}", &args.mode))
}

// #[derive(Debug)]
// pub struct ResultStatistics {
//     pub max_steps: u32,
//     pub min_steps: u32,
//     pub avg_steps: f64,
//     pub min_seconds: f64,
//     pub max_seconds: f64,
//     pub avg_seconds: f64,
// }

// impl ResultStatistics {
//     pub fn from_results(results: &[&SolverRunResult]) -> Self {
//         let mut steps = vec![];
//         let mut seconds = vec![];

//         for result in results {
//             if let SolverRunResult::Success(res) = result {
//                 steps.push(res.statistics.step_count);
//                 seconds.push(res.statistics.time.as_secs_f64());
//             }
//         }

//         let max_steps = *steps.iter().max().unwrap_or(&0);
//         let min_steps = *steps.iter().min().unwrap_or(&0);
//         let avg_steps = if !steps.is_empty() {
//             steps.iter().sum::<u32>() as f64 / steps.len() as f64
//         } else {
//             0.0
//         };

//         let max_seconds = *seconds
//             .iter()
//             .max_by(|a, b| a.partial_cmp(b).unwrap())
//             .unwrap_or(&0.0);
//         let min_seconds = *seconds
//             .iter()
//             .min_by(|a, b| a.partial_cmp(b).unwrap())
//             .unwrap_or(&0.0);
//         let avg_seconds = if !seconds.is_empty() {
//             seconds.iter().sum::<f64>() / seconds.len() as f64
//         } else {
//             0.0
//         };

//         ResultStatistics {
//             max_steps,
//             min_steps,
//             avg_steps,
//             min_seconds,
//             max_seconds,
//             avg_seconds,
//         }
//     }
// }
