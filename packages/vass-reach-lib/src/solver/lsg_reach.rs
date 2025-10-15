use std::sync::{Arc, atomic::AtomicBool};

use crate::{
    automaton::{AutomatonNode, lsg::LinearSubGraph, path::parikh_image::ParikhImage},
    logger::Logger,
    solver::{SolverResult, SolverStatus},
};

#[derive(Debug, Default)]
pub struct LSGReachSolverOptions<'l> {
    logger: Option<&'l Logger>,
    max_iterations: Option<u32>,
    max_time: Option<std::time::Duration>,
    stop_signal: Option<Arc<AtomicBool>>,
}

impl<'l> LSGReachSolverOptions<'l> {
    pub fn with_logger(mut self, logger: &'l Logger) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn with_iteration_limit(mut self, limit: u32) -> Self {
        self.max_iterations = Some(limit);
        self
    }

    pub fn with_time_limit(mut self, limit: std::time::Duration) -> Self {
        self.max_time = Some(limit);
        self
    }

    pub fn with_stop_signal(mut self, signal: Arc<AtomicBool>) -> Self {
        self.stop_signal = Some(signal);
        self
    }

    pub fn with_optional_time_limit(mut self, limit: Option<std::time::Duration>) -> Self {
        self.max_time = limit;
        self
    }

    pub fn with_optional_iteration_limit(mut self, limit: Option<u32>) -> Self {
        self.max_iterations = limit;
        self
    }

    pub fn to_solver<'g, N: AutomatonNode>(
        self,
        lsg: LinearSubGraph<'g, N>,
        initial_valuation: Box<[i32]>,
        final_valuation: Box<[i32]>,
    ) -> LSGReachSolver<'l, 'g, N> {
        LSGReachSolver::new(lsg, initial_valuation, final_valuation, self)
    }
}

#[derive(Debug, Clone)]
pub enum LSGSolutionPart {
    SubGraph(ParikhImage),
    Path(),
}

#[derive(Debug, Clone)]
pub struct LSGSolution {
    pub parts: Vec<LSGSolutionPart>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LSGReachSolverError {
    Timeout,
    MaxIterationsReached,
    SolverUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LSGReachSolverStatistics {
    pub step_count: u32,
    pub time: std::time::Duration,
}

impl LSGReachSolverStatistics {
    pub fn new(step_count: u32, time: std::time::Duration) -> Self {
        LSGReachSolverStatistics { step_count, time }
    }
}

pub type LSGReachSolverStatus = SolverStatus<LSGSolution, (), LSGReachSolverError>;

pub type LSGReachSolverResult =
    SolverResult<LSGSolution, (), LSGReachSolverError, LSGReachSolverStatistics>;

impl LSGReachSolverResult {
    pub fn get_solution(&self) -> Option<&LSGSolution> {
        match &self.status {
            SolverStatus::True(solution) => Some(solution),
            _ => None,
        }
    }
}

pub struct LSGReachSolver<'l, 'g, N: AutomatonNode> {
    lsg: LinearSubGraph<'g, N>,
    initial_valuation: Box<[i32]>,
    final_valuation: Box<[i32]>,
    options: LSGReachSolverOptions<'l>,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<'l, 'g, N: AutomatonNode> LSGReachSolver<'l, 'g, N> {
    pub fn new(
        lsg: LinearSubGraph<'g, N>,
        initial_valuation: Box<[i32]>,
        final_valuation: Box<[i32]>,
        options: LSGReachSolverOptions<'l>,
    ) -> Self {
        let stop_signal = options
            .stop_signal
            .clone()
            .unwrap_or(Arc::new(AtomicBool::new(false)));

        LSGReachSolver {
            lsg,
            initial_valuation,
            final_valuation,
            options,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
        }
    }

    pub fn solve(&mut self) -> LSGReachSolverResult {
        unimplemented!()
    }
}
