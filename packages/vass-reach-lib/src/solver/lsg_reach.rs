use std::{
    sync::{Arc, atomic::AtomicBool},
    thread,
};

use petgraph::graph::EdgeIndex;
use z3::{
    Config, Context, Solver,
    ast::{Ast, Int},
};

use crate::{
    automaton::{
        Automaton, ExplicitEdgeAutomaton,
        cfg::{CFG, update::CFGCounterUpdate},
        index_map::OptionIndexMap,
        lsg::{
            LinearSubGraph,
            part::{LSGGraph, LSGPart, LSGPath},
        },
        path::{Path, parikh_image::ParikhImage},
        utils::{cfg_updates_to_counter_update, cfg_updates_to_counter_updates},
        vass::counter::VASSCounterValuation,
    },
    logger::Logger,
    solver::{
        SolverResult, SolverStatus,
        utils::{forbid_parikh_image, parikh_image_from_edge_map},
    },
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

    pub fn to_solver<'g, C: CFG>(
        self,
        lsg: &'g LinearSubGraph<'g, C>,
        initial_valuation: &'g VASSCounterValuation,
        final_valuation: &'g VASSCounterValuation,
    ) -> LSGReachSolver<'l, 'g, C> {
        LSGReachSolver::new(lsg, initial_valuation, final_valuation, self)
    }
}

#[derive(Debug, Clone)]
pub enum LSGSolutionPart {
    SubGraph(ParikhImage<EdgeIndex>),
    Path(),
}

#[derive(Debug, Clone)]
pub struct LSGSolution {
    pub parts: Vec<LSGSolutionPart>,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
}

impl LSGSolution {
    pub fn build_run<'a, C: CFG>(
        &self,
        lsg: &LinearSubGraph<'a, C>,
        n_run: bool,
    ) -> Option<Path<C::NIndex, CFGCounterUpdate>> {
        // the parikh image already determines the initial and final valuations when
        // entering and leaving subgraphs so we can simply build the runs for
        // each part independently and concatenate them

        let dimension = self.initial_valuation.dimension();

        let mut cfg_path = Path::new(lsg.cfg.get_initial());

        let mut current_valuation = self.initial_valuation.clone();

        for (part, lsg_part) in self.parts.iter().zip(lsg.iter_parts()) {
            match part {
                LSGSolutionPart::SubGraph(image) => {
                    let subgraph = lsg_part.unwrap_subgraph();

                    // first we need to calculate the start and end valuations for the subgraph
                    let start_valuation = current_valuation.clone();
                    current_valuation
                        .apply_update(&image.get_total_counter_effect(subgraph, dimension));
                    let end_valuation = current_valuation.clone();

                    // then we can build the run for the subgraph
                    let sub_path =
                        image.build_run(subgraph, &start_valuation, &end_valuation, n_run)?;

                    // now the node and edge indices in the path are for the subgraph, so we
                    // need to map them back to the cfg
                    let mapped_path = subgraph.map_path_to_cfg(&sub_path);

                    cfg_path.concatenate(mapped_path);
                }
                LSGSolutionPart::Path() => {
                    let path = lsg_part.unwrap_path();

                    // we need to update the current valuation for possible following subgraphs
                    let update =
                        cfg_updates_to_counter_update(path.path.iter_letters().cloned(), dimension);

                    current_valuation.apply_update(&update);

                    // then we can simply add the edges to the path
                    cfg_path.concatenate(path.path.clone());
                }
            }
        }

        assert_eq!(
            &current_valuation, &self.final_valuation,
            "Final valuation does not match the expected final valuation"
        );

        Some(cfg_path)
    }
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

pub struct LSGReachSolver<'l, 'g, C: CFG> {
    lsg: &'g LinearSubGraph<'g, C>,
    initial_valuation: &'g VASSCounterValuation,
    final_valuation: &'g VASSCounterValuation,
    options: LSGReachSolverOptions<'l>,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<'l, 'g, C: CFG> LSGReachSolver<'l, 'g, C> {
    pub fn new(
        lsg: &'g LinearSubGraph<'g, C>,
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

        for (sum, target) in sums.iter().zip(self.final_valuation.iter()) {
            solver.assert(&sum._eq(&Int::from_i64(ctx, *target as i64)));
        }

        self.step_count = 1;

        loop {
            match solver.check() {
                z3::SatResult::Sat => {
                    let model = solver.get_model().unwrap();

                    let parikh_image_components = edge_maps
                        .iter()
                        .zip(self.lsg.iter_subgraph_parts())
                        .map(|(map, subgraph)| {
                            let image = parikh_image_from_edge_map(map, &model);

                            let (main_component, components) =
                                image.split_into_connected_components(subgraph);

                            (subgraph, map, main_component, components)
                        })
                        .collect::<Vec<_>>();

                    if parikh_image_components
                        .iter()
                        .all(|(_, _, _, c)| c.is_empty())
                    {
                        let mut solution_parts = Vec::new();
                        let mut image_drain = parikh_image_components.into_iter();

                        for part in self.lsg.parts.iter() {
                            match part {
                                LSGPart::Path(_) => {
                                    solution_parts.push(LSGSolutionPart::Path());
                                }
                                LSGPart::SubGraph(_) => {
                                    let (_, _, main_component, _) = image_drain.next().unwrap();
                                    solution_parts.push(LSGSolutionPart::SubGraph(main_component));
                                }
                            }
                        }

                        return self.get_solver_result(LSGReachSolverStatus::True(LSGSolution {
                            parts: solution_parts,
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

                    if let Some(l) = self.options.logger {
                        l.debug(&format!(
                            "Restricting {} connected components",
                            parikh_image_components
                                .iter()
                                .map(|(_, _, _, c)| c.len())
                                .sum::<usize>()
                        ));
                    }

                    for (subgraph, edge_map, _, components) in parikh_image_components.into_iter() {
                        for component in components {
                            forbid_parikh_image(&component, subgraph, edge_map, solver, ctx);
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
        path: &LSGPath<C::NIndex>,
        ctx: &'c Context,
        solver: &Solver,
        sums: &mut Box<[Int<'c>]>,
    ) {
        let path_updates =
            cfg_updates_to_counter_updates(path.path.iter_letters().cloned(), self.lsg.dimension);

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
        subgraph: &LSGGraph<C::NIndex>,
        ctx: &'c Context,
        solver: &Solver,
        sums: &mut Box<[Int<'c>]>,
    ) -> OptionIndexMap<EdgeIndex, Int<'c>> {
        let mut edge_map = OptionIndexMap::new(subgraph.edge_count());

        for (edge, update) in subgraph.iter_edges() {
            // we need one variable for each edge
            let edge_var =
                Int::new_const(ctx, format!("graph_{}_edge_{}", part_index, edge.index()));
            // CONSTRAINT: an edge can only be taken positive times
            solver.assert(&edge_var.ge(&Int::from_i64(ctx, 0)));

            // add the edges effect to the counter sum
            let i = update.counter();
            sums[i.to_usize()] = &sums[i.to_usize()] + &edge_var * update.op_i64();

            edge_map.insert(edge, edge_var);
        }

        for node in subgraph.iter_node_indices() {
            let outgoing = subgraph.outgoing_edge_indices(node);
            let incoming = subgraph.incoming_edge_indices(node);

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
                let edge_var = &edge_map[edge];
                outgoing_sum += edge_var;
            }

            for edge in incoming {
                let edge_var = &edge_map[edge];
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
