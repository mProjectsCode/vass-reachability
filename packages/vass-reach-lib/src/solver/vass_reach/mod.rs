use petgraph::graph::NodeIndex;
use serde::{Deserialize, Serialize};

use crate::{
    automaton::{
        Automaton, AutomatonEdge, AutomatonNode, FromLetter,
        algorithms::EdgeAutomatonAlgorithms,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::minimization::Minimizable,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        lsg::extender::{ExtensionStrategyEnum, LSGExtender},
        ltc::{LTC, translation::LTCTranslation},
        path::Path,
        vass::{counter::VASSCounterIndex, initialized::InitializedVASS},
    },
    config::{ModuloMode, VASSReachConfig},
    solver::{SolverResult, SolverStatus},
};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

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
    /// Build some automaton (LTC, LSG, ...?) to cut away the spurious path.
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

#[derive(Debug)]
pub struct VASSReachSolver {
    config: VASSReachConfig,
    state: ImplicitCFGProduct,
    step_count: u64,
    solver_start_time: Option<std::time::Instant>,
}

impl VASSReachSolver {
    pub fn new<N: AutomatonNode, E: AutomatonEdge + FromLetter>(
        ivass: &InitializedVASS<N, E>,
        config: VASSReachConfig,
    ) -> Self {
        let mut cfg = ivass.to_cfg();
        cfg.make_complete(());
        cfg = cfg.minimize();

        tracing::debug!("{}", cfg.to_graphviz(None, None));

        let state = ImplicitCFGProduct::new(
            ivass.dimension(),
            ivass.initial_valuation.clone(),
            ivass.final_valuation.clone(),
            cfg,
        );

        VASSReachSolver {
            config,
            state,
            step_count: 0,
            solver_start_time: None,
        }
    }

    pub fn solve(&mut self) -> VASSReachSolverResult {
        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();

        let status = self
            .solve_inner()
            .expect_err("expected solve_inner to return the result as an Err value");
        let result = VASSReachSolverResult::new(status, self.get_solver_statistics());
        self.print_end_banner(&result);

        result
    }

    fn solve_inner(&mut self) -> Result<(), VASSReachSolverStatus> {
        let mut step_time;

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

            // We check if we by change found a real N-reaching path
            if path.is_n_reaching(&self.state.initial_valuation, &self.state.final_valuation) {
                tracing::info!("Found N-reaching path: {:?}", path.to_fancy_string());

                return Err(SolverStatus::True(()));
            }

            tracing::debug!("Spurious path: {:?}", path.to_fancy_string());

            if true {
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
                let ltc_automaton = if *self.config.get_lts().get_enabled() {
                    tracing::debug!("Building and checking LTC");

                    Some(self.ltc(&path)?)
                } else {
                    None
                };

                let lsg_automaton = if *self.config.get_lsg().get_enabled() {
                    tracing::debug!("Building and checking LSG");

                    let mut extender = LSGExtender::from_cfg_product(
                        path,
                        &self.state,
                        ExtensionStrategyEnum::from_config(
                            *self.config.get_lsg().get_strategy(),
                            self.step_count,
                        ),
                        *self.config.get_lsg().get_max_refinement_steps(),
                    );
                    let mut cfg = extender.run();
                    cfg.invert_mut();
                    Some(cfg)
                } else {
                    None
                };

                match (ltc_automaton, lsg_automaton) {
                    (Some(ltc_cfg), Some(lsg_cfg)) => {
                        // We would expect both automata to be somewhat similar, they are built
                        // from the same path at least. So we would
                        // expect their intersection to not blow up too much.
                        let product = ltc_cfg.intersect(&lsg_cfg);
                        self.state.add_cfg(product);
                    }
                    (Some(ltc_cfg), None) => {
                        self.state.add_cfg(ltc_cfg);
                    }
                    (None, Some(lsg_cfg)) => {
                        self.state.add_cfg(lsg_cfg);
                    }
                    (None, None) => {}
                }
            }
        }

        Ok(())
    }

    /// Selects a refinement action based on the given spurious path.
    fn select_refinement_action(&self, path: &MultiGraphPath) -> VASSReachRefinementAction {
        let path_final_valuation = path.get_path_final_valuation(&self.state.initial_valuation);

        tracing::debug!("Path final valuation: {:?}", path_final_valuation);

        // we find a counter that turns negative
        if let Some((counter, path_index)) =
            path.find_negative_counter_forward(&self.state.initial_valuation)
            && !path.is_counter_forwards_pumped(self.state.dimension, counter, 3)
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
        if let Some((counter, path_index)) =
            path.find_negative_counter_backward(&self.state.final_valuation)
            && !path.is_counter_backwards_pumped(self.state.dimension, counter, 3)
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
    fn ltc(&self, path: &MultiGraphPath) -> Result<VASSCFG<()>, VASSReachSolverStatus> {
        let translation = LTCTranslation::from_multi_graph_path(&self.state, path);
        let ltc = translation.to_ltc(self.state.dimension);

        if *self.config.get_lts().get_relaxed_enabled() {
            self.ltc_relaxed(ltc, translation)
        } else {
            self.ltc_strict(ltc, translation)
        }
    }

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
