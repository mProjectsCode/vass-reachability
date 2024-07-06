use vass_reachability::automaton::{vass::VASS, AutBuild, Automaton};

#[test]
fn test_vass() {
    let mut vass = VASS::<u32, char, 2>::new(vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', [1, 0]));
    vass.add_transition(q0, q1, ('b', [-1, 0]));
    vass.add_transition(q1, q1, ('b', [-1, 0]));

    let initialized_vass = vass.init([0, 0], [2, 0], q0, q1);

    let input = "aaaabb";
    assert!(initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    let input = "aaaab";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    let input = "b";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));
}

#[test]
fn test_vass_to_cfg() {
    let mut vass = VASS::<u32, char, 2>::new(vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', [1, 0]));
    vass.add_transition(q0, q1, ('b', [-2, 0]));
    vass.add_transition(q1, q1, ('b', [-1, 0]));

    let initialized_vass = vass.init([0, 0], [0, 0], q0, q1);

    let cfg = initialized_vass.to_cfg();

    // dbg!(&cfg);
}

#[test]
fn test_vass_reach_1() {
    let mut vass = VASS::<u32, char, 2>::new(vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', [1, 0]));
    vass.add_transition(q0, q1, ('b', [-2, 0]));
    vass.add_transition(q1, q1, ('b', [-1, 0]));

    let initialized_vass = vass.init([0, 0], [0, 0], q0, q1);

    // this one is reachable, so we won't find anything with this method
    // assert!(initialized_vass.reach_1());
}

#[test]
fn test_vass_reach_2() {
    let mut vass = VASS::<u32, char, 2>::new(vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', [1, 0]));
    vass.add_transition(q0, q1, ('b', [0, 1]));
    vass.add_transition(q1, q1, ('b', [-1, 0]));

    let initialized_vass = vass.init([0, 0], [0, 0], q0, q1);

    assert!(!initialized_vass.reach_1());
}
