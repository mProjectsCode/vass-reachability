use std::{fmt::Display, str::FromStr};

use clap::Parser;
use vass_reach_lib::{
    automaton::petri_net::initialized::InitializedPetriNet,
    config::{GeneralConfig, LoggerConfig, VASSReachConfig, VASSZReachConfig},
    logger::Logger,
    solver::{
        SerializableSolverResult, vass_reach::VASSReachSolver, vass_z_reach::VASSZReachSolver,
    },
};

/// The mode to run this tool in, either solve for reachability in N (natural
/// numbers) or Z (whole numbers).
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

#[derive(Debug, Clone)]
pub enum ModeWithConfig {
    N(VASSReachConfig),
    Z(VASSZReachConfig),
}

impl ModeWithConfig {
    pub fn from_file(mode: Mode, config: Option<String>) -> anyhow::Result<ModeWithConfig> {
        Ok(match mode {
            Mode::N => Self::N(VASSReachConfig::from_optional_file(config)?),
            Mode::Z => Self::Z(VASSZReachConfig::from_optional_file(config)?),
        })
    }

    pub fn logger_config(&self) -> &LoggerConfig {
        match self {
            ModeWithConfig::N(c) => c.logger(),
            ModeWithConfig::Z(c) => c.logger(),
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
    config: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let config = ModeWithConfig::from_file(args.mode, args.config)?;

    let petri_net = InitializedPetriNet::from_file(&args.file)?;
    let vass = petri_net.to_vass();

    let logger = Logger::from_config(config.logger_config(), "Solver".into());

    match config {
        ModeWithConfig::N(c) => {
            let res = VASSReachSolver::new(&vass, c, logger.as_ref()).solve();

            let json_res = serde_json::to_string_pretty(&SerializableSolverResult::from(res))?;
            println!("{}", json_res);
        }
        ModeWithConfig::Z(c) => {
            let res = VASSZReachSolver::new(
                &vass.to_cfg(),
                vass.initial_valuation.clone(),
                vass.final_valuation.clone(),
                c,
                logger.as_ref(),
            )
            .solve();

            let json_res = serde_json::to_string_pretty(&SerializableSolverResult::from(res))?;
            println!("{}", json_res);
        }
    }

    Ok(())
}
