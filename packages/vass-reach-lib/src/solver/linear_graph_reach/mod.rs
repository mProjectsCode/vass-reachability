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

mod types;

pub(crate) use types::LinearTemplateLowerBound;
pub use types::{
    LinearGraphReachSolverError, LinearGraphReachSolverOptions, LinearGraphReachSolverResult,
    LinearGraphReachSolverStatistics, LinearGraphReachSolverStatus, LinearGraphSolution,
};

use crate::{
    automaton::{
        AutomatonIterators, ExplicitEdgeAutomaton, GIndex,
        index_map::OptionIndexMap,
        linear_graph::{
            LinearGraph, LinearGraphAutomaton,
            part::{
                LinearGraphPart, LinearGraphPathSegment, LinearGraphRegion, LinearGraphRepeatPath,
            },
        },
        utils::cfg_updates_to_counter_updates,
        vass::counter::VASSCounterValuation,
    },
    solver::{
        SolverStatus,
        utils::{
            add_cfg_update_to_sums, assert_non_negative, assert_sums_match_valuation,
            forbid_parikh_image, parikh_image_from_edge_map,
        },
    },
};

pub struct LinearGraphReachSolver<'g, NIndex: GIndex + Send + Sync, A>
where
    A: LinearGraphAutomaton<NIndex> + Send + Sync,
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
    A: LinearGraphAutomaton<NIndex> + Send + Sync,
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

        assert_sums_match_valuation(solver, &sums, self.final_valuation);

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
            assert_non_negative(solver, &edge_var);

            // add the edges effect to the counter sum
            add_cfg_update_to_sums(sums, &edge_var, update);

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
        let one = Int::from_i64(1);
        assert_non_negative(solver, &count);

        let (required, after_credit) = cfg_updates_to_counter_updates(
            repeated.path.transitions.iter().cloned(),
            self.linear_graph.dimension,
        );
        let positive_count = count.gt(Int::from_i64(0));

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
