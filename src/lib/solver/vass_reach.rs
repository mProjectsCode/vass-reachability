use std::sync::{atomic::AtomicBool, Arc};

use petgraph::graph::EdgeIndex;

use crate::{
    automaton::{
        cfg::CFGCounterUpdate,
        dfa::VASSCFG,
        ltc::LTCTranslation,
        path::{Path, PathNReaching},
        vass::InitializedVASS,
        AutomatonEdge, AutomatonNode,
    },
    logger::{LogLevel, Logger},
};

use super::vass_z_reach::solve_z_reach_for_cfg;

#[derive(Debug)]
pub struct VASSReachSolver<N: AutomatonNode, E: AutomatonEdge> {
    options: VASSReachSolverOptions,
    logger: Logger,
    ivass: InitializedVASS<N, E>,
    // thread_pool: ThreadPool<LTCSolverResult>,
    cfg: VASSCFG<Vec<Option<N>>>,
    mu: u32,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<N: AutomatonNode, E: AutomatonEdge> VASSReachSolver<N, E> {
    pub fn new(options: VASSReachSolverOptions, ivass: InitializedVASS<N, E>) -> Self {
        let logger = Logger::new(
            options.log_level.clone(),
            "VASS Reach Solver".to_string(),
            options.log_file.clone(),
        );

        // let thread_pool = ThreadPool::<LTCSolverResult>::new(options.thread_pool_size);

        let mut cfg = ivass.to_cfg();
        cfg.add_failure_state(vec![]);
        cfg = cfg.minimize();

        let stop_signal = Arc::new(AtomicBool::new(false));
        if let Some(max_time) = options.max_time {
            let stop_signal = stop_signal.clone();
            std::thread::spawn(move || {
                std::thread::sleep(max_time);
                stop_signal.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        VASSReachSolver {
            options,
            logger,
            ivass,
            // thread_pool,
            cfg,
            mu: 2,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
        }
    }

    pub fn solve_n(&mut self) -> VASSReachSolverStatistics {
        // IDEA: on paths, for each node, try to find loops back to that node and include them in the ltc check.
        // this makes the ltc check more powerful and can cut away more paths.

        // IDEA: check for n-reach on each counter individually, treating other counter updates as empty transitions.
        // if a counter is not n-reachable, the entire thing can't be n-reachable either.

        let dimension = self.ivass.dimension();

        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();
        self.logger.empty(LogLevel::Info);

        self.logger.info("Checking Z-Reachability");
        let z_reach_time = std::time::Instant::now();

        let z_reach_result = solve_z_reach_for_cfg(
            &self.cfg,
            &self.ivass.initial_valuation,
            &self.ivass.final_valuation,
            Some(&self.logger),
        );
        if z_reach_result.is_failure() {
            self.logger.debug("CFG is not Z-Reachable");
            return VASSReachSolverStatistics::new(
                false,
                self.step_count,
                self.mu,
                self.get_solver_time().unwrap_or_default(),
            );
        }

        self.logger.info(&format!(
            "Checked Z-Reachability in: {:?}",
            z_reach_time.elapsed()
        ));

        let result;
        let mut step_time;

        loop {
            self.step_count += 1;
            step_time = std::time::Instant::now();

            self.logger
                .object("Step Info")
                .add_field("step", &self.step_count.to_string())
                .add_field("mu", &self.mu.to_string())
                .add_field("cfg.states", &self.cfg.state_count().to_string())
                .add_field("cfg.transitions", &self.cfg.graph.edge_count().to_string())
                .log(LogLevel::Info);

            if self.max_iterations_reached() {
                return self.max_iterations_reached_error();
            }

            if self.max_mu_reached() {
                return self.max_mu_reached_error();
            }

            if self.max_time_reached() {
                return self.max_time_reached_error();
            }

            // if self.handle_thread_pool() {
            //     result = true;
            //     break;
            // }

            let reach_path = self.run_modulo_bfs();

            if reach_path.is_none() {
                self.logger.debug("No path found");

                result = false;
                break;
            }

            let path = reach_path.unwrap();
            let (reaching, counters) = path.is_n_reaching(
                &self.ivass.initial_valuation,
                &self.ivass.final_valuation,
                |x| *self.cfg.edge_weight(x),
            );

            if reaching == PathNReaching::True {
                self.logger.debug(&format!(
                    "Reaching: {:?}",
                    path.simple_print(|x| self.get_cfg_edge_weight(x))
                ));
                result = true;
                break;
            } else {
                self.logger.debug(&format!(
                    "Not reaching ({:?}): {:?}",
                    counters,
                    path.simple_print(|x| self.get_cfg_edge_weight(x))
                ));
                path.to_parikh_image().print(&self.logger, LogLevel::Debug);

                // TODO: We should probably do the negative check first, cut the path,
                // then handle cut paths with and without loops separately.
                // If the path stays positive, we do the normal LTC or modulo increase.

                if let PathNReaching::Negative(index) = reaching {
                    self.logger
                        .debug(&format!("Path does not stay positive at index {:?}", index));

                    let cut_path = path.slice(index);

                    self.logger.debug(&format!(
                        "Cut path {:?}",
                        cut_path.simple_print(|x| self.get_cfg_edge_weight(x))
                    ));

                    let cut_path_loop = cut_path.has_loop();
                    if cut_path_loop {
                        self.logger.warn("Cut path has loop");
                    }

                    let dfa =
                        cut_path.simple_to_dfa(true, dimension, |x| self.get_cfg_edge_weight(x));

                    self.intersect_cfg(dfa);
                } else {
                    self.logger.debug("Building and checking LTC");

                    let mut ltc_translation = LTCTranslation::from_path(&path);
                    // self.logger.debug(&format!("LTC: {:#?}", ltc_translation));
                    ltc_translation = ltc_translation.expand(&self.cfg);
                    // self.logger.debug(&format!("LTC: {:#?}", ltc_translation));
                    let ltc = ltc_translation.to_ltc(dimension, |x| self.get_cfg_edge_weight(x));

                    let initial_v = &self.ivass.initial_valuation;
                    let final_v = &self.ivass.final_valuation;

                    // self.thread_pool
                    //     .schedule(move || ltc.reach_n(&initial_v, &final_v));

                    let result_relaxed = ltc.reach_n_relaxed(initial_v, final_v);

                    if result_relaxed.is_success() {
                        self.logger.debug("LTC is relaxed N-Reachable");

                        let result_strict = ltc.reach_n(initial_v, final_v);

                        if result_strict.is_success() {
                            self.logger.debug("LTC is N-Reachable");
                            result = true;
                            break;
                        } else {
                            self.logger.debug("LTC is not N-Reachable");

                            let dfa = ltc_translation
                                .to_dfa(false, dimension, |x| self.get_cfg_edge_weight(x));

                            self.intersect_cfg(dfa);
                        }
                    } else {
                        self.logger.debug("LTC is not N-Reachable");

                        let dfa = ltc_translation
                            .to_dfa(true, dimension, |x| self.get_cfg_edge_weight(x));

                        self.intersect_cfg(dfa);
                    }

                    // let dfa = ltc_translation.to_dfa(dimension, |x| self.get_cfg_edge_weight(x));

                    // self.intersect_cfg(dfa);

                    if path.len() > (self.mu as usize) * dimension {
                        self.logger.debug("Path too long, increasing mu");
                        self.increment_mu();
                    }
                }
            }

            self.logger
                .debug(&format!("Step time: {:?}", step_time.elapsed()));
            self.logger.empty(LogLevel::Info);
        }

        self.logger
            .debug(&format!("Step time: {:?}", step_time.elapsed()));
        self.logger.empty(LogLevel::Info);

        // self.logger.info("Joining thread pool");

        // if result {
        //     self.thread_pool.join(false);

        //     self.logger.debug(&format!(
        //         "Canceled jobs: {:?}",
        //         self.thread_pool.get_active_jobs()
        //     ));
        // } else {
        //     self.logger.debug(&format!(
        //         "Waiting on {:?} active jobs",
        //         self.thread_pool.get_active_jobs()
        //     ));

        //     self.thread_pool.join(true);

        //     assert_eq!(self.thread_pool.get_active_jobs(), 0);

        //     for solver_result in self.thread_pool.get_finished_jobs() {
        //         if solver_result.result {
        //             self.logger.debug("A thread found a solution");
        //             result = true;
        //             break;
        //         }
        //     }
        // }

        self.logger.empty(LogLevel::Info);

        let statistics = VASSReachSolverStatistics::new(
            result,
            self.step_count,
            self.mu,
            self.get_solver_time().unwrap_or_default(),
        );

        self.print_end_banner(&statistics);

        statistics
    }

    // fn solve_z(&self) -> bool {
    //     self.logger.debug("Solving VASS for Z-Reach");

    //     let result = solve_z_reach(&self.ivass, &self.logger);

    //     self.logger.debug(&format!(
    //         "Solved Z-Reach in {:?} with result: {:?}",
    //         result.duration, result.status
    //     ));

    //     result.is_success()
    // }

    fn max_mu_reached(&self) -> bool {
        self.options.max_mu.map(|x| x <= self.mu).unwrap_or(false)
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

    fn max_mu_reached_error(&self) -> VASSReachSolverStatistics {
        VASSReachSolverStatistics::from_error(
            VASSReachSolverError::MaxMuReached,
            self.step_count,
            self.mu,
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn max_iterations_reached_error(&self) -> VASSReachSolverStatistics {
        VASSReachSolverStatistics::from_error(
            VASSReachSolverError::MaxIterationsReached,
            self.step_count,
            self.mu,
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn max_time_reached_error(&self) -> VASSReachSolverStatistics {
        VASSReachSolverStatistics::from_error(
            VASSReachSolverError::Timeout,
            self.step_count,
            self.mu,
            self.get_solver_time().unwrap_or_default(),
        )
    }

    fn get_solver_time(&self) -> Option<std::time::Duration> {
        self.solver_start_time.map(|x| x.elapsed())
    }

    // fn handle_thread_pool(&self) -> bool {
    //     if self.thread_pool.get_active_jobs() >= self.options.thread_pool_size * 4 {
    //         self.logger.info(&format!(
    //             "Waiting on thread pool to empty. Active jobs: {:?}, Target: {:?}",
    //             self.thread_pool.get_active_jobs(),
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

    fn run_modulo_bfs(&self) -> Option<Path> {
        self.cfg.modulo_reach(
            self.mu,
            &self.ivass.initial_valuation,
            &self.ivass.final_valuation,
        )
    }

    fn get_cfg_edge_weight(&self, edge: EdgeIndex<u32>) -> CFGCounterUpdate {
        *self.cfg.edge_weight(edge)
    }

    fn intersect_cfg(&mut self, dfa: VASSCFG<()>) {
        self.cfg = self.cfg.intersect(&dfa);
        self.cfg = self.cfg.minimize();
    }

    fn increment_mu(&mut self) {
        self.mu += 1;
    }

    fn print_start_banner(&self) {
        self.logger
            .object("Solver Info")
            .add_field("dimension", &self.ivass.dimension().to_string())
            .add_field("vass.states", &self.ivass.vass.state_count().to_string())
            .add_field(
                "vass.transitions",
                &self.ivass.vass.transition_count().to_string(),
            )
            .add_field("cfg.states", &self.cfg.state_count().to_string())
            .add_field("cfg.transitions", &self.cfg.graph.edge_count().to_string())
            .log(LogLevel::Info);
    }

    fn print_end_banner(&self, statistics: &VASSReachSolverStatistics) {
        self.logger
            .object("Result")
            .add_field("result", &format!("{:?}", statistics.result))
            .add_field("max mu", &statistics.mu.to_string())
            .add_field("step count", &statistics.iterations.to_string())
            .add_field("time", &format!("{:?}", statistics.time))
            .log(LogLevel::Info);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VASSReachSolverOptions {
    log_level: LogLevel,
    thread_pool_size: usize,
    max_iterations: Option<u32>,
    max_mu: Option<u32>,
    max_time: Option<std::time::Duration>,
    log_file: Option<String>,
}

impl VASSReachSolverOptions {
    pub fn new(
        log_level: LogLevel,
        thread_pool_size: usize,
        max_iterations: Option<u32>,
        max_mu: Option<u32>,
        max_time: Option<std::time::Duration>,
        log_file: Option<String>,
    ) -> Self {
        VASSReachSolverOptions {
            log_level,
            thread_pool_size,
            max_iterations,
            max_mu,
            max_time,
            log_file,
        }
    }

    pub fn default_mu_limited() -> Self {
        VASSReachSolverOptions::default().with_mu_limit(100)
    }

    pub fn with_mu_limit(mut self, mu: u32) -> Self {
        self.max_mu = Some(mu);
        self
    }

    pub fn with_iteration_limit(mut self, iterations: u32) -> Self {
        self.max_iterations = Some(iterations);
        self
    }

    pub fn with_time_limit(mut self, time: std::time::Duration) -> Self {
        self.max_time = Some(time);
        self
    }

    pub fn with_log_level(mut self, level: LogLevel) -> Self {
        self.log_level = level;
        self
    }

    pub fn with_thread_pool_size(mut self, size: usize) -> Self {
        self.thread_pool_size = size;
        self
    }

    pub fn with_log_file(mut self, file: &str) -> Self {
        self.log_file = Some(file.to_string());
        self
    }

    pub fn to_solver<N: AutomatonNode, E: AutomatonEdge>(
        self,
        ivass: InitializedVASS<N, E>,
    ) -> VASSReachSolver<N, E> {
        VASSReachSolver::new(self, ivass)
    }
}

impl Default for VASSReachSolverOptions {
    fn default() -> Self {
        VASSReachSolverOptions {
            log_level: LogLevel::Info,
            thread_pool_size: 4,
            max_iterations: None,
            max_mu: None,
            max_time: None,
            log_file: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VASSReachSolverResult {
    Reachable,
    Unreachable,
    Unknown(VASSReachSolverError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VASSReachSolverError {
    Timeout,
    MaxIterationsReached,
    MaxMuReached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VASSReachSolverStatistics {
    pub result: VASSReachSolverResult,
    pub iterations: u32,
    pub mu: u32,
    pub time: std::time::Duration,
}

impl VASSReachSolverStatistics {
    pub fn new(result: bool, iterations: u32, mu: u32, time: std::time::Duration) -> Self {
        VASSReachSolverStatistics {
            result: match result {
                true => VASSReachSolverResult::Reachable,
                false => VASSReachSolverResult::Unreachable,
            },
            iterations,
            mu,
            time,
        }
    }

    pub fn from_error(
        error: VASSReachSolverError,
        iterations: u32,
        mu: u32,
        time: std::time::Duration,
    ) -> Self {
        VASSReachSolverStatistics {
            result: VASSReachSolverResult::Unknown(error),
            iterations,
            mu,
            time,
        }
    }

    pub fn reachable(&self) -> bool {
        matches!(self.result, VASSReachSolverResult::Reachable)
    }

    pub fn unreachable(&self) -> bool {
        matches!(self.result, VASSReachSolverResult::Unreachable)
    }

    pub fn unknown(&self) -> bool {
        matches!(self.result, VASSReachSolverResult::Unknown(_))
    }
}
