use std::{fmt::Debug, sync::{atomic::AtomicBool, Arc}};

use petgraph::graph::EdgeIndex;

use crate::{
    automaton::{
        dfa::{DFA, VASSCFG},
        ltc::{LTCSolverResult, LTCTranslation},
        path::{Path, PathNReaching},
        vass::InitializedVASS,
        AutEdge, AutNode,
    },
    logger::{LogLevel, Logger},
    threading::thread_pool::ThreadPool,
};

#[derive(Debug)]
pub struct VASSReachSolver<N: AutNode, E: AutEdge> {
    options: VASSReachSolverOptions,
    logger: Logger,
    ivass: InitializedVASS<N, E>,
    thread_pool: ThreadPool<LTCSolverResult>,
    cfg: DFA<Vec<Option<N>>, i32>,
    mu: u32,
    step_count: u32,
    solver_start_time: Option<std::time::Instant>,
    stop_signal: Arc<AtomicBool>,
}

impl<N: AutNode, E: AutEdge> VASSReachSolver<N, E> {
    pub fn new(options: VASSReachSolverOptions, ivass: InitializedVASS<N, E>) -> Self {
        let logger = Logger::new(options.log_level.clone(), "VASS Reach Solver".to_string());

        let thread_pool = ThreadPool::<LTCSolverResult>::new(options.thread_pool_size);

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
            thread_pool,
            cfg,
            mu: 2,
            step_count: 0,
            solver_start_time: None,
            stop_signal,
        }
    }

    pub fn solve_n(&mut self) -> VASSReachSolverStatistics {
        let dimension = self.ivass.dimension();

        self.solver_start_time = Some(std::time::Instant::now());

        self.print_start_banner();
        self.logger.empty(LogLevel::Info);

        let mut result;
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

            if self.handle_thread_pool() {
                result = true;
                break;
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

                if path.has_loop() {
                    self.logger.debug("Path has loop, checking LTC");

                    let ltc_translation = LTCTranslation::from_path(&path);
                    let ltc = ltc_translation.to_ltc(dimension, |x| self.get_cfg_edge_weight(x));

                    let initial_v = self.ivass.initial_valuation.clone();
                    let final_v = self.ivass.final_valuation.clone();

                    self.thread_pool
                        .schedule(move || ltc.reach_n(&initial_v, &final_v));

                    let dfa = ltc_translation.to_dfa(dimension, |x| self.get_cfg_edge_weight(x));

                    self.intersect_cfg(dfa);
                } else if let PathNReaching::Negative(index) = reaching {
                    self.logger
                        .debug(&format!("Path does not stay positive at index {:?}", index));

                    let sliced_path = path.slice(index);
                    let dfa =
                        sliced_path.simple_to_dfa(true, dimension, |x| self.get_cfg_edge_weight(x));

                    self.intersect_cfg(dfa);
                } else {
                    self.logger
                        .debug("Path only modulo reaching, increasing mu");

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

        self.logger.info("Joining thread pool");

        if result {
            self.thread_pool.join(false);

            self.logger.debug(&format!(
                "Canceled jobs: {:?}",
                self.thread_pool.get_active_jobs()
            ));
        } else {
            self.logger.debug(&format!(
                "Waiting on {:?} active jobs",
                self.thread_pool.get_active_jobs()
            ));

            self.thread_pool.join(true);

            assert_eq!(self.thread_pool.get_active_jobs(), 0);

            for solver_result in self.thread_pool.get_finished_jobs() {
                if solver_result.result {
                    self.logger.debug("A thread found a solution");
                    result = true;
                    break;
                }
            }
        }

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

    fn handle_thread_pool(&self) -> bool {
        if self.thread_pool.get_active_jobs() >= self.options.thread_pool_size * 4 {
            self.logger.info(&format!(
                "Waiting on thread pool to empty. Active jobs: {:?}, Target: {:?}",
                self.thread_pool.get_active_jobs(),
                self.options.thread_pool_size
            ));
            self.thread_pool
                .block_until_x_active_jobs(self.options.thread_pool_size);
        }
        self.thread_pool.block_until_x_active_jobs_if_above_y(4, 10);

        let finished_jobs = self.thread_pool.get_finished_jobs();

        if finished_jobs.iter().any(|x| x.result) {
            self.logger.debug("A thread found a solution");
            return true;
        }

        false
    }

    fn run_modulo_bfs(&self) -> Option<Path> {
        self.cfg.modulo_reach(
            self.mu,
            &self.ivass.initial_valuation,
            &self.ivass.final_valuation,
        )
    }

    fn get_cfg_edge_weight(&self, edge: EdgeIndex<u32>) -> i32 {
        *self.cfg.edge_weight(edge)
    }

    fn intersect_cfg(&mut self, dfa: DFA<(), i32>) {
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
}

impl VASSReachSolverOptions {
    pub fn new(
        log_level: LogLevel,
        thread_pool_size: usize,
        max_iterations: Option<u32>,
        max_mu: Option<u32>,
        max_time: Option<std::time::Duration>,
    ) -> Self {
        VASSReachSolverOptions {
            log_level,
            thread_pool_size,
            max_iterations,
            max_mu,
            max_time,
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

    pub fn to_solver<N: AutNode, E: AutEdge>(
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
}
