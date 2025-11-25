use num::Integer;
use serde::{Deserialize, Serialize};

use crate::{
    automaton::{
        AutomatonEdge, AutomatonNode,
        dfa::minimization::Minimizable,
        implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath},
        lsg::extender::{LSGExtender, RandomNodeChooser},
        ltc::{LTC, translation::LTCTranslation},
        path::{Path, PathNReaching},
        vass::{counter::VASSCounterIndex, initialized::InitializedVASS},
    },
    config::{ModuloMode, VASSReachConfig},
    logger::{LogLevel, Logger},
    solver::{SolverResult, SolverStatus},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VASSReachSolverError {
    Timeout,
    MaxIterationsReached,
    MaxMuReached,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VASSReachSolverStatistics {
    pub step_count: u64,
    pub mu: Box<[i32]>,
    pub limit: Box<[u32]>,
    pub time: std::time::Duration,
}

impl VASSReachSolverStatistics {
    pub fn new(
        step_count: u64,
        mu: Box<[i32]>,
        limit: Box<[u32]>,
        time: std::time::Duration,
    ) -> Self {
        VASSReachSolverStatistics {
            step_count,
            mu,
            limit,
            time,
        }
    }
}

pub type VASSReachSolverStatus = SolverStatus<(), (), VASSReachSolverError>;

pub type VASSReachSolverResult =
    SolverResult<(), (), VASSReachSolverError, VASSReachSolverStatistics>;

#[derive(Debug)]
pub struct VASSReachSolver<'l> {
    options: VASSReachConfig,
    logger: Option<&'l Logger>,
    state: ImplicitCFGProduct,
    step_count: u64,
    solver_start_time: Option<std::time::Instant>,
}

impl<'l> VASSReachSolver<'l> {
    pub fn new<N: AutomatonNode, E: AutomatonEdge>(
        ivass: &InitializedVASS<N, E>,
        config: VASSReachConfig,
        logger: Option<&'l Logger>,
    ) -> Self {
        let mut cfg = ivass.to_cfg();
        cfg.add_failure_state(());
        cfg = cfg.minimize();

        if let Some(l) = logger {
            l.debug(&cfg.to_graphviz(None as Option<Path>));
        }

        let state = ImplicitCFGProduct::new(
            ivass.dimension(),
            ivass.initial_valuation.clone(),
            ivass.final_valuation.clone(),
            cfg,
        );

        VASSReachSolver {
            options: config,
            logger,
            state,
            step_count: 0,
            solver_start_time: None,
        }
    }

    pub fn solve(&mut self) -> VASSReachSolverResult {
        // IDEA: on paths, for each node, try to find loops back to that node and
        // include them in the ltc check. this makes the ltc check more powerful
        // and can cut away more paths.

        // IDEA: check for n-reach on each counter individually, treating other counter
        // updates as empty transitions. if a counter is not n-reachable, the
        // entire thing can't be n-reachable either.

        // IDEA: if negative, cut away with automaton that counts until highest value
        // reached on that counter

        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();
        if let Some(l) = self.logger {
            l.empty(LogLevel::Info);
        }

        // let z_reachable = self.check_z_reach();
        // if z_reachable.is_failure() {
        //     return self.get_solver_result(false);
        // }

        let result;
        let mut step_time;

        loop {
            self.step_count += 1;
            step_time = std::time::Instant::now();

            if let Some(l) = self.logger {
                l.object("Step Info")
                    .add_field("step", &self.step_count.to_string())
                    .add_field("mu", &format!("{:?}", self.state.mu))
                    .add_field("limit", &format!("{:?}", self.state.limit_values()))
                    .add_field("intersection size", &self.state.other_cfg.len().to_string())
                    .log(LogLevel::Info);
            }

            if self.max_iterations_reached() {
                return self.max_iterations_reached_result();
            }

            if self.max_time_reached() {
                return self.max_time_reached_result();
            }

            let reach_path = self.state.reach();

            // if let Some(l) = self.logger {
            //     l.debug(&self.cfg.to_graphviz(None as Option<Path>));
            // }

            if reach_path.is_none() {
                if let Some(l) = self.logger {
                    l.debug("No path found");
                }

                result = false;
                break;
            }

            let path = reach_path.unwrap();
            let (reaching, counters) =
                path.is_n_reaching(&self.state.initial_valuation, &self.state.final_valuation);

            // we found a path that is n-reachable => we are done
            if reaching == PathNReaching::True {
                if let Some(l) = self.logger {
                    l.debug(&format!("Reaching: {:?}", path.to_fancy_string()));
                }
                result = true;
                break;
            }

            if let Some(l) = self.logger {
                l.debug(&format!(
                    "Not reaching ({:?}): {:?}",
                    counters,
                    path.to_fancy_string()
                ));
            }

            // ---
            // Bounded counting separator
            // ---

            if let PathNReaching::Negative((index, counter)) = reaching {
                if let Some(l) = self.logger {
                    l.debug(&format!("Path does not stay positive at index {:?}", index));
                }

                let max_counter_value =
                    path.max_counter_value(&self.state.initial_valuation, counter);

                self.state.set_limit(counter, max_counter_value);
            }

            // ---
            // LTC
            // ---

            if *self.options.get_lts().get_enabled() {
                if let Some(l) = self.logger {
                    l.debug("Building and checking LTC");
                }

                if let Some(r) = self.ltc(&path) {
                    result = r;
                    break;
                }
            }

            // ---
            // LSG
            // ---

            if *self.options.get_lsg().get_enabled() {
                if let Some(l) = self.logger {
                    l.debug("Building and checking LSG");
                }

                let mut extender = LSGExtender::from_cfg_product(
                    &path,
                    &self.state,
                    RandomNodeChooser::new(5, self.step_count as u64),
                    *self.options.get_lsg().get_max_refinement_steps(),
                );
                let mut cfg = extender.run();
                cfg.invert_mut();
                self.state.add_cfg(cfg);
            }

            // ---
            // mu
            // ---

            for i in VASSCounterIndex::iter_counters(self.state.dimension) {
                let max_value = path.max_counter_value(&self.state.initial_valuation, i);
                let mu = self.state.get_mu(i) as u32;

                if max_value > 2 * mu {
                    let new_mu = match self.options.get_modulo().get_mode() {
                        ModuloMode::Increment => mu + 1,
                        ModuloMode::LeastCommonMultiple => mu.lcm(&max_value),
                    };

                    if let Some(l) = self.logger {
                        l.debug(&format!(
                            "Counter {:?} max value {:?} is more than double mu {:?}, increasing mu to {:?}",
                            i, max_value, mu, new_mu
                        ));
                    }

                    self.state.set_mu(i, new_mu as i32);
                }
            }

            if let Some(l) = self.logger {
                l.debug(&format!("Step time: {:?}", step_time.elapsed()));
                l.empty(LogLevel::Info);
            }
        }

        if let Some(l) = self.logger {
            l.debug(&format!("Step time: {:?}", step_time.elapsed()));
            l.empty(LogLevel::Info);
        }

        if let Some(l) = self.logger {
            l.empty(LogLevel::Info);
        }

        let statistics = self.get_solver_result(result);

        self.print_end_banner(&statistics);

        statistics
    }

    fn ltc(&mut self, path: &MultiGraphPath) -> Option<bool> {
        let translation = LTCTranslation::from_multi_graph_path(&self.state, &path);
        let ltc = translation.to_ltc(&self.state.cfg, self.state.dimension);

        if *self.options.get_lts().get_relaxed_enabled() {
            self.ltc_relaxed(ltc, translation)
        } else {
            self.ltc_strict(ltc, translation)
        }
    }

    fn ltc_relaxed(&mut self, ltc: LTC, translation: LTCTranslation) -> Option<bool> {
        let result_relaxed =
            ltc.reach_n_relaxed(&self.state.initial_valuation, &self.state.final_valuation);

        if result_relaxed.is_success() {
            if let Some(l) = self.logger {
                l.debug("LTC is relaxed reachable");
            }

            self.ltc_strict(ltc, translation)
        } else {
            if let Some(l) = self.logger {
                l.debug("LTC is not relaxed reachable");
            }

            self.state
                .add_cfg(translation.to_dfa(&self.state.cfg, self.state.dimension, true));

            None
        }
    }

    fn ltc_strict(&mut self, ltc: LTC, translation: LTCTranslation) -> Option<bool> {
        let result_strict = ltc.reach_n(&self.state.initial_valuation, &self.state.final_valuation);

        if result_strict.is_success() {
            if let Some(l) = self.logger {
                l.debug("LTC is N-reachable");
            }

            Some(true)
        } else {
            if let Some(l) = self.logger {
                l.debug("LTC is not N-reachable");
            }

            self.state
                .add_cfg(translation.to_dfa(&self.state.cfg, self.state.dimension, false));

            None
        }
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

    fn max_iterations_reached_result(&self) -> VASSReachSolverResult {
        VASSReachSolverResult::new(
            SolverStatus::Unknown(VASSReachSolverError::MaxIterationsReached),
            self.get_solver_statistics(),
        )
    }

    fn max_time_reached_result(&self) -> VASSReachSolverResult {
        VASSReachSolverResult::new(
            SolverStatus::Unknown(VASSReachSolverError::Timeout),
            self.get_solver_statistics(),
        )
    }

    fn get_solver_statistics(&self) -> VASSReachSolverStatistics {
        VASSReachSolverStatistics::new(
            self.step_count,
            self.state.mu.clone(),
            self.state.limit_values(),
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn get_solver_result(&self, result: bool) -> VASSReachSolverResult {
        VASSReachSolverResult::new(result.into(), self.get_solver_statistics())
    }

    fn get_solver_time(&self) -> Option<std::time::Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }

    fn print_start_banner(&self) {
        if let Some(l) = self.logger {
            l.object("Solver Info")
                .add_field("dimension", &self.state.dimension.to_string())
                .add_field("cfg.states", &self.state.cfg.state_count().to_string())
                .add_field(
                    "cfg.transitions",
                    &self.state.cfg.graph.edge_count().to_string(),
                )
                .log(LogLevel::Info);
        }
    }

    fn print_end_banner(&self, result: &VASSReachSolverResult) {
        if let Some(l) = self.logger {
            l.object("Result")
                .add_field("result", &format!("{:?}", result.status))
                .add_field("mu", &format!("{:?}", &result.statistics.mu))
                .add_field("limit", &format!("{:?}", &result.statistics.limit))
                .add_field("step count", &result.statistics.step_count.to_string())
                .add_field("time", &format!("{:?}", result.statistics.time))
                .log(LogLevel::Info);
        }
    }
}
