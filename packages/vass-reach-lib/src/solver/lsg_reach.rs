use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
};

use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{
    graph::EdgeIndex,
    visit::{EdgeRef, IntoEdgeReferences},
};
use z3::{
    Config, Context, Solver,
    ast::{Ast, Bool, Int},
};

use crate::{
    automaton::{
        AutomatonNode,
        lsg::{LSGGraph, LSGPart, LSGPath, LinearSubGraph},
        path::parikh_image::ParikhImage,
        utils::cfg_updates_to_counter_updates,
        vass::counter::VASSCounterValuation,
    },
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
        lsg: &'g LinearSubGraph<'g, N>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
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
    lsg: &'g LinearSubGraph<'g, N>,
    initial_valuation: &'g VASSCounterValuation,
    final_valuation: &'g VASSCounterValuation,
    options: LSGReachSolverOptions<'l>,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<'l, 'g, N: AutomatonNode> LSGReachSolver<'l, 'g, N> {
    pub fn new(
        lsg: &'g LinearSubGraph<'g, N>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
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
        self.solver_start_time = Some(std::time::Instant::now());

        let mut config = Config::new();
        config.set_model_generation(true);
        let ctx = Context::new(&config);
        let solver = Solver::new(&ctx);

        let context_handle = ctx.handle();

        let start_time = self.solver_start_time.unwrap();
        let stop_signal = self.stop_signal.clone();
        let max_time = self.options.max_time;

        let mut result = None;

        thread::scope(|s| {
            s.spawn(|| {
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(10));

                    if let Some(max_time) = max_time
                        && start_time.elapsed() >= max_time
                    {
                        stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
                    }

                    if stop_signal.load(std::sync::atomic::Ordering::SeqCst) {
                        context_handle.interrupt();
                        break;
                    }
                }
            });

            result = Some(self.solve_inner(&ctx, &solver));

            stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
        });

        result.expect("Thread panicked")
    }

    fn solve_inner(&mut self, ctx: &Context, solver: &Solver) -> LSGReachSolverResult {
        let mut sums: Box<[_]> = self
            .initial_valuation
            .iter()
            .map(|x| Int::from_i64(ctx, *x as i64))
            .collect();

        for (sum, target) in sums.iter().zip(self.final_valuation.iter()) {
            solver.assert(&sum._eq(&Int::from_i64(ctx, *target as i64)));
        }

        let edge_maps = self
            .lsg
            .parts
            .iter()
            .enumerate()
            .filter_map(|(i, part)| match part {
                LSGPart::Path(path) => {
                    self.build_path_constraints(path, ctx, solver, &mut sums);
                    None
                }
                LSGPart::SubGraph(subgraph) => {
                    let edge_map =
                        self.build_subgraph_constraints(i, subgraph, ctx, solver, &mut sums);
                    Some(edge_map)
                }
            })
            .collect::<Vec<_>>();

        self.step_count = 1;

        loop {
            match solver.check() {
                z3::SatResult::Sat => {
                    let model = solver.get_model().unwrap();

                    let parikh_image_components = edge_maps
                        .iter()
                        .zip(self.lsg.iter_subgraph_parts())
                        .map(|(map, subgraph)| {
                            let parikh_image: HashMap<EdgeIndex, _> = map
                                .iter()
                                .map(|(id, var)| {
                                    (
                                        *id,
                                        model.get_const_interp(var).unwrap().as_u64().unwrap()
                                            as u32,
                                    )
                                })
                                .filter(|(_, count)| *count > 0)
                                .collect();

                            let image = ParikhImage::new(parikh_image);

                            let (_, components) = image
                                .split_into_connected_components(&subgraph.graph, subgraph.start);

                            (subgraph, map, components)
                        })
                        .collect::<Vec<_>>();

                    if parikh_image_components.iter().all(|(_, _, c)| c.is_empty()) {
                        return self.get_solver_result(LSGReachSolverStatus::True(LSGSolution {
                            parts: vec![],
                        }));
                    }

                    if self.max_iterations_reached() {
                        return self.max_iterations_reached_result();
                    }

                    if self.max_time_reached() {
                        return self.max_time_reached_result();
                    }

                    if let Some(l) = self.options.logger {
                        l.debug(&format!(
                            "Restricting {} connected components",
                            parikh_image_components
                                .iter()
                                .map(|(_, _, c)| c.len())
                                .sum::<usize>()
                        ));
                    }

                    for (subgraph, edge_map, components) in parikh_image_components.into_iter() {
                        for component in components {
                            // bools that represent whether each individual edge in the component is
                            // taken
                            let edges = component
                                .iter_edges()
                                .map(|edge| edge_map.get(&edge).unwrap().ge(&Int::from_i64(ctx, 1)))
                                .collect_vec();
                            let edges_ref = edges.iter().collect_vec();

                            // bool that represent whether each individual edge that is incoming
                            // from the component is taken
                            let incoming = component
                                .get_incoming_edges(&subgraph.graph)
                                .iter()
                                .map(|edge| edge_map.get(edge).unwrap().ge(&Int::from_i64(ctx, 1)))
                                .collect_vec();
                            let incoming_ref = incoming.iter().collect_vec();

                            let edges_ast = Bool::and(ctx, &edges_ref);
                            let incoming_ast = Bool::or(ctx, &incoming_ref);

                            // CONSTRAINT: if all edges in the component are taken, then at least
                            // one incoming edge must be taken as well
                            // this is because we need to enter the
                            // component. outgoing edges don't work
                            // because we may leave the component via a final
                            // state
                            solver.assert(&edges_ast.implies(&incoming_ast));
                        }
                    }

                    self.step_count += 1;
                }
                z3::SatResult::Unsat => {
                    return self.get_solver_result(LSGReachSolverStatus::False(()));
                }
                z3::SatResult::Unknown => {
                    return self.get_solver_result(LSGReachSolverStatus::Unknown(
                        LSGReachSolverError::SolverUnknown,
                    ));
                }
            }
        }
    }

    fn build_path_constraints<'c>(
        &self,
        path: &LSGPath,
        ctx: &'c Context,
        solver: &Solver,
        sums: &mut Box<[Int<'c>]>,
    ) {
        let path_updates = cfg_updates_to_counter_updates(
            path.path.iter_cfg_updates(self.lsg.cfg),
            self.lsg.dimension,
        );

        // first subtract the minimums
        for (update, sum) in path_updates.0.iter().zip(sums.iter_mut()) {
            let update_ast = Int::from_i64(ctx, *update as i64);
            *sum = &*sum - &update_ast;
        }

        // then assert non-negativity
        for sum in sums.iter() {
            let zero = Int::from_i64(ctx, 0);
            let geq_zero = sum.ge(&zero);
            solver.assert(&geq_zero);
        }

        // then add the rest to get the path's effect
        for (update, sum) in path_updates.1.iter().zip(sums.iter_mut()) {
            let update_ast = Int::from_i64(ctx, *update as i64);
            *sum = &*sum + &update_ast;
        }
    }

    fn build_subgraph_constraints<'c>(
        &self,
        part_index: usize,
        subgraph: &LSGGraph,
        ctx: &'c Context,
        solver: &Solver,
        sums: &mut Box<[Int<'c>]>,
    ) -> HashMap<EdgeIndex, Int<'c>> {
        let mut edge_map = HashMap::new();

        for edge in subgraph.graph.edge_references() {
            let edge_marking = edge.weight();

            // we need one variable for each edge
            let edge_var = Int::new_const(
                ctx,
                format!("graph_{}_edge_{}", part_index, edge.id().index()),
            );
            // CONSTRAINT: an edge can only be taken positive times
            solver.assert(&edge_var.ge(&Int::from_i64(ctx, 0)));

            // add the edges effect to the counter sum
            let i = edge_marking.counter();
            sums[i.to_usize()] = &sums[i.to_usize()] + &edge_var * edge_marking.op_i64();

            edge_map.insert(edge.id(), edge_var);
        }

        for node in subgraph.graph.node_indices() {
            let outgoing = subgraph
                .graph
                .edges_directed(node, petgraph::Direction::Outgoing);
            let incoming = subgraph
                .graph
                .edges_directed(node, petgraph::Direction::Incoming);

            // the end node has one additional outgoing connection, this works, because we
            // always have exactly one end node
            let mut outgoing_sum = if node == subgraph.end {
                Int::from_i64(ctx, 1)
            } else {
                Int::from_i64(ctx, 0)
            };
            // the start node has one additional incoming connection
            let mut incoming_sum = if node == subgraph.start {
                Int::from_i64(ctx, 1)
            } else {
                Int::from_i64(ctx, 0)
            };

            for edge in outgoing {
                let edge_var = edge_map.get(&edge.id()).unwrap();
                outgoing_sum += edge_var;
            }

            for edge in incoming {
                let edge_var = edge_map.get(&edge.id()).unwrap();
                incoming_sum += edge_var;
            }

            // CONSTRAINT: the sum of all outgoing edges must be equal to the sum of all
            // incoming edges for each node
            solver.assert(&outgoing_sum._eq(&incoming_sum));
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
        self.stop_signal.load(std::sync::atomic::Ordering::SeqCst)
    }

    fn max_iterations_reached_result(&self) -> LSGReachSolverResult {
        LSGReachSolverResult::new(
            SolverStatus::Unknown(LSGReachSolverError::MaxIterationsReached),
            self.get_solver_statistics(),
        )
    }

    fn max_time_reached_result(&self) -> LSGReachSolverResult {
        LSGReachSolverResult::new(
            SolverStatus::Unknown(LSGReachSolverError::Timeout),
            self.get_solver_statistics(),
        )
    }

    fn get_solver_result(&self, status: LSGReachSolverStatus) -> LSGReachSolverResult {
        LSGReachSolverResult::new(status, self.get_solver_statistics())
    }

    fn get_solver_statistics(&self) -> LSGReachSolverStatistics {
        LSGReachSolverStatistics::new(self.step_count, self.get_solver_time().unwrap_or_default())
    }

    fn get_solver_time(&self) -> Option<std::time::Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }
}
