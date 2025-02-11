use std::{fs, time::Duration, vec};

use rand::{rngs::StdRng, Rng, SeedableRng};
use vass_reachability::{
    automaton::{petri_net::PetriNet, vass::VASS, AutBuild, Automaton},
    boxed_slice,
    solver::vass_reach::{VASSReachSolverOptions, VASSReachSolverResult},
};

#[test]
fn test_vass() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    vass.add_transition(q0, q1, ('b', vec![-1, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![2, 0].into(), q0, q1);

    let input = "aaaabb";
    assert!(initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    let input = "aaaab";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    let input = "b";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));
}

#[test]
fn test_vass_to_cfg() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    vass.add_transition(q0, q1, ('b', vec![-2, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let _cfg = initialized_vass.to_cfg();

    assert!(true);

    // dbg!(&cfg);
}

#[test]
fn test_vass_reach_1() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    vass.add_transition(q0, q1, ('b', vec![-2, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(res.reachable());
}

#[test]
fn test_vass_reach_2() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
    vass.add_transition(q0, q1, ('b', vec![0, 1].into()));
    vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

// this test currently runs forever
#[test]
fn test_vass_reach_3() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q1, ('a', vec![-1, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

// this test currently runs forever
#[test]
fn test_vass_reach_4() {
    // this is a simple model for mutual exclusion
    // we have two processes
    // a with counter 1 and 2
    // b with counter 3 and 4
    // and a shared resource with counter 5
    // counter 2 and 4 are the critical sections
    let mut vass = VASS::<u32, char>::new(5, vec!['a', 'b', 'c', 'd', 'e']);

    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);
    let q2 = vass.add_state(2);

    // we use q1 to initialize the entire system
    vass.add_transition(q0, q1, ('e', vec![1, 0, 1, 0, 0].into()));

    vass.add_transition(q1, q1, ('a', vec![-1, 1, 0, 0, -1].into()));
    vass.add_transition(q1, q1, ('b', vec![0, 0, -1, 1, -1].into()));
    vass.add_transition(q1, q1, ('c', vec![1, -1, 0, 0, 1].into()));
    vass.add_transition(q1, q1, ('d', vec![0, 0, 1, -1, 1].into()));

    // we can only reach q1 when we are in the critical section on both processes
    vass.add_transition(q1, q2, ('e', vec![0, -1, 0, -1, 0].into()));

    let initialized_vass = vass.init(
        vec![0, 0, 0, 0, 0].into(),
        vec![0, 0, 0, 0, 0].into(),
        q0,
        q2,
    );

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

#[test]
fn test_vass_reach_5() {
    // same as test 4, but we build it from a petri net.

    let mut petri_net = PetriNet::new(5);

    petri_net.add_transition(vec![(1, 1), (1, 5)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 3), (1, 5)], vec![(1, 4)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 1), (1, 5)]);
    petri_net.add_transition(vec![(1, 4)], vec![(1, 3), (1, 5)]);

    let initialized_petri_net =
        petri_net.init(boxed_slice![1, 0, 1, 0, 0], boxed_slice![0, 1, 0, 1, 0]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

#[test]
fn test_vass_reach_6() {
    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 0, 0], boxed_slice![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();

    // dbg!(&initialized_vass);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(res.reachable());
}

#[test]
fn test_vass_reach_7() {
    let mut petri_net = PetriNet::new(3);

    petri_net.add_transition(vec![], vec![(2, 1)]);
    petri_net.add_transition(vec![(1, 1), (1, 2)], vec![(2, 2), (2, 3)]);
    petri_net.add_transition(vec![(2, 3)], vec![(2, 1), (1, 2)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 2], boxed_slice![1, 2, 2]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Debug)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

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
            .solve_n();

        if res.unknown() {
            initialized_petri_net.to_file(&format!("{}/unknown_{}.json", path, _i));
        }

        println!("{}: {:?}", _i, res.result);
        results.push(res);
    }

    println!();
    println!("{:?}", results);

    let solved = results
        .iter()
        .filter(|r| !matches!(r.result, VASSReachSolverResult::Unknown(_)))
        .count();

    println!("Solved {solved} of {count}");
}

#[test]
fn test_vass_reach_random() {
    random_vass_test(1, 3, 3, 3, 50, 10, "3");
    random_vass_test(2, 4, 8, 4, 50, 10, "4");
}
