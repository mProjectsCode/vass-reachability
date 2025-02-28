use std::time::Duration;

use vass_reachability::{
    automaton::{AutBuild, petri_net::PetriNet, vass::VASS},
    boxed_slice,
    solver::vass_reach::VASSReachSolverOptions,
};

#[test]
fn test_vass_n_reach_1() {
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
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(res.is_success());
}

#[test]
fn test_vass_n_reach_2() {
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
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(!res.is_success());
}

#[test]
fn test_vass_n_reach_3() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q1, ('a', vec![-1, 0].into()));
    vass.add_transition(q1, q1, ('b', vec![1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(!res.is_success());
}

#[test]
fn test_vass_n_reach_4() {
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
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(!res.is_success());
}

#[test]
fn test_vass_n_reach_5() {
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
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(!res.is_success());
}

#[test]
fn test_vass_n_reach_6() {
    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 0, 0], boxed_slice![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();

    // dbg!(&initialized_vass);

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(res.is_success());
}

#[test]
fn test_vass_n_reach_7() {
    let mut petri_net = PetriNet::new(3);

    petri_net.add_transition(vec![], vec![(2, 1)]);
    petri_net.add_transition(vec![(1, 1), (1, 2)], vec![(2, 2), (2, 3)]);
    petri_net.add_transition(vec![(2, 3)], vec![(2, 1), (1, 2)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 2], boxed_slice![1, 2, 2]);

    let initialized_vass = initialized_petri_net.to_vass();

    let res = VASSReachSolverOptions::default()
        .with_mu_limit(100)
        .with_time_limit(Duration::from_secs(5)) // some time that is long enough, but makes the test run in a reasonable time
        .with_log_level(vass_reachability::logger::LogLevel::Info)
        .to_solver(initialized_vass)
        .solve();

    assert!(!res.is_success());
}
