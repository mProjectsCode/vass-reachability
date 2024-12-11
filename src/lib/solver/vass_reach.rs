use crate::{
    automaton::{
        dfa::{DFA, VASSCFG},
        ltc::{LTCSolverResult, LTCTranslation},
        path::PathNReaching,
        vass::InitializedVASS,
        AutEdge, AutNode,
    },
    threading::thread_pool::ThreadPool,
};

pub struct VASSReachSolver {
    options: VASSReachSolverOptions,
}

impl VASSReachSolver {
    pub fn new(options: VASSReachSolverOptions) -> Self {
        VASSReachSolver { options }
    }

    pub fn solve_n<N: AutNode, E: AutEdge>(
        &self,
        ivass: &InitializedVASS<N, E>,
    ) -> VASSReachSolverStatistics {
        let dimension = ivass.dimension();

        let time = std::time::Instant::now();

        let mut thread_pool = ThreadPool::<LTCSolverResult>::new(self.options.thread_pool_size);

        let mut cfg: DFA<Vec<Option<N>>, i32> = ivass.to_cfg();
        cfg.add_failure_state(vec![]);
        cfg = cfg.minimize();

        self.print_start_banner(ivass, &cfg);

        let mut mu = 2;
        let mut result;
        let mut step_time;
        let mut step_count = 0;

        loop {
            step_count += 1;

            if self.max_iterations_reached(step_count) {
                return VASSReachSolverStatistics::from_error(
                    VASSReachSolverError::MaxIterationsReached,
                    step_count,
                    mu,
                    time.elapsed(),
                );
            }

            if self.max_mu_reached(mu) {
                return VASSReachSolverStatistics::from_error(
                    VASSReachSolverError::MaxMuReached,
                    step_count,
                    mu,
                    time.elapsed(),
                );
            }

            step_time = std::time::Instant::now();

            println!();
            println!("--- Step: {} ---", step_count);

            thread_pool.block_until_x_active_jobs_if_above_y(4, 10);

            let finished_jobs = thread_pool.get_finished_jobs();
            println!("Finished jobs: {:?}", finished_jobs);
            if finished_jobs.iter().any(|x| x.result) {
                println!("One of the threads found a solution");

                result = true;
                break;
            }

            println!("Thread pool handling time: {:?}", step_time.elapsed());

            println!("Mu: {}", mu);
            println!(
                "CFG: {:?} states, {:?} transitions",
                cfg.state_count(),
                cfg.graph.edge_count()
            );

            let reach_time = std::time::Instant::now();

            let reach_path = cfg.modulo_reach(mu, &ivass.initial_valuation, &ivass.final_valuation);

            println!("BFS time: {:?}", reach_time.elapsed());

            if reach_path.is_none() {
                println!("No paths found");

                result = false;
                break;
            }

            let path = reach_path.unwrap();
            let (reaching, counters) =
                path.is_n_reaching(&ivass.initial_valuation, &ivass.final_valuation, |x| {
                    *cfg.edge_weight(x)
                });

            if reaching == PathNReaching::True {
                println!("Reaching: {:?}", path.simple_print(|x| *cfg.edge_weight(x)));
                result = true;
                break;
            } else {
                println!(
                    "Not reaching: {:?} = {:?}",
                    path.simple_print(|x| *cfg.edge_weight(x)),
                    counters
                );

                if path.has_loop() {
                    println!("Path has loop, checking LTC");

                    let ltc_translation = LTCTranslation::from_path(&path);
                    let ltc = ltc_translation.to_ltc(dimension, |x| *cfg.edge_weight(x));

                    let initial_v = ivass.initial_valuation.clone();
                    let final_v = ivass.final_valuation.clone();

                    thread_pool.schedule(move || ltc.reach_n(&initial_v, &final_v));

                    let dfa = ltc_translation.to_dfa(dimension, |x| *cfg.edge_weight(x));

                    cfg = cfg.intersect(&dfa);
                    cfg = cfg.minimize();
                } else if let PathNReaching::Negative(index) = reaching {
                    println!("Does not stay positive at index {:?}", index);

                    let sliced_path = path.slice(index);

                    let dfa = sliced_path.simple_to_dfa(true, dimension, |x| *cfg.edge_weight(x));

                    cfg = cfg.intersect(&dfa);
                    cfg = cfg.minimize();
                } else {
                    println!("Path only modulo reaching, increasing mu");

                    mu += 1;
                }
            }

            println!("Time for step: {:?}", step_time.elapsed());
        }

        println!();
        println!("Stopping Workers");

        if result {
            thread_pool.join(false);

            let finished_jobs = thread_pool.get_finished_jobs();
            println!("Finished jobs: {:?}", finished_jobs);
            println!("Unfinished jobs: {:?}", thread_pool.get_active_jobs());
        } else {
            thread_pool.join(true);

            let finished_jobs = thread_pool.get_finished_jobs();
            println!("Finished jobs: {:?}", finished_jobs);
            println!("Unfinished jobs: {:?}", thread_pool.get_active_jobs());

            for solver_result in thread_pool.get_finished_jobs() {
                if solver_result.result {
                    println!("One of the threads found a solution");
                    result = true;
                    break;
                }
            }
        }

        let statistics = VASSReachSolverStatistics::new(result, step_count, mu, time.elapsed());

        self.print_end_banner(&statistics);

        statistics
    }

    fn max_mu_reached(&self, mu: u32) -> bool {
        self.options.max_mu.map(|x| x <= mu).unwrap_or(false)
    }

    fn max_iterations_reached(&self, iterations: u32) -> bool {
        self.options
            .max_iterations
            .map(|x| x <= iterations)
            .unwrap_or(false)
    }

    fn print_start_banner<N: AutNode, E: AutEdge>(
        &self,
        ivass: &InitializedVASS<N, E>,
        cfg: &DFA<Vec<Option<N>>, i32>,
    ) {
        if !self.options.verbose {
            return;
        }

        println!();
        println!("--- VASS N-Reach Solver ---");
        println!(
            "VASS: {:?} states, {:?} transitions",
            ivass.vass.state_count(),
            ivass.vass.transition_count()
        );
        println!("Dimension: {:?}", ivass.dimension());
        println!(
            "CFG: {:?} states, {:?} transitions",
            cfg.state_count(),
            cfg.graph.edge_count()
        );
        println!("-----");
    }

    fn print_end_banner(&self, statistics: &VASSReachSolverStatistics) {
        if !self.options.verbose {
            return;
        }

        println!();
        println!("--- Results ---");
        println!("Result: {:?}", statistics.result);
        println!("Max mu: {}", statistics.mu);
        println!("Step count: {}", statistics.iterations);
        println!("Time: {:?}", statistics.time);
        println!("-----");
        println!();
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VASSReachSolverOptions {
    verbose: bool,
    thread_pool_size: usize,
    max_iterations: Option<u32>,
    max_mu: Option<u32>,
    max_time: Option<std::time::Duration>,
}

impl VASSReachSolverOptions {
    pub fn new(
        verbose: bool,
        thread_pool_size: usize,
        max_iterations: Option<u32>,
        max_mu: Option<u32>,
        max_time: Option<std::time::Duration>,
    ) -> Self {
        VASSReachSolverOptions {
            verbose,
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

    pub fn verbose(mut self) -> Self {
        self.verbose = true;
        self
    }

    pub fn quiet(mut self) -> Self {
        self.verbose = false;
        self
    }

    pub fn with_thread_pool_size(mut self, size: usize) -> Self {
        self.thread_pool_size = size;
        self
    }

    pub fn to_solver(self) -> VASSReachSolver {
        VASSReachSolver::new(self)
    }
}

impl Default for VASSReachSolverOptions {
    fn default() -> Self {
        VASSReachSolverOptions {
            verbose: true,
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
