use std::{fmt::Display, str::FromStr};

use chrono::Local;
use clap::Parser;
use vass_reach_lib::{
    automaton::petri_net::initialized::InitializedPetriNet,
    logger::{LogLevel, Logger},
    solver::{
        SerializableSolverResult, vass_reach::VASSReachSolverOptions,
        vass_z_reach::VASSZReachSolverOptions,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    N,
    Z,
}

impl FromStr for Mode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "n" => Ok(Mode::N),
            "z" => Ok(Mode::Z),
            _ => Err(format!("Invalid mode: {}", s)),
        }
    }
}

impl Display for Mode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mode::N => write!(f, "N"),
            Mode::Z => write!(f, "Z"),
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "VASS Reachability Tool")]
#[command(version = "0.1")]
#[command(about = "Solve reachability for VASS and Petri-Nets", long_about = None)]
struct Args {
    file: String,

    #[arg(short, long, default_value_t = Mode::N)]
    mode: Mode,

    #[arg(short, long)]
    timeout: Option<u64>,

    #[arg(short, long, default_value_t = LogLevel::Info)]
    log: LogLevel,

    #[arg(long, default_value_t = false)]
    log_file: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    let timeout = args.timeout.map(std::time::Duration::from_secs);

    let petri_net = InitializedPetriNet::from_file(&args.file)?;
    let vass = petri_net.to_vass();

    let log_file_path = if args.log_file {
        Some(format!(
            "logs/solver_run_{}.txt",
            Local::now().format("%Y-%m-%d_%H-%M-%S")
        ))
    } else {
        None
    };

    let logger = Logger::new(args.log, "Solver".to_owned(), log_file_path);

    match args.mode {
        Mode::N => {
            let res = VASSReachSolverOptions::default()
                .with_optional_time_limit(timeout)
                .with_logger(&logger)
                .to_vass_solver(&vass)
                .solve();

            let json_res = serde_json::to_string_pretty(&SerializableSolverResult::from(res))?;
            println!("{}", json_res);
        }
        Mode::Z => {
            let res = VASSZReachSolverOptions::default()
                .with_optional_time_limit(timeout)
                .with_logger(&logger)
                .to_vass_solver(vass)
                .solve();

            let json_res = serde_json::to_string_pretty(&SerializableSolverResult::from(res))?;
            println!("{}", json_res);
        }
    }

    Ok(())
}
