use std::{
    ops::Rem,
    sync::{Arc, atomic::AtomicBool},
};

use petgraph::graph::EdgeIndex;

use super::{
    SolverResult, SolverStatus,
    vass_z_reach::{VASSZReachSolverOptions, VASSZReachSolverResult},
};
use crate::{
    automaton::{
        AutomatonEdge, AutomatonNode,
        dfa::cfg::{CFGCounterUpdate, VASSCFG},
        ltc::translation::LTCTranslation,
        path::{Path, PathNReaching},
        vass::initialized::InitializedVASS,
    },
    logger::{LogLevel, Logger},
};

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
pub enum VASSReachSolverError {
    Timeout,
    MaxIterationsReached,
    MaxMuReached,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VASSReachSolverStatistics {
    pub step_count: u32,
    pub mu: u32,
    pub time: std::time::Duration,
}

impl VASSReachSolverStatistics {
    pub fn new(step_count: u32, mu: u32, time: std::time::Duration) -> Self {
        VASSReachSolverStatistics {
            step_count,
            mu,
            time,
        }
    }
}

pub type VASSReachSolverStatus = SolverStatus<(), (), VASSReachSolverError>;

pub type VASSReachSolverResult =
    SolverResult<(), (), VASSReachSolverError, VASSReachSolverStatistics>;

#[derive(Debug)]
pub struct VASSReachSolver<N: AutomatonNode, E: AutomatonEdge> {
    options: VASSReachSolverOptions,
    logger: Logger,
    ivass: InitializedVASS<N, E>,
    // thread_pool: ThreadPool<LTCSolverResult>,
    cfg: VASSCFG<()>,
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

        // let thread_pool =
        // ThreadPool::<LTCSolverResult>::new(options.thread_pool_size);

        let mut cfg = ivass.to_cfg();
        cfg.add_failure_state(());
        cfg = cfg.minimize();

        let stop_signal = Arc::new(AtomicBool::new(false));

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

    pub fn solve(&mut self) -> VASSReachSolverResult {
        // IDEA: on paths, for each node, try to find loops back to that node and
        // include them in the ltc check. this makes the ltc check more powerful
        // and can cut away more paths.

        // IDEA: check for n-reach on each counter individually, treating other counter
        // updates as empty transitions. if a counter is not n-reachable, the
        // entire thing can't be n-reachable either.

        self.start_watchdog();

        let dimension = self.ivass.dimension();

        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();
        self.logger.empty(LogLevel::Info);

        let z_reachable = self.check_z_reach();
        if z_reachable.is_failure() {
            return self.get_solver_result(false);
        }

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
                return self.max_iterations_reached_result();
            }

            if self.max_mu_reached() {
                return self.max_mu_reached_result();
            }

            if self.max_time_reached() {
                return self.max_time_reached_result();
            }

            // if self.handle_thread_pool() {
            //     result = true;
            //     break;
            // }

            if self.step_count.rem(10) == 0 {
                let z_reachable = self.check_z_reach();
                if z_reachable.is_failure() {
                    result = false;
                    break;
                }
            }

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
                    path.to_fancy_string(|x| self.get_cfg_edge_weight(x).to_string())
                ));
                result = true;
                break;
            } else {
                self.logger.debug(&format!(
                    "Not reaching ({:?}): {:?}",
                    counters,
                    path.to_fancy_string(|x| self.get_cfg_edge_weight(x).to_string())
                ));
                // let parikh_image: ParikhImage = (&path).into();
                // parikh_image.print(&self.logger, LogLevel::Debug);

                let mut to_intersect = vec![];

                // When a path becomes negative at some index, we also cut away the prefix until
                // that point
                if let PathNReaching::Negative(index) = reaching {
                    self.logger
                        .debug(&format!("Path does not stay positive at index {:?}", index));

                    let cut_path = path.slice(index);

                    self.logger.debug(&format!(
                        "Cut path {:?}",
                        cut_path.to_fancy_string(|x| self.get_cfg_edge_weight(x).to_string())
                    ));

                    let dfa =
                        cut_path.to_negative_cut_dfa(dimension, |x| self.get_cfg_edge_weight(x));

                    to_intersect.push(dfa);
                }

                self.logger.debug("Building and checking LTC");

                let ltc_translation = LTCTranslation::from(&path).expand(&self.cfg);
                // self.logger.debug(&format!("LTC: {}", ltc_translation.to_fancy_string(|x|
                // self.get_cfg_edge_weight(x).to_string())));
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

                        to_intersect.push(dfa);
                    }
                } else {
                    self.logger.debug("LTC is not N-Reachable");

                    let dfa =
                        ltc_translation.to_dfa(true, dimension, |x| self.get_cfg_edge_weight(x));

                    to_intersect.push(dfa);
                }

                for dfa in to_intersect {
                    self.intersect_cfg(dfa);
                }
                self.minimize_cfg();

                if path.len() > (self.mu as usize) * dimension {
                    self.logger.debug("Path too long, increasing mu");
                    self.increment_mu();
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
            self.mu,
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

    fn run_modulo_bfs(&self) -> Option<Path> {
        self.cfg.modulo_reach(
            self.mu,
            &self.ivass.initial_valuation,
            &self.ivass.final_valuation,
        )
    }

    fn check_z_reach(&self) -> VASSZReachSolverResult {
        let result = VASSZReachSolverOptions::default()
            .with_time_limit(std::time::Duration::from_secs(5))
            .to_solver(
                self.cfg.clone(),
                self.ivass.initial_valuation.clone(),
                self.ivass.final_valuation.clone(),
            )
            .solve();

        self.logger.debug(&format!(
            "Checked Z-Reachability in {:?} and {:?} steps",
            result.statistics.time, result.statistics.step_count
        ));
        if result.is_failure() {
            self.logger.debug("CFG is not Z-Reachable");
        }
        if result.is_unknown() {
            self.logger.debug("CFG Z-Reachability Unknown");
        }

        result
    }

    fn get_cfg_edge_weight(&self, edge: EdgeIndex<u32>) -> CFGCounterUpdate {
        *self.cfg.edge_weight(edge)
    }

    fn intersect_cfg(&mut self, dfa: VASSCFG<()>) {
        self.cfg = self.cfg.intersect(&dfa);
    }

    fn minimize_cfg(&mut self) {
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

    fn print_end_banner(&self, result: &VASSReachSolverResult) {
        self.logger
            .object("Result")
            .add_field("result", &format!("{:?}", result.status))
            .add_field("max mu", &result.statistics.mu.to_string())
            .add_field("step count", &result.statistics.step_count.to_string())
            .add_field("time", &format!("{:?}", result.statistics.time))
            .log(LogLevel::Info);
    }
}
