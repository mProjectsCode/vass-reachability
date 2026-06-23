use std::{
    sync::{Arc, atomic::AtomicBool},
    time::Duration,
};

use hashbrown::HashMap;
use petgraph::graph::EdgeIndex;

use super::LinearGraphReachSolver;
use crate::{
    automaton::{
        GIndex,
        cfg::update::CFGCounterUpdate,
        linear_graph::{LinearGraph, LinearGraphAutomaton, part::LinearGraphPart},
        path::{Path, parikh_image::ParikhImage},
        utils::cfg_updates_to_counter_update,
        vass::counter::VASSCounterValuation,
    },
    solver::{SolverResult, SolverStatus},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinearTemplateLowerBound {
    pub coefficients: Box<[i32]>,
    pub bound: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct LinearGraphBoundaryConstraints {
    pub lower_bounds: Vec<LinearTemplateLowerBound>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum LinearGraphBoundPoint<NIndex> {
    Boundary { index: usize, state: NIndex },
}

#[derive(Debug, Default)]
pub struct LinearGraphReachSolverOptions {
    pub(super) max_iterations: Option<u32>,
    pub(super) max_time: Option<Duration>,
    pub(super) stop_signal: Option<Arc<AtomicBool>>,
}

impl LinearGraphReachSolverOptions {
    pub fn with_iteration_limit(mut self, limit: u32) -> Self {
        self.max_iterations = Some(limit);
        self
    }

    pub fn with_time_limit(mut self, limit: Duration) -> Self {
        self.max_time = Some(limit);
        self
    }

    pub fn with_stop_signal(mut self, signal: Arc<AtomicBool>) -> Self {
        self.stop_signal = Some(signal);
        self
    }

    pub fn with_optional_time_limit(mut self, limit: Option<Duration>) -> Self {
        self.max_time = limit;
        self
    }

    pub fn with_optional_iteration_limit(mut self, limit: Option<u32>) -> Self {
        self.max_iterations = limit;
        self
    }

    pub fn into_solver<'g, NIndex: GIndex + Send + Sync, A>(
        self,
        linear_graph: &'g LinearGraph<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
    ) -> LinearGraphReachSolver<'g, NIndex, A>
    where
        A: LinearGraphAutomaton<NIndex> + Send + Sync,
    {
        LinearGraphReachSolver::new(linear_graph, initial_valuation, final_valuation, self)
    }

    pub(crate) fn into_solver_with_boundary_lower_bounds<'g, NIndex: GIndex + Send + Sync, A>(
        self,
        linear_graph: &'g LinearGraph<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
        boundary_lower_bounds: HashMap<
            LinearGraphBoundPoint<NIndex>,
            LinearGraphBoundaryConstraints,
        >,
    ) -> LinearGraphReachSolver<'g, NIndex, A>
    where
        A: LinearGraphAutomaton<NIndex> + Send + Sync,
    {
        LinearGraphReachSolver::new_with_boundary_lower_bounds(
            linear_graph,
            initial_valuation,
            final_valuation,
            self,
            boundary_lower_bounds,
        )
    }
}

#[derive(Debug, Clone)]
pub struct LinearGraphSolution {
    pub sub_graph_parikh_images: Vec<ParikhImage<EdgeIndex>>,
    pub repeat_path_counts: Vec<u32>,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
}

impl LinearGraphSolution {
    pub(crate) fn boundary_valuations<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &LinearGraph<'a, NIndex, A>,
    ) -> Vec<(NIndex, VASSCounterValuation)>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        let mut valuation = self.initial_valuation.clone();
        let mut boundaries = Vec::with_capacity(linear_graph.sequence.len() + 1);

        let Some(first) = linear_graph.sequence.first() else {
            return boundaries;
        };
        boundaries.push((first.start(linear_graph).clone(), valuation.clone()));

        for part in &linear_graph.sequence {
            match part {
                LinearGraphPart::Graph(idx) => {
                    valuation.apply_update(
                        &self.sub_graph_parikh_images[*idx].get_total_counter_effect(
                            linear_graph.graph(*idx),
                            linear_graph.dimension,
                        ),
                    );
                }
                LinearGraphPart::Path(idx) => {
                    valuation.apply_update(&cfg_updates_to_counter_update(
                        linear_graph.path(*idx).path.transitions.iter().cloned(),
                        linear_graph.dimension,
                    ));
                }
                LinearGraphPart::RepeatPath(idx) => {
                    let effect = cfg_updates_to_counter_update(
                        linear_graph
                            .repeat_path(*idx)
                            .path
                            .transitions
                            .iter()
                            .cloned(),
                        linear_graph.dimension,
                    );
                    for _ in 0..self.repeat_path_counts[*idx] {
                        valuation.apply_update(&effect);
                    }
                }
            }

            boundaries.push((part.end(linear_graph).clone(), valuation.clone()));
        }

        boundaries
    }

    pub fn build_run<'a, NIndex: GIndex, A>(
        &self,
        linear_graph: &LinearGraph<'a, NIndex, A>,
        n_run: bool,
    ) -> Option<Path<NIndex, CFGCounterUpdate>>
    where
        A: LinearGraphAutomaton<NIndex>,
    {
        let timer = std::time::Instant::now();
        let dimension = self.initial_valuation.dimension();
        let mut product_path =
            Path::<NIndex, CFGCounterUpdate>::new(linear_graph.automaton.get_initial());
        let mut current_valuation = self.initial_valuation.clone();

        for part in linear_graph.sequence.iter() {
            match part {
                LinearGraphPart::Graph(idx) => {
                    let graph = linear_graph.graph(*idx);
                    let image = &self.sub_graph_parikh_images[*idx];

                    let start_valuation = current_valuation.clone();
                    current_valuation
                        .apply_update(&image.get_total_counter_effect(graph, dimension));
                    let end_valuation = current_valuation.clone();
                    let sub_path =
                        image.build_run(graph, &start_valuation, &end_valuation, n_run)?;
                    let mapped_path = graph.map_path_to_product(&sub_path);

                    product_path.concat(mapped_path);
                }
                LinearGraphPart::Path(idx) => {
                    let path = linear_graph.path(*idx);
                    let update = cfg_updates_to_counter_update(
                        path.path.transitions.iter().cloned(),
                        dimension,
                    );

                    current_valuation.apply_update(&update);
                    product_path.concat(path.path.clone());
                }
                LinearGraphPart::RepeatPath(idx) => {
                    let repeated = linear_graph.repeat_path(*idx);
                    let count = self.repeat_path_counts[*idx];
                    let update = cfg_updates_to_counter_update(
                        repeated.path.transitions.iter().cloned(),
                        dimension,
                    );

                    for _ in 0..count {
                        current_valuation.apply_update(&update);
                        product_path.concat(repeated.path.clone());
                    }
                }
            }
        }

        assert_eq!(
            &current_valuation, &self.final_valuation,
            "Final valuation does not match the expected final valuation"
        );

        tracing::debug!(
            "Built run for linear graph solution in {} ms",
            timer.elapsed().as_millis()
        );

        Some(product_path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinearGraphReachSolverError {
    Timeout,
    MaxIterationsReached,
    SolverUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinearGraphReachSolverStatistics {
    pub step_count: u32,
    pub time: Duration,
}

impl LinearGraphReachSolverStatistics {
    pub fn new(step_count: u32, time: Duration) -> Self {
        LinearGraphReachSolverStatistics { step_count, time }
    }
}

pub type LinearGraphReachSolverStatus =
    SolverStatus<LinearGraphSolution, (), LinearGraphReachSolverError>;

pub type LinearGraphReachSolverResult = SolverResult<
    LinearGraphSolution,
    (),
    LinearGraphReachSolverError,
    LinearGraphReachSolverStatistics,
>;

impl LinearGraphReachSolverResult {
    pub fn get_solution(&self) -> Option<&LinearGraphSolution> {
        match &self.status {
            SolverStatus::True(solution) => Some(solution),
            _ => None,
        }
    }
}
