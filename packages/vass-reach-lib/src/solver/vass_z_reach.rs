use petgraph::graph::EdgeIndex;
use serde::{Deserialize, Serialize};
use z3::{Config, Solver, ast::Int, with_z3_config};

use crate::{
    automaton::{
        AutomatonIterators,
        cfg::{ExplicitEdgeCFG, update::CFGCounterUpdate},
        index_map::OptionIndexMap,
        path::{Path, parikh_image::ParikhImage},
        vass::counter::VASSCounterValuation,
    },
    config::VASSZReachConfig,
    solver::{
        SolverResult, SolverStatus,
        utils::{forbid_parikh_image, parikh_image_from_edge_map},
    },
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VASSZReachSolverError {
    Timeout,
    MaxIterationsReached,
    SolverUnknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VASSZReachSolverStatistics {
    pub step_count: u64,
    pub time: std::time::Duration,
}

impl VASSZReachSolverStatistics {
    pub fn new(step_count: u64, time: std::time::Duration) -> Self {
        VASSZReachSolverStatistics { step_count, time }
    }
}

pub type VASSZReachSolverStatus = SolverStatus<ParikhImage<EdgeIndex>, (), VASSZReachSolverError>;

pub type VASSZReachSolverResult =
    SolverResult<ParikhImage<EdgeIndex>, (), VASSZReachSolverError, VASSZReachSolverStatistics>;

impl VASSZReachSolverResult {
    pub fn get_parikh_image(&self) -> Option<&ParikhImage<EdgeIndex>> {
        match &self.status {
            SolverStatus::True(parikh_image) => Some(parikh_image),
            _ => None,
        }
    }

    pub fn build_run<C: ExplicitEdgeCFG>(
        &self,
        cfg: &C,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
        n_run: bool,
    ) -> Option<Path<C::NIndex, CFGCounterUpdate>> {
        self.get_parikh_image()?
            .build_run(cfg, initial_valuation, final_valuation, n_run)
    }
}

/// Solves a VASS CFG for Z-Reachability.
///
/// The basic idea is to use a SAT solver to find a Z-Run through the CFG.
///
/// We create a variable for each edge in the CFG that represents how often the
/// edge is taken. Additionally we have one variable for each accepting node
/// that represents whether the node is used as the final node. We then create
/// constraints that ensure that the sum of all incoming edges is equal to the
/// sum of all outgoing edges for each node. (Kirchhoff Equations)
/// We also create constraints that ensure that the final valuation is equal to
/// the sum of all edge valuations plus the initial valuation.
///
/// We then check if the constraints are satisfiable.
/// Due to the nature of the the Kirchhoff Equations, the Parikh Image generated
/// by the solver may not form a connected Z-Run. Should a solution not be
/// connected, we add an additional constraint that forces:
///
/// > If all edges in a connected component are taken, then at least one
/// > outgoing edge (to a node that is not part of the connected component) must
/// > be taken as well.
///
/// This constraint ensures that the connected component must either be bigger
/// or connected to the main Z-Run in the next iteration.
///
/// Since this constraint act's on sets of nodes and there are only a limited
/// number of subsets of nodes, the solver terminates.
pub struct VASSZReachSolver<'c, C: ExplicitEdgeCFG + Sync> {
    cfg: &'c C,
    initial_valuation: VASSCounterValuation,
    final_valuation: VASSCounterValuation,
    options: VASSZReachConfig,
    step_count: u64,
    solver_start_time: Option<std::time::Instant>,
}

impl<'c, C: ExplicitEdgeCFG + Sync> VASSZReachSolver<'c, C> {
    pub fn new(
        cfg: &'c C,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        options: VASSZReachConfig,
    ) -> Self {
        VASSZReachSolver {
            cfg,
            initial_valuation,
            final_valuation,
            options,
            step_count: 0,
            solver_start_time: None,
        }
    }

    pub fn solve(&mut self) -> VASSZReachSolverResult {
        self.solver_start_time = Some(std::time::Instant::now());

        let mut config = Config::new();
        config.set_model_generation(true);
        with_z3_config(&config, || {
            let solver = Solver::new();

            self.solve_inner(&solver)
        })
    }

    fn solve_inner(&mut self, solver: &Solver) -> VASSZReachSolverResult {
        // a map that allows us to access the edge variables by their edge id
        let mut edge_map = OptionIndexMap::new(self.cfg.edge_count());

        // all the counter sums along the path
        let mut sums: Box<[_]> = self
            .initial_valuation
            .iter()
            .map(|x| Int::from_i64(*x as i64))
            .collect();

        for (edge, update) in self.cfg.iter_edges() {
            // we need one variable for each edge
            let edge_var = Int::new_const(format!("edge_{}", edge.index()));
            // CONSTRAINT: an edge can only be taken positive times
            solver.assert(edge_var.ge(Int::from_i64(0)));

            // add the edges effect to the counter sum
            let i = update.counter();
            sums[i.to_usize()] = &sums[i.to_usize()] + &edge_var * update.op_i64();

            edge_map.insert(edge, edge_var);
        }

        let mut final_var_sum = Int::from_i64(0);

        for node in self.cfg.iter_node_indices() {
            let outgoing = self.cfg.outgoing_edge_indices(&node);
            let incoming = self.cfg.incoming_edge_indices(&node);

            let mut outgoing_sum = Int::from_i64(0);
            // the start node has one additional incoming connection
            let mut incoming_sum = if node == self.cfg.get_initial() {
                Int::from_i64(1)
            } else {
                Int::from_i64(0)
            };

            if self.cfg.is_accepting(&node) {
                // for each accepting node, we need some additional variable that denotes
                // whether the node is used as the final node
                let final_var = Int::new_const(format!("node_{}_final", node.index()));
                solver.assert(final_var.ge(Int::from_i64(0)));

                outgoing_sum += &final_var;
                final_var_sum += &final_var;
            }

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
            solver.assert(outgoing_sum.eq(incoming_sum));
        }

        // CONSTRAINT: only one final variable can be set
        solver.assert(final_var_sum.eq(Int::from_i64(1)));

        // CONSTRAINT: the final valuation must be equal to the counter sums
        for (sum, target) in sums.iter().zip(self.final_valuation.iter()) {
            solver.assert(sum.eq(Int::from_i64(*target as i64)));
        }

        self.step_count = 1;
        let status;

        loop {
            match solver.check() {
                z3::SatResult::Sat => {
                    let model = solver.get_model();
                    let Some(model) = model else {
                        status = SolverStatus::Unknown(VASSZReachSolverError::SolverUnknown);
                        break;
                    };

                    let parikh_image = parikh_image_from_edge_map(&edge_map, &model);
                    let (_, components) = parikh_image
                        .clone()
                        .split_into_connected_components(self.cfg);

                    if components.is_empty() {
                        status = SolverStatus::True(parikh_image);
                        break;
                    }

                    if self.max_iterations_reached() {
                        return self.max_iterations_reached_result();
                    }

                    if self.max_time_reached() {
                        return self.max_time_reached_result();
                    }

                    tracing::debug!("Restricting {} connected components", components.len());

                    for component in components {
                        forbid_parikh_image(&component, self.cfg, &edge_map, solver);
                    }

                    self.step_count += 1;
                }
                z3::SatResult::Unsat => {
                    status = SolverStatus::False(());
                    break;
                }
                z3::SatResult::Unknown => {
                    status = SolverStatus::Unknown(VASSZReachSolverError::SolverUnknown);
                    break;
                }
            };
        }

        tracing::debug!("Solved Z-Reach in {} steps", self.step_count);

        self.get_solver_result(status)
    }

    fn max_iterations_reached(&self) -> bool {
        self.options
            .get_max_iterations()
            .map(|x| x <= self.step_count)
            .unwrap_or(false)
    }

    fn max_time_reached(&self) -> bool {
        match (self.get_solver_time(), self.options.get_timeout()) {
            (Some(t), Some(max_time)) => &t > max_time,
            _ => false,
        }
    }

    fn max_iterations_reached_result(&self) -> VASSZReachSolverResult {
        VASSZReachSolverResult::new(
            SolverStatus::Unknown(VASSZReachSolverError::MaxIterationsReached),
            self.get_solver_statistics(),
        )
    }

    fn max_time_reached_result(&self) -> VASSZReachSolverResult {
        VASSZReachSolverResult::new(
            SolverStatus::Unknown(VASSZReachSolverError::Timeout),
            self.get_solver_statistics(),
        )
    }

    fn get_solver_statistics(&self) -> VASSZReachSolverStatistics {
        VASSZReachSolverStatistics::new(self.step_count, self.get_solver_time().unwrap_or_default())
    }

    fn get_solver_result(&self, status: VASSZReachSolverStatus) -> VASSZReachSolverResult {
        VASSZReachSolverResult::new(status, self.get_solver_statistics())
    }

    fn get_solver_time(&self) -> Option<std::time::Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }
}
