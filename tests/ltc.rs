use vass_reachability::{automaton::ltc::LTC, boxed_slice};

#[test]
fn ltc_n_reach_1() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(boxed_slice![0, 0], boxed_slice![1, 0]);
    ltc.add_loop(boxed_slice![0, 0], boxed_slice![0, 2]);
    ltc.add_transition(boxed_slice![1, 6], boxed_slice![0, 0]);

    // this one should be reachable in N and Z, and the loop should be taken three times

    assert!(ltc.reach_n(&vec![0, 0], &vec![0, 0]).result);
    assert!(ltc.reach_z(&vec![0, 0], &vec![0, 0]).result);
}

#[test]
fn ltc_n_reach_2() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(boxed_slice![0, 1], boxed_slice![0, 1]);

    // this one should not be reachable in N, but should be in Z

    assert!(!ltc.reach_n(&vec![0, 0], &vec![0, 0]).result);
    assert!(ltc.reach_z(&vec![0, 0], &vec![0, 0]).result);
}

#[test]
fn ltc_n_reach_3() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(boxed_slice![0, 0], boxed_slice![1, 0]);
    ltc.add_loop(boxed_slice![0, 0], boxed_slice![0, 2]);
    ltc.add_transition(boxed_slice![1, 5], boxed_slice![0, 0]);

    // this one should not be reachable in N and Z, as the loop can only produce even numbers on counter two

    assert!(!ltc.reach_n(&vec![0, 0], &vec![0, 0]).result);
    assert!(!ltc.reach_z(&vec![0, 0], &vec![0, 0]).result);
}
