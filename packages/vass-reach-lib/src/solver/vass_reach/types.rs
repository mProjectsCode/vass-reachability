use serde::{Deserialize, Serialize};

use crate::{
    automaton::vass::counter::VASSCounterIndex,
    solver::{SolverResult, SolverStatus},
};

/// Enum representing the different refinement actions that the algorithm can
/// do.
pub enum VASSReachRefinementAction {
    /// Increase the modulo for the given counter, depending on strategy, so
    /// that the given value does no longer equal the final valuation modulo mu.
    IncreaseModulo(VASSCounterIndex, i32),
    /// Increase the forward counting bound for the given counter to the given
    /// value.
    IncreaseForwardsBound(VASSCounterIndex, u32),
    /// Increase the backward counting bound for the given counter to the given
    /// value.
    IncreaseBackwardsBound(VASSCounterIndex, u32),
    /// Build some automaton (LTC, LinearGraph, ...?) to cut away the spurious
    /// path.
    BuildAutomaton,
}

/// The different errors that can occur during VASS reachability solving.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VASSReachSolverError {
    /// We ran out of time.
    Timeout,
    /// We hit the maximum number of iterations.
    MaxIterationsReached,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VASSReachSolverStatistics {
    pub step_count: u64,
    pub mu: Box<[i32]>,
    pub forwards_bound: Box<[u32]>,
    pub backwards_bound: Box<[u32]>,
    pub time: std::time::Duration,
}

impl VASSReachSolverStatistics {
    pub fn new(
        step_count: u64,
        mu: Box<[i32]>,
        forwards_bound: Box<[u32]>,
        backwards_bound: Box<[u32]>,
        time: std::time::Duration,
    ) -> Self {
        VASSReachSolverStatistics {
            step_count,
            mu,
            forwards_bound,
            backwards_bound,
            time,
        }
    }
}

pub type VASSReachSolverStatus = SolverStatus<(), (), VASSReachSolverError>;

pub type VASSReachSolverResult =
    SolverResult<(), (), VASSReachSolverError, VASSReachSolverStatistics>;
