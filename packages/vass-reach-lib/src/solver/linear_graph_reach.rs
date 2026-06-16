use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use hashbrown::HashMap;
use petgraph::graph::EdgeIndex;
use z3::{Config, Context, Solver, ast::Int, with_z3_config};

use crate::{
    automaton::{
        Alphabet, AutomatonIterators, Deterministic, ExplicitEdgeAutomaton, GIndex,
        InitializedAutomaton, TransitionSystem,
        cfg::update::CFGCounterUpdate,
        index_map::OptionIndexMap,
        linear_graph::{
            LinearGraph,
            part::{
                LinearGraphPart, LinearGraphPathSegment, LinearGraphRegion, LinearGraphRepeatPath,
            },
        },
        path::{Path, parikh_image::ParikhImage},
        scc::SCCAlgorithms,
        utils::{cfg_updates_to_counter_update, cfg_updates_to_counter_updates},
        vass::counter::VASSCounterValuation,
    },
    solver::{
        SolverResult, SolverStatus,
        utils::{forbid_parikh_image, parikh_image_from_edge_map},
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LinearTemplateLowerBound {
    pub coefficients: Box<[i32]>,
    pub bound: i32,
}

#[derive(Debug, Default)]
pub struct LinearGraphReachSolverOptions {
    max_iterations: Option<u32>,
    max_time: Option<Duration>,
    stop_signal: Option<Arc<AtomicBool>>,
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
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>
            + Send
            + Sync,
    {
        LinearGraphReachSolver::new(linear_graph, initial_valuation, final_valuation, self)
    }

    pub(crate) fn into_solver_with_boundary_lower_bounds<'g, NIndex: GIndex + Send + Sync, A>(
        self,
        linear_graph: &'g LinearGraph<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
        boundary_lower_bounds: HashMap<NIndex, Vec<LinearTemplateLowerBound>>,
    ) -> LinearGraphReachSolver<'g, NIndex, A>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>
            + Send
            + Sync,
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
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
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
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>,
    {
        // the parikh image already determines the initial and final valuations when
        // entering and leaving graphs so we can simply build the runs for
        // each part independently and concatenate them

        let timer = std::time::Instant::now();

        let dimension = self.initial_valuation.dimension();

        let mut product_path =
            Path::<NIndex, CFGCounterUpdate>::new(linear_graph.automaton.get_initial());

        let mut current_valuation = self.initial_valuation.clone();

        // TODO: this does not work at all with inconsistent LGSs. If we have an unused
        // path or graph we are in trouble. First we do more solving and then we also
        // run into index errors here.

        for part in linear_graph.sequence.iter() {
            match part {
                LinearGraphPart::Graph(idx) => {
                    let graph = linear_graph.graph(*idx);
                    let image = &self.sub_graph_parikh_images[*idx];

                    // first we need to calculate the start and end valuations for the graph
                    let start_valuation = current_valuation.clone();
                    current_valuation
                        .apply_update(&image.get_total_counter_effect(graph, dimension));
                    let end_valuation = current_valuation.clone();

                    // then we can build the run for the graph
                    let sub_path =
                        image.build_run(graph, &start_valuation, &end_valuation, n_run)?;

                    // now the node and edge indices in the path are for the graph, so we
                    // need to map them back to the cfg
                    let mapped_path = graph.map_path_to_product(&sub_path);

                    product_path.concat(mapped_path);
                }
                LinearGraphPart::Path(idx) => {
                    let path = linear_graph.path(*idx);

                    // we need to update the current valuation for possible following graphs
                    let update = cfg_updates_to_counter_update(
                        path.path.transitions.iter().cloned(),
                        dimension,
                    );

                    current_valuation.apply_update(&update);

                    // then we can simply add the edges to the path
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

pub struct LinearGraphReachSolver<'g, NIndex: GIndex + Send + Sync, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>
        + Send
        + Sync,
{
    linear_graph: &'g LinearGraph<'g, NIndex, A>,
    initial_valuation: &'g VASSCounterValuation,
    final_valuation: &'g VASSCounterValuation,
    options: LinearGraphReachSolverOptions,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
    boundary_lower_bounds: HashMap<NIndex, Vec<LinearTemplateLowerBound>>,
}

impl<'g, NIndex: GIndex + Send + Sync, A> LinearGraphReachSolver<'g, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>
        + Send
        + Sync,
{
    pub fn new(
        linear_graph: &'g LinearGraph<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
        options: LinearGraphReachSolverOptions,
    ) -> Self {
        Self::new_with_boundary_lower_bounds(
            linear_graph,
            initial_valuation,
            final_valuation,
            options,
            HashMap::new(),
        )
    }

    pub(crate) fn new_with_boundary_lower_bounds(
        linear_graph: &'g LinearGraph<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
        options: LinearGraphReachSolverOptions,
        boundary_lower_bounds: HashMap<NIndex, Vec<LinearTemplateLowerBound>>,
    ) -> Self {
        let stop_signal = options
            .stop_signal
            .clone()
            .unwrap_or(Arc::new(AtomicBool::new(false)));

        linear_graph.assert_consistent();

        LinearGraphReachSolver {
            linear_graph,
            initial_valuation,
            final_valuation,
            options,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
            boundary_lower_bounds,
        }
    }

    pub fn solve(&mut self) -> LinearGraphReachSolverResult {
        self.solver_start_time = Some(std::time::Instant::now());

        let mut config = Config::new();
        config.set_model_generation(true);

        with_z3_config(&config, || {
            let solver = Solver::new();

            let context = Context::thread_local();
            let context_handle = context.handle();

            let start_time = self.solver_start_time.unwrap();
            let stop_signal = self.stop_signal.clone();
            let max_time = self.options.max_time;

            let mut result = None;

            thread::scope(|s| {
                s.spawn(|| {
                    loop {
                        std::thread::sleep(Duration::from_millis(10));

                        if let Some(max_time) = max_time
                            && start_time.elapsed() >= max_time
                        {
                            stop_signal.store(true, Ordering::SeqCst);
                        }

                        if stop_signal.load(Ordering::SeqCst) {
                            context_handle.interrupt();
                            break;
                        }
                    }
                });

                result = Some(self.solve_inner(&solver));

                stop_signal.store(true, Ordering::SeqCst);
            });

            tracing::debug!(
                "Linear graph reachability solver finished in {} ms",
                self.get_solver_time().unwrap_or_default().as_millis()
            );

            result.expect("Thread panicked")
        })
    }

    fn solve_inner(&mut self, solver: &Solver) -> LinearGraphReachSolverResult {
        let mut sums: Box<[_]> = self
            .initial_valuation
            .iter()
            .map(|x| Int::from_i64(*x as i64))
            .collect();

        let mut edge_maps = Vec::new();
        let mut repeat_vars = vec![None; self.linear_graph.repeat_paths.len()];

        for (i, part) in self.linear_graph.sequence.iter().enumerate() {
            self.build_boundary_lower_bound_constraints(
                part.start(self.linear_graph),
                solver,
                &sums,
            );

            match part {
                LinearGraphPart::Path(idx) => {
                    self.build_path_constraints(self.linear_graph.path(*idx), solver, &mut sums);
                }
                LinearGraphPart::Graph(idx) => {
                    let edge_map = self.build_graph_constraints(
                        i,
                        self.linear_graph.graph(*idx),
                        solver,
                        &mut sums,
                    );
                    edge_maps.push(edge_map);
                }
                LinearGraphPart::RepeatPath(idx) => {
                    let count = self.build_repeat_path_constraints(
                        i,
                        self.linear_graph.repeat_path(*idx),
                        solver,
                        &mut sums,
                    );
                    repeat_vars[*idx] = Some(count);
                }
            }
        }

        if let Some(last) = self.linear_graph.sequence.last() {
            self.build_boundary_lower_bound_constraints(last.end(self.linear_graph), solver, &sums);
        }

        for (sum, target) in sums.iter().zip(self.final_valuation.iter()) {
            solver.assert(sum.eq(Int::from_i64(*target as i64)));
        }

        self.step_count = 1;

        loop {
            match solver.check() {
                z3::SatResult::Sat => {
                    let model = solver.get_model().unwrap();

                    let parikh_image_components = edge_maps
                        .iter()
                        .zip(self.linear_graph.iter_graph_parts())
                        .map(|(map, graph)| {
                            let image = parikh_image_from_edge_map(map, &model);

                            let (main_component, components) =
                                image.split_into_connected_components(graph);

                            (graph, map, main_component, components)
                        })
                        .collect::<Vec<_>>();

                    if parikh_image_components
                        .iter()
                        .all(|(_, _, _, c)| c.is_empty())
                    {
                        return self.get_solver_result(LinearGraphReachSolverStatus::True(
                            LinearGraphSolution {
                                sub_graph_parikh_images: parikh_image_components
                                    .into_iter()
                                    .map(|(_, _, main_component, _)| main_component)
                                    .collect(),
                                repeat_path_counts: repeat_vars
                                    .iter()
                                    .map(|var| {
                                        let var = var
                                            .as_ref()
                                            .expect("every repeated path must have a variable");
                                        model
                                            .get_const_interp(var)
                                            .expect("repeat count must be in the model")
                                            .as_u64()
                                            .expect("repeat count must be a non-negative integer")
                                            as u32
                                    })
                                    .collect(),
                                initial_valuation: self.initial_valuation.clone(),
                                final_valuation: self.final_valuation.clone(),
                            },
                        ));
                    }

                    if self.max_iterations_reached() {
                        return self.max_iterations_reached_result();
                    }

                    if self.max_time_reached() {
                        return self.max_time_reached_result();
                    }

                    tracing::debug!(
                        "Restricting {} connected components",
                        parikh_image_components
                            .iter()
                            .map(|(_, _, _, c)| c.len())
                            .sum::<usize>()
                    );

                    for (graph, edge_map, _, components) in parikh_image_components.into_iter() {
                        for component in components {
                            forbid_parikh_image(&component, graph, edge_map, solver);
                        }
                    }

                    self.step_count += 1;
                }
                z3::SatResult::Unsat => {
                    return self.get_solver_result(LinearGraphReachSolverStatus::False(()));
                }
                z3::SatResult::Unknown => {
                    return self.get_solver_result(LinearGraphReachSolverStatus::Unknown(
                        LinearGraphReachSolverError::SolverUnknown,
                    ));
                }
            }
        }
    }

    fn build_path_constraints(
        &self,
        path: &LinearGraphPathSegment<NIndex>,
        solver: &Solver,
        sums: &mut Box<[Int]>,
    ) {
        let path_updates = cfg_updates_to_counter_updates(
            path.path.transitions.iter().cloned(),
            self.linear_graph.dimension,
        );

        // first subtract the minimums
        for (update, sum) in path_updates.0.iter().zip(sums.iter_mut()) {
            let update_ast = Int::from_i64(*update as i64);
            *sum = &*sum - &update_ast;
        }

        // then assert non-negativity
        for sum in sums.iter() {
            let zero = Int::from_i64(0);
            let geq_zero = sum.ge(&zero);
            solver.assert(&geq_zero);
        }

        // then add the rest to get the path's effect
        for (update, sum) in path_updates.1.iter().zip(sums.iter_mut()) {
            let update_ast = Int::from_i64(*update as i64);
            *sum = &*sum + &update_ast;
        }
    }

    fn build_graph_constraints(
        &self,
        part_index: usize,
        graph: &LinearGraphRegion<NIndex>,
        solver: &Solver,
        sums: &mut Box<[Int]>,
    ) -> OptionIndexMap<EdgeIndex, Int> {
        let mut edge_map = OptionIndexMap::new(graph.edge_count());

        for (edge, update) in graph.iter_edges() {
            // we need one variable for each edge
            let edge_var = Int::new_const(format!("graph_{}_edge_{}", part_index, edge.index()));
            // CONSTRAINT: an edge can only be taken positive times
            solver.assert(edge_var.ge(Int::from_i64(0)));

            // add the edges effect to the counter sum
            let i = update.counter();
            sums[i.to_usize()] = &sums[i.to_usize()] + &edge_var * update.op_i64();

            edge_map.insert(edge, edge_var);
        }

        for node in graph.iter_node_indices() {
            let outgoing = graph.outgoing_edge_indices(&node);
            let incoming = graph.incoming_edge_indices(&node);

            // the end node has one additional outgoing connection, this works, because we
            // always have exactly one end node
            let mut outgoing_sum = if node == graph.end {
                Int::from_i64(1)
            } else {
                Int::from_i64(0)
            };
            // the start node has one additional incoming connection
            let mut incoming_sum = if node == graph.start {
                Int::from_i64(1)
            } else {
                Int::from_i64(0)
            };

            for edge in outgoing {
                let edge_var = &edge_map[edge];
                outgoing_sum += edge_var;
            }

            for edge in incoming {
                let edge_var = &edge_map[edge];
                incoming_sum += edge_var;
            }

            // CONSTRAINT: the sum of all outgoing edges must be equal to the sum of all
            // incoming edges for each node
            solver.assert(outgoing_sum.eq(&incoming_sum));
        }

        edge_map
    }

    fn build_repeat_path_constraints(
        &self,
        part_index: usize,
        repeated: &LinearGraphRepeatPath<NIndex>,
        solver: &Solver,
        sums: &mut Box<[Int]>,
    ) -> Int {
        let count = Int::new_const(format!("repeat_path_{part_index}_count"));
        let zero = Int::from_i64(0);
        let one = Int::from_i64(1);
        solver.assert(count.ge(&zero));

        let (required, after_credit) = cfg_updates_to_counter_updates(
            repeated.path.transitions.iter().cloned(),
            self.linear_graph.dimension,
        );
        let positive_count = count.gt(&zero);

        for i in 0..self.linear_graph.dimension {
            let required_value = required[i];
            let effect = after_credit[i] - required_value;
            let required_ast = Int::from_i64(required_value as i64);
            solver.assert(positive_count.implies(sums[i].ge(&required_ast)));

            if effect < 0 {
                let last_iteration_start =
                    &sums[i] + (&count - &one) * Int::from_i64(effect as i64);
                solver.assert(positive_count.implies(last_iteration_start.ge(&required_ast)));
            }

            sums[i] = &sums[i] + &count * Int::from_i64(effect as i64);
        }

        count
    }

    fn build_boundary_lower_bound_constraints(
        &self,
        state: &NIndex,
        solver: &Solver,
        sums: &[Int],
    ) {
        let Some(lower_bound) = self.boundary_lower_bounds.get(state) else {
            return;
        };

        for template in lower_bound {
            let value = sums
                .iter()
                .zip(template.coefficients.iter())
                .filter(|(_, coefficient)| **coefficient != 0)
                .fold(Int::from_i64(0), |value, (sum, coefficient)| {
                    value + sum * Int::from_i64(*coefficient as i64)
                });
            solver.assert(value.ge(Int::from_i64(template.bound as i64)));
        }
    }

    fn max_iterations_reached(&self) -> bool {
        self.options
            .max_iterations
            .map(|x| x <= self.step_count)
            .unwrap_or(false)
    }

    fn max_time_reached(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }

    fn max_iterations_reached_result(&self) -> LinearGraphReachSolverResult {
        LinearGraphReachSolverResult::new(
            SolverStatus::Unknown(LinearGraphReachSolverError::MaxIterationsReached),
            self.get_solver_statistics(),
        )
    }

    fn max_time_reached_result(&self) -> LinearGraphReachSolverResult {
        LinearGraphReachSolverResult::new(
            SolverStatus::Unknown(LinearGraphReachSolverError::Timeout),
            self.get_solver_statistics(),
        )
    }

    fn get_solver_result(
        &self,
        status: LinearGraphReachSolverStatus,
    ) -> LinearGraphReachSolverResult {
        LinearGraphReachSolverResult::new(status, self.get_solver_statistics())
    }

    fn get_solver_statistics(&self) -> LinearGraphReachSolverStatistics {
        LinearGraphReachSolverStatistics::new(
            self.step_count,
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn get_solver_time(&self) -> Option<Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }
}
