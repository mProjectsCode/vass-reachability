use std::vec;

use vass_reachability::automaton::{vass::VASS, AutBuild, Automaton};

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
