use std::vec;

use rand::{rngs::StdRng, Rng, SeedableRng};
use vass_reachability::{
    automaton::{petri_net::PetriNet, vass::VASS, AutBuild, Automaton},
    solver::vass_reach::VASSReachSolverOptions,
};

#[test]
fn test_vass() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0]));
    vass.add_transition(q0, q1, ('b', vec![-1, 0]));
    vass.add_transition(q1, q1, ('b', vec![-1, 0]));

    let initialized_vass = vass.init(vec![0, 0], vec![2, 0], q0, q1);

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

    vass.add_transition(q0, q0, ('a', vec![1, 0]));
    vass.add_transition(q0, q1, ('b', vec![-2, 0]));
    vass.add_transition(q1, q1, ('b', vec![-1, 0]));

    let initialized_vass = vass.init(vec![0, 0], vec![0, 0], q0, q1);

    let _cfg = initialized_vass.to_cfg();

    assert!(true);

    // dbg!(&cfg);
}

#[test]
fn test_vass_reach_1() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0]));
    vass.add_transition(q0, q1, ('b', vec![-2, 0]));
    vass.add_transition(q1, q1, ('b', vec![-1, 0]));

    let initialized_vass = vass.init(vec![0, 0], vec![0, 0], q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(res.reachable());
}

#[test]
fn test_vass_reach_2() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', vec![1, 0]));
    vass.add_transition(q0, q1, ('b', vec![0, 1]));
    vass.add_transition(q1, q1, ('b', vec![-1, 0]));

    let initialized_vass = vass.init(vec![0, 0], vec![0, 0], q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
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

    vass.add_transition(q0, q1, ('a', vec![-1, 0]));
    vass.add_transition(q1, q1, ('b', vec![1, 0]));

    let initialized_vass = vass.init(vec![0, 0], vec![0, 0], q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
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
    vass.add_transition(q0, q1, ('e', vec![1, 0, 1, 0, 0]));

    vass.add_transition(q1, q1, ('a', vec![-1, 1, 0, 0, -1]));
    vass.add_transition(q1, q1, ('b', vec![0, 0, -1, 1, -1]));
    vass.add_transition(q1, q1, ('c', vec![1, -1, 0, 0, 1]));
    vass.add_transition(q1, q1, ('d', vec![0, 0, 1, -1, 1]));

    // we can only reach q1 when we are in the critical section on both processes
    vass.add_transition(q1, q2, ('e', vec![0, -1, 0, -1, 0]));

    let initialized_vass = vass.init(vec![0, 0, 0, 0, 0], vec![0, 0, 0, 0, 0], q0, q2);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
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

    let initialized_petri_net = petri_net.init(vec![1, 0, 1, 0, 0], vec![0, 1, 0, 1, 0]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
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

    let initialized_petri_net = petri_net.init(vec![1, 0, 0, 0], vec![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();

    // dbg!(&initialized_vass);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
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

    let initialized_petri_net = petri_net.init(vec![1, 0, 2], vec![1, 2, 2]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .to_solver(initialized_vass)
        .solve_n();

    assert!(!res.reachable());
}

// #[test]
fn test_vass_reach_random() {
    let mut r = StdRng::seed_from_u64(1);
    let place_count = 3;
    let transition_count = 3;
    let max_tokens_per_transition = 3;

    let mut results = vec![];

    for _i in 0..10 {
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

        let mut initial_m = vec![];
        let mut final_m = vec![];

        for _ in 0..place_count {
            initial_m.push(r.gen_range(0..max_tokens_per_transition));
            final_m.push(r.gen_range(0..max_tokens_per_transition));
        }

        let initialized_petri_net = petri_net.init(initial_m.clone(), final_m.clone());

        // if _i == 4 {
        //     dbg!(&initialized_petri_net);
        //     break;
        // }

        let time = std::time::Instant::now();

        let initialized_vass = initialized_petri_net.to_vass();

        let res = VASSReachSolverOptions::default()
            .with_mu_limit(100)
            .to_solver(initialized_vass)
            .solve_n();

        results.push((res, time.elapsed()));
    }

    println!("{:?}", results);
}
