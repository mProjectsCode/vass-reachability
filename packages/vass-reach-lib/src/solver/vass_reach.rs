use std::sync::{Arc, atomic::AtomicBool};

use petgraph::graph::EdgeIndex;
use serde::{Deserialize, Serialize};

use crate::{
    automaton::{
        AutomatonEdge, AutomatonNode,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::minimization::Minimizable,
        implicit_cfg_product::ImplicitCFGProduct,
        lsg::extender::{LSGExtender, RandomNodeChooser},
        ltc::translation::LTCTranslation,
        path::{Path, PathNReaching},
        vass::{counter::VASSCounterIndex, initialized::InitializedVASS},
    },
    logger::{LogLevel, Logger},
    solver::{
        SolverResult, SolverStatus,
        vass_z_reach::{VASSZReachSolverOptions, VASSZReachSolverResult},
    },
    threading::thread_pool::ThreadPool,
};

#[derive(Clone, Debug)]
pub struct VASSReachSolverOptions<'a> {
    logger: Option<&'a Logger>,
    thread_pool_size: usize,
    max_iterations: Option<u32>,
    max_mu: Option<u32>,
    max_time: Option<std::time::Duration>,
}

impl<'a> VASSReachSolverOptions<'a> {
    pub fn new(
        logger: Option<&'a Logger>,
        thread_pool_size: usize,
        max_iterations: Option<u32>,
        max_mu: Option<u32>,
        max_time: Option<std::time::Duration>,
    ) -> Self {
        VASSReachSolverOptions {
            logger,
            thread_pool_size,
            max_iterations,
            max_mu,
            max_time,
        }
    }

    pub fn default_mu_limited() -> Self {
        VASSReachSolverOptions::default().with_mu_limit(100)
    }

    pub fn with_logger(mut self, logger: &'a Logger) -> Self {
        self.logger = Some(logger);
        self
    }

    pub fn with_mu_limit(mut self, mu: u32) -> Self {
        self.max_mu = Some(mu);
        self
    }

    pub fn with_optional_mu_limit(mut self, mu: Option<u32>) -> Self {
        self.max_mu = mu;
        self
    }

    pub fn with_iteration_limit(mut self, iterations: u32) -> Self {
        self.max_iterations = Some(iterations);
        self
    }

    pub fn with_optional_iteration_limit(mut self, iterations: Option<u32>) -> Self {
        self.max_iterations = iterations;
        self
    }

    pub fn with_time_limit(mut self, time: std::time::Duration) -> Self {
        self.max_time = Some(time);
        self
    }

    pub fn with_optional_time_limit(mut self, time: Option<std::time::Duration>) -> Self {
        self.max_time = time;
        self
    }

    pub fn with_thread_pool_size(mut self, size: usize) -> Self {
        self.thread_pool_size = size;
        self
    }

    pub fn to_vass_solver<N: AutomatonNode, E: AutomatonEdge>(
        self,
        ivass: &InitializedVASS<N, E>,
    ) -> VASSReachSolver<'a> {
        VASSReachSolver::new(self, ivass)
    }
}

impl Default for VASSReachSolverOptions<'_> {
    fn default() -> Self {
        VASSReachSolverOptions {
            logger: None,
            thread_pool_size: 4,
            max_iterations: None,
            max_mu: None,
            max_time: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum VASSReachSolverError {
    Timeout,
    MaxIterationsReached,
    MaxMuReached,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VASSReachSolverStatistics {
    pub step_count: u32,
    pub mu: Box<[i32]>,
    pub limit: Box<[u32]>,
    pub time: std::time::Duration,
}

impl VASSReachSolverStatistics {
    pub fn new(
        step_count: u32,
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
pub struct VASSReachSolver<'a> {
    options: VASSReachSolverOptions<'a>,
    logger: Option<&'a Logger>,
    state: ImplicitCFGProduct,
    thread_pool: ThreadPool<VASSZReachSolverResult>,
    z_reach_stop_signal: Arc<AtomicBool>,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<'a> VASSReachSolver<'a> {
    pub fn new<N: AutomatonNode, E: AutomatonEdge>(
        options: VASSReachSolverOptions<'a>,
        ivass: &InitializedVASS<N, E>,
    ) -> Self {
        let logger = options.logger;

        let thread_pool = ThreadPool::<VASSZReachSolverResult>::new(options.thread_pool_size);

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

        let stop_signal = Arc::new(AtomicBool::new(false));
        let z_reach_stop_signal = Arc::new(AtomicBool::new(false));

        VASSReachSolver {
            options,
            logger,
            state,
            thread_pool,
            z_reach_stop_signal,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
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

        self.start_watchdog();

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
                self.thread_pool.join(false);
                return self.max_iterations_reached_result();
            }

            if self.max_mu_reached() {
                self.thread_pool.join(false);
                return self.max_mu_reached_result();
            }

            if self.max_time_reached() {
                self.thread_pool.join(false);
                return self.max_time_reached_result();
            }

            // if self.handle_thread_pool() {
            //     result = true;
            //     break;
            // }

            // // every 100 iterations we cancel the current Z-Reach solver and wait for it
            // to // finish
            // if self.step_count.rem(100) == 0 && !self.thread_pool.is_idle() {
            //     self.z_reach_stop_signal
            //         .store(true, std::sync::atomic::Ordering::SeqCst);
            //     if let Some(l) = self.logger {
            //         l.info("Waiting on thread pool to finish Z-Reach checks");
            //     }
            //     self.thread_pool.block_until_no_active_jobs();
            // }

            // // every iteration we check if the Z-Reach solver is done
            // let finished_jobs = self.thread_pool.get_finished_jobs();
            // if finished_jobs.iter().any(|x| x.is_failure()) {
            //     if let Some(l) = self.logger {
            //         l.debug("A thread was not Z-Reachable");
            //     }
            //     result = false;
            //     break;
            // }

            // // every iteration we check if the Z-Reach solver is done
            // // and start a new one if it is
            // if self.thread_pool.is_idle() {
            //     let z_reach_stop_signal = self.z_reach_stop_signal.clone();
            //     z_reach_stop_signal.store(false, std::sync::atomic::Ordering::SeqCst);

            //     let cfg = self.cfg.clone();
            //     let initial_valuation = self.ivass.initial_valuation.clone();
            //     let final_valuation = self.ivass.final_valuation.clone();
            //     self.thread_pool.schedule(|| {
            //         VASSZReachSolverOptions::default()
            //             .with_time_limit(std::time::Duration::from_secs(10 * 60))
            //             .with_stop_signal(z_reach_stop_signal)
            //             .to_solver(cfg, initial_valuation, final_valuation)
            //             .solve()
            //     });
            // }

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

            if let Some(l) = self.logger {
                l.debug("Building and checking LTC");
            }

            let ltc_translation = LTCTranslation::from_multi_graph_path(&self.state, &path);
            // ltc_translation = ltc_translation.expand(&self.cfg);
            let ltc = ltc_translation.to_ltc(&self.state.cfg, self.state.dimension);

            let result_relaxed =
                ltc.reach_n_relaxed(&self.state.initial_valuation, &self.state.final_valuation);

            if result_relaxed.is_success() {
                if let Some(l) = self.logger {
                    l.debug("LTC is relaxed reachable");
                }

                let result_strict =
                    ltc.reach_n(&self.state.initial_valuation, &self.state.final_valuation);

                if result_strict.is_success() {
                    if let Some(l) = self.logger {
                        l.debug("LTC is N-reachable");
                    }
                    result = true;
                    break;
                } else {
                    if let Some(l) = self.logger {
                        l.debug("LTC is not N-reachable");
                    }

                    let dfa = ltc_translation.to_dfa(&self.state.cfg, self.state.dimension, false);

                    self.intersect_cfg(dfa);
                }
            } else {
                if let Some(l) = self.logger {
                    l.debug("LTC is not relaxed reachable");
                }

                let dfa = ltc_translation.to_dfa(&self.state.cfg, self.state.dimension, true);

                self.intersect_cfg(dfa);
            }

            // if path.len() > (self.mu as usize) * dimension {
            //     if let Some(l) = self.logger {
            //         l.debug("Path too long, increasing mu");
            //     }
            //     self.increment_mu();
            // }

            // ---
            // LSG
            // ---

            if let Some(l) = self.logger {
                l.debug("Building and checking LSG");
            }

            let mut extender = LSGExtender::from_cfg_product(
                &path,
                &self.state,
                RandomNodeChooser::new(5, self.step_count as u64),
                5,
            );
            self.intersect_cfg(extender.run());

            // ---
            // mu
            // ---

            for i in VASSCounterIndex::iter_counters(self.state.dimension) {
                let max_value = path.max_counter_value(&self.state.initial_valuation, i);
                let mu = self.state.get_mu(i) as u32;

                if max_value > 2 * mu {
                    if let Some(l) = self.logger {
                        l.debug(&format!(
                            "Counter {:?} max value {:?} is more than double mu {:?}, increasing mu",
                            i, max_value, mu
                        ));
                    }
                    self.increment_mu(i);
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

        self.thread_pool.join(false);

        if let Some(l) = self.logger {
            l.empty(LogLevel::Info);
        }

        let statistics = self.get_solver_result(result);

        self.print_end_banner(&statistics);

        statistics
    }

    fn start_watchdog(&self) {
        if let Some(max_time) = self.options.max_time {
            let stop_signal = self.stop_signal.clone();
            std::thread::spawn(move || {
                std::thread::sleep(max_time);
                stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }
    }

    fn max_mu_reached(&self) -> bool {
        self.options
            .max_mu
            .map(|x| self.state.mu.iter().any(|mu| *mu > x as i32))
            .unwrap_or(false)
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

    fn max_mu_reached_result(&self) -> VASSReachSolverResult {
        VASSReachSolverResult::new(
            SolverStatus::Unknown(VASSReachSolverError::MaxMuReached),
            self.get_solver_statistics(),
        )
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

    // fn handle_thread_pool(&self) -> bool {
    //     if self.thread_pool.get_active_jobs() >= self.options.thread_pool_size *
    // 4 {         self.logger.info(&format!(
    //             "Waiting on thread pool to empty. Active jobs: {:?}, Target:
    // {:?}",             self.thread_pool.get_active_jobs(),
    //             self.options.thread_pool_size
    //         ));
    //         self.thread_pool
    //             .block_until_x_active_jobs(self.options.thread_pool_size);
    //     }
    //     self.thread_pool.block_until_x_active_jobs_if_above_y(4, 10);

    //     let finished_jobs = self.thread_pool.get_finished_jobs();

    //     if finished_jobs.iter().any(|x| x.result) {
    //         self.logger.debug("A thread found a solution");
    //         return true;
    //     }

    //     false
    // }

    fn check_z_reach(&self) -> VASSZReachSolverResult {
        let result = VASSZReachSolverOptions::default()
            .with_time_limit(std::time::Duration::from_secs(10 * 60))
            .to_solver(
                self.state.cfg.clone(),
                self.state.initial_valuation.clone(),
                self.state.final_valuation.clone(),
            )
            .solve();

        if let Some(l) = self.logger {
            l.debug(&format!(
                "Checked Z-Reachability in {:?} and {:?} steps",
                result.statistics.time, result.statistics.step_count
            ));
            if result.is_failure() {
                l.debug("CFG is not Z-Reachable");
            }
            if result.is_unknown() {
                l.debug("CFG Z-Reachability Unknown");
            }
        }

        result
    }

    fn get_cfg_edge_weight(&self, edge: EdgeIndex<u32>) -> CFGCounterUpdate {
        *self.state.cfg.edge_weight(edge)
    }

    fn intersect_cfg(&mut self, other: VASSCFG<()>) {
        self.state.add_cfg(other);
    }

    fn increment_mu(&mut self, counter_index: VASSCounterIndex) {
        self.state.increment_mu(counter_index);
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
