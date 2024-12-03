use std::vec;

use vass_reachability::automaton::{vass::VASS, AutBuild, Automaton};

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

    let cfg = initialized_vass.to_cfg();

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

    assert!(initialized_vass.reach_1());
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

    assert!(!initialized_vass.reach_1());
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

    assert!(!initialized_vass.reach_1());
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

    assert!(!initialized_vass.reach_1());
}
