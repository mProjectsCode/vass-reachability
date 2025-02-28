use std::{fs, time::Duration};

use rand::{Rng, SeedableRng, rngs::StdRng};
use vass_reachability::{
    automaton::petri_net::PetriNet,
    solver::{SolverStatus, vass_reach::VASSReachSolverOptions},
};

fn random_vass_test(
    seed: u64,
    place_count: usize,
    transition_count: usize,
    max_tokens_per_transition: usize,
    count: usize,
    timeout: u64,
    folder_name: &str,
) {
    let mut r = StdRng::seed_from_u64(seed);
    let mut results = vec![];

    println!();
    println!("Solving {} random Petri nets", count);
    println!("places: {}", place_count);
    println!("transitions: {}", transition_count);
    println!("max tokens per transition: {}", max_tokens_per_transition);
    println!();

    let path = format!("test_data/petri_nets/{}", folder_name);

    if !fs::exists(&path).unwrap() {
        fs::create_dir(&path).unwrap();
    }

    for _i in 0..count {
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

        let initial_m: Box<[usize]> = (0..place_count)
            .into_iter()
            .map(|_| r.gen_range(0..max_tokens_per_transition))
            .collect();
        let final_m: Box<[usize]> = (0..place_count)
            .into_iter()
            .map(|_| r.gen_range(0..max_tokens_per_transition))
            .collect();

        let initialized_petri_net = petri_net.init(initial_m, final_m);

        // if _i == 4 {
        //     dbg!(&initialized_petri_net);
        //     break;
        // }

        let initialized_vass = initialized_petri_net.to_vass();

        let res = VASSReachSolverOptions::default()
            .with_time_limit(Duration::from_secs(timeout)) // some time that is long enough, but makes the test run in a reasonable time
            .with_log_level(vass_reachability::logger::LogLevel::Error)
            .to_solver(initialized_vass)
            .solve();

        if res.is_unknown() {
            initialized_petri_net.to_file(&format!("{}/unknown_{}.json", path, _i));
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

    println!("Solved {solved} of {count}");
}

#[test]
fn test_vass_reach_random() {
    random_vass_test(1, 3, 3, 3, 50, 30, "3");
    random_vass_test(2, 4, 8, 4, 50, 30, "4");
}
