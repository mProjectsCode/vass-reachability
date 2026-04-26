use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use petgraph::graph::EdgeIndex;
use z3::{Config, Context, Solver, ast::Int, with_z3_config};

use crate::{
    automaton::{
        Alphabet, AutomatonIterators, Deterministic, ExplicitEdgeAutomaton, GIndex,
        InitializedAutomaton, TransitionSystem,
        cfg::update::CFGCounterUpdate,
        index_map::OptionIndexMap,
        mgts::{
            MGTS,
            part::{MGTSPart, MarkedGraph, MarkedPath},
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

#[derive(Debug, Default)]
pub struct MGTSReachSolverOptions {
    max_iterations: Option<u32>,
    max_time: Option<Duration>,
    stop_signal: Option<Arc<AtomicBool>>,
}

impl MGTSReachSolverOptions {
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

    pub fn to_solver<'g, NIndex: GIndex + Send + Sync, A>(
        self,
        mgts: &'g MGTS<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
    ) -> MGTSReachSolver<'g, NIndex, A>
    where
        A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
            + SCCAlgorithms
            + Alphabet<Letter = CFGCounterUpdate>
            + Send
            + Sync,
    {
        MGTSReachSolver::new(mgts, initial_valuation, final_valuation, self)
    }
}

#[derive(Debug, Clone)]
pub struct MGTSSolution {
    pub sub_graph_parikh_images: Vec<ParikhImage<EdgeIndex>>,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
}

impl MGTSSolution {
    pub fn build_run<'a, NIndex: GIndex, A>(
        &self,
        mgts: &MGTS<'a, NIndex, A>,
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

        let mut product_path = Path::<NIndex, CFGCounterUpdate>::new(mgts.automaton.get_initial());

        let mut current_valuation = self.initial_valuation.clone();

        // TODO: this does not work at all with inconsistent LGSs. If we have an unused
        // path or graph we are in trouble. First we do more solving and then we also
        // run into index errors here.

        for part in mgts.sequence.iter() {
            match part {
                MGTSPart::Graph(idx) => {
                    let graph = mgts.graph(*idx);
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
                MGTSPart::Path(idx) => {
                    let path = mgts.path(*idx);

                    // we need to update the current valuation for possible following graphs
                    let update = cfg_updates_to_counter_update(
                        path.path.transitions.iter().cloned(),
                        dimension,
                    );

                    current_valuation.apply_update(&update);

                    // then we can simply add the edges to the path
                    product_path.concat(path.path.clone());
                }
            }
        }

        assert_eq!(
            &current_valuation, &self.final_valuation,
            "Final valuation does not match the expected final valuation"
        );

        tracing::debug!(
            "Built run for MGTSs solution in {} ms",
            timer.elapsed().as_millis()
        );

        Some(product_path)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MGTSReachSolverError {
    Timeout,
    MaxIterationsReached,
    SolverUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MGTSReachSolverStatistics {
    pub step_count: u32,
    pub time: Duration,
}

impl MGTSReachSolverStatistics {
    pub fn new(step_count: u32, time: Duration) -> Self {
        MGTSReachSolverStatistics { step_count, time }
    }
}

pub type MGTSReachSolverStatus = SolverStatus<MGTSSolution, (), MGTSReachSolverError>;

pub type MGTSReachSolverResult =
    SolverResult<MGTSSolution, (), MGTSReachSolverError, MGTSReachSolverStatistics>;

impl MGTSReachSolverResult {
    pub fn get_solution(&self) -> Option<&MGTSSolution> {
        match &self.status {
            SolverStatus::True(solution) => Some(solution),
            _ => None,
        }
    }
}

pub struct MGTSReachSolver<'g, NIndex: GIndex + Send + Sync, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>
        + Send
        + Sync,
{
    mgts: &'g MGTS<'g, NIndex, A>,
    initial_valuation: &'g VASSCounterValuation,
    final_valuation: &'g VASSCounterValuation,
    options: MGTSReachSolverOptions,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<'g, NIndex: GIndex + Send + Sync, A> MGTSReachSolver<'g, NIndex, A>
where
    A: InitializedAutomaton<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + TransitionSystem<Deterministic, NIndex = NIndex, Letter = CFGCounterUpdate>
        + SCCAlgorithms
        + Alphabet<Letter = CFGCounterUpdate>
        + Send
        + Sync,
{
    pub fn new(
        mgts: &'g MGTS<'g, NIndex, A>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
        options: MGTSReachSolverOptions,
    ) -> Self {
        let stop_signal = options
            .stop_signal
            .clone()
            .unwrap_or(Arc::new(AtomicBool::new(false)));

        mgts.assert_consistent();

        MGTSReachSolver {
            mgts,
            initial_valuation,
            final_valuation,
            options,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
        }
    }

    pub fn solve(&mut self) -> MGTSReachSolverResult {
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
                "MGTS reachability solver finished in {} ms",
                self.get_solver_time().unwrap_or_default().as_millis()
            );

            result.expect("Thread panicked")
        })
    }

    fn solve_inner(&mut self, solver: &Solver) -> MGTSReachSolverResult {
        let mut sums: Box<[_]> = self
            .initial_valuation
            .iter()
            .map(|x| Int::from_i64(*x as i64))
            .collect();

        let edge_maps = self
            .mgts
            .sequence
            .iter()
            .enumerate()
            .filter_map(|(i, part)| match part {
                MGTSPart::Path(idx) => {
                    self.build_path_constraints(self.mgts.path(*idx), solver, &mut sums);
                    None
                }
                MGTSPart::Graph(idx) => {
                    let edge_map =
                        self.build_graph_constraints(i, self.mgts.graph(*idx), solver, &mut sums);
                    Some(edge_map)
                }
            })
            .collect::<Vec<_>>();

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
                        .zip(self.mgts.iter_graph_parts())
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
                        return self.get_solver_result(MGTSReachSolverStatus::True(MGTSSolution {
                            sub_graph_parikh_images: parikh_image_components
                                .into_iter()
                                .map(|(_, _, main_component, _)| main_component)
                                .collect(),
                            initial_valuation: self.initial_valuation.clone(),
                            final_valuation: self.final_valuation.clone(),
                        }));
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
                    return self.get_solver_result(MGTSReachSolverStatus::False(()));
                }
                z3::SatResult::Unknown => {
                    return self.get_solver_result(MGTSReachSolverStatus::Unknown(
                        MGTSReachSolverError::SolverUnknown,
                    ));
                }
            }
        }
    }

    fn build_path_constraints(
        &self,
        path: &MarkedPath<NIndex>,
        solver: &Solver,
        sums: &mut Box<[Int]>,
    ) {
        let path_updates = cfg_updates_to_counter_updates(
            path.path.transitions.iter().cloned(),
            self.mgts.dimension,
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
        graph: &MarkedGraph<NIndex>,
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

    fn max_iterations_reached(&self) -> bool {
        self.options
            .max_iterations
            .map(|x| x <= self.step_count)
            .unwrap_or(false)
    }

    fn max_time_reached(&self) -> bool {
        self.stop_signal.load(Ordering::SeqCst)
    }

    fn max_iterations_reached_result(&self) -> MGTSReachSolverResult {
        MGTSReachSolverResult::new(
            SolverStatus::Unknown(MGTSReachSolverError::MaxIterationsReached),
            self.get_solver_statistics(),
        )
    }

    fn max_time_reached_result(&self) -> MGTSReachSolverResult {
        MGTSReachSolverResult::new(
            SolverStatus::Unknown(MGTSReachSolverError::Timeout),
            self.get_solver_statistics(),
        )
    }

    fn get_solver_result(&self, status: MGTSReachSolverStatus) -> MGTSReachSolverResult {
        MGTSReachSolverResult::new(status, self.get_solver_statistics())
    }

    fn get_solver_statistics(&self) -> MGTSReachSolverStatistics {
        MGTSReachSolverStatistics::new(self.step_count, self.get_solver_time().unwrap_or_default())
    }

    fn get_solver_time(&self) -> Option<Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }
}
