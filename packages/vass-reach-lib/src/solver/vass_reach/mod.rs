use petgraph::graph::NodeIndex;

pub mod debug_trace;
mod preprocess;
mod types;
mod witness;

pub use types::{
    VASSReachRefinementAction, VASSReachSolverError, VASSReachSolverResult,
    VASSReachSolverStatistics, VASSReachSolverStatus,
};

use self::debug_trace::DebugTraceWriter;
use crate::{
    automaton::{
        Automaton, AutomatonEdge, AutomatonNode, FromLetter, GIndex, InitializedAutomaton,
        algorithms::EdgeAutomatonAlgorithms,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::minimization::Minimizable,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        linear_graph::{
            LinearGraph,
            extender::{LinearGraphExtender, LinearGraphExtenderOutput},
        },
        ltc::{LTC, translation::LTCTranslation},
        path::Path,
        scc::{SCCAlgorithms, SCCDag, SCCDagRouteSummary},
        vass::initialized::InitializedVASS,
    },
    config::{ModuloMode, VASSReachConfig, VASSZReachConfig},
    solver::{SolverStatus, vass_z_reach::VASSZReachSolver},
};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

#[derive(Debug)]
pub struct VASSReachSolver {
    config: VASSReachConfig,
    state: ImplicitCFGProduct,
    initial_status: Option<VASSReachSolverStatus>,
    step_count: u64,
    solver_start_time: Option<std::time::Instant>,
    debug_trace_writer: Option<DebugTraceWriter>,
}

impl VASSReachSolver {
    pub fn new<N: AutomatonNode, E: AutomatonEdge + FromLetter>(
        ivass: &InitializedVASS<N, E>,
        config: VASSReachConfig,
    ) -> Self {
        let time = std::time::Instant::now();

        let short_witness = witness::find_short_witness(ivass, config.get_short_witness());
        let mut initial_status = short_witness
            .as_ref()
            .map(|_| VASSReachSolverStatus::True(()));
        if let Some(found) = &short_witness {
            tracing::info!(
                depth = found.depth,
                explored_configurations = found.explored_configurations,
                "Short witness precheck found an N-reaching run"
            );
        }

        let mut cfg = ivass.to_cfg();
        cfg.make_complete(());
        cfg = cfg.minimize();

        if initial_status.is_none() {
            let unprocessed_cfg = cfg.clone();
            cfg = match preprocess::run_preprocess_unreachable_linear_graph_from_scc_dag(
                cfg,
                &ivass.initial_valuation,
                &ivass.final_valuation,
                &config,
                Some(time),
            ) {
                Ok(preprocess::PreprocessOutcome::Refined(cfg)) => cfg,
                Ok(preprocess::PreprocessOutcome::Reachable(run)) => {
                    tracing::info!(
                        run_length = run.len(),
                        "Reused concrete N-reaching run from LinearGraph preprocessing"
                    );
                    initial_status = Some(SolverStatus::True(()));
                    unprocessed_cfg
                }
                Err(status @ SolverStatus::Unknown(VASSReachSolverError::Timeout)) => {
                    tracing::warn!("CFG preprocessing exhausted the global solver timeout");
                    initial_status = Some(status);
                    unprocessed_cfg
                }
                Err(status) => {
                    tracing::warn!(
                        ?status,
                        "CFG preprocessing failed; continuing with unprocessed CFG"
                    );
                    unprocessed_cfg
                }
            };
        }

        tracing::debug!("{}", cfg.to_graphviz(None, None));

        let bounded_counting_enabled = *config.get_bounded_counting_enabled();

        let state = ImplicitCFGProduct::new(
            ivass.dimension(),
            ivass.initial_valuation.clone(),
            ivass.final_valuation.clone(),
            cfg,
            bounded_counting_enabled,
        );

        let debug_trace_writer = match DebugTraceWriter::from_config(&config, ivass) {
            Ok(writer) => writer,
            Err(err) => {
                tracing::warn!(error = %err, "failed to initialize debug trace writer; continuing without trace output");
                None
            }
        };

        tracing::info!("Solver initialized in {:?}", time.elapsed());

        VASSReachSolver {
            config,
            state,
            initial_status,
            step_count: 0,
            solver_start_time: None,
            debug_trace_writer,
        }
    }

    pub fn solve(&mut self) -> VASSReachSolverResult {
        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();

        let status = self
            .solve_inner()
            .expect_err("expected solve_inner to return the result as an Err value");
        let result = VASSReachSolverResult::new(status, self.get_solver_statistics());
        if let Some(writer) = &mut self.debug_trace_writer
            && let Err(err) = writer.write_light_result(&result)
        {
            tracing::warn!(error = %err, "failed to write light debug trace result");
        }
        self.print_end_banner(&result);

        result
    }

    fn solve_inner(&mut self) -> Result<(), VASSReachSolverStatus> {
        let mut step_time;

        if let Some(status) = &self.initial_status {
            return Err(status.clone());
        }

        self.z_reach_precheck()?;

        loop {
            self.step_count += 1;

            self.max_iterations_reached()?;
            self.max_time_reached()?;

            step_time = std::time::Instant::now();

            tracing::info!(
                step = %self.step_count,
                mu = ?self.state.mu,
                forward_bounds = ?self.state.get_forward_bounds(),
                backward_bounds = ?self.state.get_backward_bounds(),
                intersection_size = %self.state.cfgs.len(),
                "Step Info"
            );

            // Run reachability on the current approximation
            let reach_path = self.state.reach();

            // Since we over-approximate reachability, not finding a path means there can't
            // be a real one
            let Some(path) = reach_path else {
                tracing::info!("No path in approximation found. Instance is unreachable.");

                return Err(SolverStatus::False(()));
            };

            let is_n_reaching =
                path.is_n_reaching(&self.state.initial_valuation, &self.state.final_valuation);

            self.write_debug_trace_seed(&path, is_n_reaching);

            // We check if we by change found a real N-reaching path
            if is_n_reaching {
                tracing::info!("Found N-reaching path: {:?}", path.to_fancy_string());

                return Err(SolverStatus::True(()));
            }

            tracing::debug!("Spurious path of length: {:?}", path.len());

            if false {
                let cfg_path = path.to_path_in_cfg(self.state.main_cfg_index());
                // tracing::debug!("{:?}", cfg_path);

                let graphviz = self
                    .state
                    .main_cfg()
                    .to_graphviz(None, Some(cfg_path.visited_edges(self.state.main_cfg())));
                tracing::debug!("{}", graphviz);
            }

            // Now we know that the path is spurious, we need to refine our approximation.
            self.refinement_step(path)?;

            tracing::debug!("Step time: {:?}", step_time.elapsed());
        }
    }

    fn z_reach_precheck(&mut self) -> Result<(), VASSReachSolverStatus> {
        if !*self.config.get_preprocessing().get_enabled()
            || !*self
                .config
                .get_preprocessing()
                .get_z_reach_precheck_enabled()
        {
            return Ok(());
        }

        let presolve_time = std::time::Instant::now();

        let z_reach_config = VASSZReachConfig::default()
            .with_timeout(*self.config.get_timeout())
            .with_max_iterations(*self.config.get_max_iterations());

        let z_reach_result = VASSZReachSolver::new(
            self.state.main_cfg(),
            self.state.initial_valuation.clone(),
            self.state.final_valuation.clone(),
            z_reach_config,
        )
        .solve();

        tracing::info!("Preprocessing finished in {:?}", presolve_time.elapsed());

        match z_reach_result.status {
            SolverStatus::False(_) => {
                tracing::info!("Z-reach pre-check proved instance unreachable");
                Err(SolverStatus::False(()))
            }
            SolverStatus::True(_) => Ok(()),
            SolverStatus::Unknown(reason) => {
                tracing::warn!(
                    ?reason,
                    "Z-reach pre-check returned unknown; continuing with N-reach solver"
                );
                Ok(())
            }
        }
    }

    fn write_debug_trace_seed(&self, path: &MultiGraphPath, is_n_reaching: bool) {
        let Some(writer) = &self.debug_trace_writer else {
            return;
        };

        let dag = self.state.find_scc_dag();

        if let Err(err) = writer.write_step_seed(
            self.step_count,
            &self.state.initial_valuation,
            path,
            &dag,
            &self.state,
            is_n_reaching,
        ) {
            tracing::warn!(
                step = self.step_count,
                error = %err,
                trace_dir = %writer.run_dir().display(),
                "failed to write debug trace step seed"
            );
        }
    }

    fn refinement_step(&mut self, path: MultiGraphPath) -> Result<(), VASSReachSolverStatus> {
        // We select a refinement action based on the path.
        match self.select_refinement_action(&path) {
            VASSReachRefinementAction::IncreaseModulo(counter_index, x) => {
                let current_mu = self.state.get_mu(counter_index);
                let new_mu = match self.config.get_modulo().get_mode() {
                    ModuloMode::Increment => current_mu + 1,
                    ModuloMode::LeastCommonMultiple => {
                        let mut new_mu = current_mu;
                        while x.rem_euclid(new_mu)
                            == self.state.final_valuation[counter_index].rem_euclid(new_mu)
                        {
                            new_mu += current_mu;
                        }
                        new_mu
                    }
                };
                self.state.set_mu(counter_index, new_mu);

                tracing::debug!(
                    "Increasing mu for counter {:?} from {:?} to {:?}",
                    counter_index,
                    current_mu,
                    new_mu
                );
            }
            VASSReachRefinementAction::IncreaseForwardsBound(counter_index, bound) => {
                self.state.set_forward_bound(counter_index, bound);

                tracing::debug!(
                    "Increasing forward bound for counter {:?} to {:?}",
                    counter_index,
                    bound
                );
            }
            VASSReachRefinementAction::IncreaseBackwardsBound(counter_index, bound) => {
                self.state.set_backward_bound(counter_index, bound);

                tracing::debug!(
                    "Increasing backward bound for counter {:?} to {:?}",
                    counter_index,
                    bound
                );
            }
            VASSReachRefinementAction::BuildAutomaton => {
                let cfg = self.build_linear_graph_refinement_cfg(path)?;
                self.state.add_cfg(cfg.minimize());
            }
        }

        Ok(())
    }

    fn build_linear_graph_refinement_cfg(
        &mut self,
        primary_path: MultiGraphPath,
    ) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
        tracing::debug!("Building and checking LinearGraph");

        let starting_paths = self.linear_graph_starting_paths(primary_path);
        let fallback_primary_path = starting_paths[0].clone();
        let product_view = self.state.full_view();
        let view_paths = starting_paths
            .iter()
            .map(|path| product_view.project_path(path))
            .collect::<Vec<_>>();
        let full_dag = product_view.find_scc_dag();
        self.max_time_reached()?;
        log_scc_dag_route_summary_before_linear_graph("implicit_product", &full_dag);

        let mut extender = if let Some(remaining) = self.remaining_solver_time() {
            LinearGraphExtender::from_product_view_paths_with_config_and_time_limit(
                view_paths,
                &product_view,
                self.config.get_linear_graph(),
                remaining,
            )
        } else {
            LinearGraphExtender::from_product_view_paths_with_config(
                view_paths,
                &product_view,
                self.config.get_linear_graph(),
            )
        }
        .with_scc_dag(full_dag);
        let mut cfg = match extender.run_with_witness() {
            LinearGraphExtenderOutput::Refinement(cfg) => cfg,
            LinearGraphExtenderOutput::Reachable(run)
                if product_view.is_accepting(run.end())
                    && run.is_n_reaching(
                        &self.state.initial_valuation,
                        &self.state.final_valuation,
                    ) =>
            {
                tracing::info!(
                    run_length = run.len(),
                    "Reused concrete N-reaching run from LinearGraph refinement"
                );
                return Err(SolverStatus::True(()));
            }
            LinearGraphExtenderOutput::Reachable(run) => {
                tracing::warn!(
                    run_length = run.len(),
                    "Rejected invalid reachable run returned by LinearGraph refinement"
                );
                LinearGraph::from_path(fallback_primary_path, &product_view, self.state.dimension)
                    .to_cfg()
            }
            LinearGraphExtenderOutput::Timeout => {
                return Err(SolverStatus::Unknown(VASSReachSolverError::Timeout));
            }
        };
        cfg.invert_mut();
        Ok(cfg)
    }

    fn linear_graph_starting_paths(&self, primary_path: MultiGraphPath) -> Vec<MultiGraphPath> {
        let config = self.config.get_linear_graph();
        let extra_paths = *config.get_extra_auxiliary_paths();

        if !*config.get_multiple_starting_paths_enabled() || extra_paths == 0 {
            return vec![primary_path];
        }

        let max_paths = extra_paths.saturating_add(1);
        let mut paths = vec![primary_path];

        for candidate in self.state.reach_paths(max_paths) {
            if paths.len() >= max_paths {
                break;
            }

            if !paths.iter().any(|path| path == &candidate) {
                paths.push(candidate);
            }
        }

        tracing::debug!(
            starting_paths = paths.len(),
            extra_auxiliary_paths = paths.len().saturating_sub(1),
            configured_extra_auxiliary_paths = extra_paths,
            "Collected LinearGraph starting paths"
        );

        paths
    }

    /// Selects a refinement action based on the given spurious path.
    fn select_refinement_action(&self, path: &MultiGraphPath) -> VASSReachRefinementAction {
        // return VASSReachRefinementAction::BuildAutomaton;

        let path_final_valuation = path.get_path_final_valuation(&self.state.initial_valuation);

        tracing::debug!("Path final valuation: {:?}", path_final_valuation);

        // we find a counter that turns negative
        if *self.config.get_bounded_counting_enabled()
            && let Some((counter, path_index)) =
                path.find_negative_counter_forward(&self.state.initial_valuation)
            && !path.is_counter_forwards_pumped(
                self.state.dimension,
                counter,
                3,
                *self.config.get_consider_modulo_for_pumping(),
            )
        {
            let segment = path.slice(0..path_index);
            // if the path before wasn't pumped, we increase the bound we count up to, to
            // cover this path

            let current_bound = self.state.get_forward_bound(counter);
            let max_value = segment.max_counter_value(&self.state.initial_valuation, counter);
            let max_value = i32::max(1, max_value) as u32;

            if current_bound < max_value {
                return VASSReachRefinementAction::IncreaseForwardsBound(counter, max_value);
            }
        }

        // same as above, but from the back of the path
        if *self.config.get_bounded_counting_enabled()
            && let Some((counter, path_index)) =
                path.find_negative_counter_backward(&self.state.final_valuation)
            && !path.is_counter_backwards_pumped(
                self.state.dimension,
                counter,
                3,
                *self.config.get_consider_modulo_for_pumping(),
            )
        {
            let segment = path.slice(path_index..path.len());

            let current_bound = self.state.get_backward_bound(counter);
            let max_value =
                segment.max_counter_value_from_back(&self.state.initial_valuation, counter);
            let max_value = i32::max(1, max_value) as u32;
            if current_bound < max_value {
                return VASSReachRefinementAction::IncreaseBackwardsBound(counter, max_value);
            }
        }

        if let Some((mismatch, difference)) =
            path_final_valuation.find_mismatch(&self.state.final_valuation)
        {
            let max_value = path.max_counter_value(&self.state.initial_valuation, mismatch);
            let current_mu = self.state.get_mu(mismatch);

            // First we want the max value to be a lot bigger than mu. This way we don't
            // increase mu when we stay bounded. Second we want the difference
            // between the expected and actual final value to be quite small.
            //
            // TODO: Maybe we want to make sure that mu always stays below our counting
            // bounds. When we would increase mu, but it would exceed the bound, we increase
            // the bound instead.
            if max_value > current_mu * current_mu && difference.abs() <= current_mu * 2 {
                return VASSReachRefinementAction::IncreaseModulo(
                    mismatch,
                    path_final_valuation[mismatch],
                );
            }
        }

        VASSReachRefinementAction::BuildAutomaton
    }

    /// Builds and checks the LTC automaton for the given path.
    #[allow(dead_code)]
    fn ltc(&self, path: &MultiGraphPath) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
        let translation = LTCTranslation::from_multi_graph_path(&self.state, path);
        let ltc = translation.to_ltc(self.state.dimension);

        if *self.config.get_lts().get_relaxed_enabled() {
            self.ltc_relaxed(ltc, translation)
        } else {
            self.ltc_strict(ltc, translation)
        }
    }

    #[allow(dead_code)]
    fn ltc_relaxed(
        &self,
        ltc: LTC,
        translation: LTCTranslation<NodeIndex>,
    ) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
        let result_relaxed =
            ltc.reach_n_relaxed(&self.state.initial_valuation, &self.state.final_valuation);

        if result_relaxed.is_success() {
            tracing::debug!("LTC is relaxed reachable");

            self.ltc_strict(ltc, translation)
        } else {
            tracing::debug!("LTC is not relaxed reachable");

            Ok(translation.to_dfa(self.state.dimension, true))
        }
    }

    #[allow(dead_code)]
    fn ltc_strict(
        &self,
        ltc: LTC,
        translation: LTCTranslation<NodeIndex>,
    ) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
        let result_strict = ltc.reach_n(&self.state.initial_valuation, &self.state.final_valuation);

        if result_strict.is_success() {
            tracing::debug!("LTC is N-reachable");

            Err(VASSReachSolverStatus::True(()))
        } else {
            tracing::debug!("LTC is not N-reachable");

            Ok(translation.to_dfa(self.state.dimension, false))
        }
    }

    /// Checks if the maximum number of iterations has been reached.
    /// If so, returns an `Err` value.
    fn max_iterations_reached(&self) -> Result<(), VASSReachSolverStatus> {
        if self
            .config
            .get_max_iterations()
            .map(|x| x <= self.step_count)
            .unwrap_or(false)
        {
            return Err(SolverStatus::Unknown(
                VASSReachSolverError::MaxIterationsReached,
            ));
        }

        Ok(())
    }

    /// Checks if the time limit has been reached.
    /// If so, returns an `Err` value.
    fn max_time_reached(&self) -> Result<(), VASSReachSolverStatus> {
        if let Some(t) = self.get_solver_time()
            && let Some(max_time) = self.config.get_timeout()
            && &t > max_time
        {
            return Err(SolverStatus::Unknown(VASSReachSolverError::Timeout));
        }

        Ok(())
    }

    fn get_solver_statistics(&self) -> VASSReachSolverStatistics {
        VASSReachSolverStatistics::new(
            self.step_count,
            self.state.mu.clone(),
            self.state.get_forward_bounds(),
            self.state.get_backward_bounds(),
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn get_solver_time(&self) -> Option<std::time::Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }

    fn remaining_solver_time(&self) -> Option<std::time::Duration> {
        self.config
            .get_timeout()
            .map(|timeout| timeout.saturating_sub(self.get_solver_time().unwrap_or_default()))
    }

    fn print_start_banner(&self) {
        tracing::info!(
            dimension = %self.state.dimension,
            cfg_states = %self.state.main_cfg().node_count(),
            cfg_transitions = %self.state.main_cfg().graph.edge_count(),
            "Solver Info"
        );
    }

    fn print_end_banner(&self, result: &VASSReachSolverResult) {
        tracing::info!(
            result = ?result.status,
            mu = ?result.statistics.mu,
            forwards_bound = ?result.statistics.forwards_bound,
            backwards_bound = ?result.statistics.backwards_bound,
            step_count = %result.statistics.step_count,
            time = ?result.statistics.time,
            "Result"
        );
    }
}

fn log_scc_dag_route_summary_before_linear_graph<NIndex: GIndex>(
    product: &'static str,
    dag: &SCCDag<NIndex, CFGCounterUpdate>,
) {
    let SCCDagRouteSummary {
        components,
        edges,
        accepting_components,
        accepting_states,
        accepting_component_routes,
        accepting_state_routes,
    } = dag.accepting_route_summary();

    tracing::info!(
        product,
        scc_components = components,
        scc_edges = edges,
        accepting_components,
        accepting_states,
        accepting_component_routes,
        accepting_state_routes,
        "SCC-DAG accepting routes before LinearGraph refinement"
    );
}
