use std::{fs, time::Duration};

use itertools::Itertools;
use rand::{Rng, SeedableRng, rngs::StdRng};
use vass_reach_lib::{
    automaton::{
        AutBuild,
        petri_net::PetriNet,
        vass::{VASS, counter::VASSCounterValuation, initialized::InitializedVASS},
    },
    logger::{LogLevel, Logger},
    solver::{SolverStatus, vass_reach::VASSReachSolverOptions},
};

pub struct RandomOptions<'a> {
    pub seed: u64,
    pub count: usize,
    pub solver_options: VASSReachSolverOptions<'a>,
    pub folder_name: Option<String>,
}

impl<'a> Default for RandomOptions<'a> {
    fn default() -> Self {
        RandomOptions {
            seed: 1,
            count: 10,
            solver_options: VASSReachSolverOptions::default()
                .with_time_limit(Duration::from_secs(10)),
            folder_name: None,
        }
    }
}

impl<'a> RandomOptions<'a> {
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = seed;
        self
    }

    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn with_solver_options(mut self, solver_options: VASSReachSolverOptions<'a>) -> Self {
        self.solver_options = solver_options;
        self
    }

    pub fn with_folder_name(mut self, folder_name: String) -> Self {
        self.folder_name = Some(folder_name);
        self
    }
}

fn random_petri_net_test(
    options: RandomOptions,
    place_count: usize,
    transition_count: usize,
    max_tokens_per_transition: usize,
) {
    let mut r = StdRng::seed_from_u64(options.seed);
    let mut results = vec![];

    println!();
    println!("Solving {} random Petri nets", options.count);
    println!("places: {}", place_count);
    println!("transitions: {}", transition_count);
    println!("max tokens per transition: {}", max_tokens_per_transition);
    println!();

    let path = options.folder_name.map(|s| format!("test_data/{s}"));

    if let Some(path) = &path {
        if !fs::exists(&path).unwrap() {
            fs::create_dir(&path).unwrap();
        }
    }

    for _i in 0..options.count {
        let mut petri_net = PetriNet::new(place_count);

        for _ in 0..transition_count {
            let mut input = vec![];
            let mut output = vec![];

            for p in 1..=place_count {
                input.push((r.gen_range(0..max_tokens_per_transition), p));
                output.push((r.gen_range(0..max_tokens_per_transition), p));
            }

            petri_net.add_transition(input, output);
        }

        let initial_m: VASSCounterValuation = (0..place_count)
            .into_iter()
            .map(|_| r.gen_range(0..max_tokens_per_transition) as i32)
            .collect();
        let final_m: VASSCounterValuation = (0..place_count)
            .into_iter()
            .map(|_| r.gen_range(0..max_tokens_per_transition) as i32)
            .collect();

        let initialized_petri_net = petri_net.init(initial_m, final_m);

        let initialized_vass = initialized_petri_net.to_vass();

        let res = options
            .solver_options
            .clone()
            .to_vass_solver(&initialized_vass)
            .solve();

        if res.is_unknown() {
            if let Some(path) = &path {
                initialized_petri_net.to_file(&format!("{}/unknown_{}.json", path, _i));
            }
        }

        println!("{}: {:?}", _i, res.status);
        results.push(res);
    }

    println!();
    println!("{:?}", results);

    let solved = results
        .iter()
        .filter(|r| !matches!(r.status, SolverStatus::Unknown(_)))
        .count();

    println!("Solved {solved} of {}", options.count);
}

fn random_vass_test(
    options: RandomOptions,
    state_count: usize,
    dimension: usize,
    transition_count: usize,
    max_tokens_per_transition: i32,
) {
    let mut r = StdRng::seed_from_u64(options.seed);
    let mut results = vec![];

    println!();
    println!("Solving {} random VASS", options.count);
    println!("dimension: {}", dimension);
    println!("states: {}", state_count);
    println!("transitions: {}", transition_count);
    println!("max tokens per transition: {}", max_tokens_per_transition);
    println!();

    // let path = options.folder_name.map(|s| format!("test_data/{s}"));

    // if let Some(path) = &path {
    //     if !fs::exists(&path).unwrap() {
    //         fs::create_dir(&path).unwrap();
    //     }
    // }

    let alphabet = (0..transition_count).collect_vec();

    for _i in 0..options.count {
        let mut vass = VASS::<(), usize>::new(dimension, alphabet.clone());

        let mut states = vec![];
        for _i in 0..state_count {
            let state = vass.add_state(());
            states.push(state);
        }

        for i in 0..transition_count {
            let from = r.gen_range(0..state_count);
            let to = r.gen_range(0..state_count);

            let mut input = vec![];

            for p in 0..dimension {
                input.push(r.gen_range(-max_tokens_per_transition..=max_tokens_per_transition));
            }

            vass.add_transition(states[from], states[to], (i, input.into()));
        }

        let initial_m: VASSCounterValuation = (0..dimension)
            .into_iter()
            .map(|_| r.gen_range(0..=max_tokens_per_transition))
            .collect();

        let final_m: VASSCounterValuation = (0..dimension)
            .into_iter()
            .map(|_| r.gen_range(0..=max_tokens_per_transition))
            .collect();

        let initialized_vass = vass.init(initial_m, final_m, states[0], states[state_count - 1]);

        let res = options
            .solver_options
            .clone()
            .to_vass_solver(&initialized_vass)
            .solve();

        // if res.is_unknown() {
        //     if let Some(path) = &path {
        //         initialized_petri_net.to_file(&format!("{}/unknown_{}.json", path,
        // _i));     }
        // }

        println!("{}: {:?}", _i, res.status);
        results.push(res);
    }

    println!();
    println!("{:?}", results);

    let solved = results
        .iter()
        .filter(|r| !matches!(r.status, SolverStatus::Unknown(_)))
        .count();

    println!("Solved {solved} of {}", options.count);
}

#[test]
fn test_vass_reach_random() {
    let logger = Logger::new(LogLevel::Error, "test".to_string(), None);

    // random_vass_test(1, 3, 3, 3, 1000, 20, "3");
    let options = RandomOptions::default()
        .with_seed(1)
        .with_count(1000)
        .with_solver_options(
            VASSReachSolverOptions::default()
                .with_iteration_limit(1000)
                .with_time_limit(Duration::from_secs(20))
                .with_logger(&logger),
        );
    // .with_folder_name("petri_nets/1".to_string());
    // random_petri_net_test(options, 3, 5, 5);
    // random_petri_net_test(options, 3, 6, 3);
    // random_petri_net_test(options, 1, 4, 10);

    let options = RandomOptions::default()
        .with_seed(2)
        .with_count(100)
        .with_solver_options(
            VASSReachSolverOptions::default()
                .with_iteration_limit(1000)
                .with_time_limit(Duration::from_secs(30))
                .with_logger(&logger),
        );
    // .with_folder_name("petri_nets/4".to_string());

    // random_petri_net_test(options, 4, 8, 4);

    let options = RandomOptions::default()
        .with_seed(3)
        .with_count(100)
        .with_solver_options(
            VASSReachSolverOptions::default()
                .with_iteration_limit(1000)
                .with_time_limit(Duration::from_secs(20))
                .with_logger(&logger),
        );

    // random_vass_test(options, 6, 4, 10, 3);

    // let options = RandomOptions::default()
    //     .with_seed(3)
    //     .with_count(100)
    //     .with_solver_options(
    //         VASSReachSolverOptions::default()
    //             .with_iteration_limit(1000)
    //             .with_time_limit(Duration::from_secs(20))
    //
    // .with_log_level(vass_reachability::logger::LogLevel::Error),     //
    // );

    // random_vass_test(options, 6, 5, 15, 3);
}
