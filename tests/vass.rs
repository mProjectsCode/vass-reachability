use vass_reachability::automaton::{dfa::DfaNodeData, vass::VASS, Automaton, AutBuild};

#[test]
fn test_vass() {
    let mut vass = VASS::<u32, char, 2>::new_from_data(vec!['a', 'b'], DfaNodeData::new(false, 0));
    let q0 = vass.start();
    let q1 = vass.add_state(DfaNodeData::new(true, 1));

    vass.add_transition(q0, q0, ('a', [1, 0]));
    vass.add_transition(q0, q1, ('b', [-1, 0]));
    vass.add_transition(q1, q1, ('b', [-1, 0]));

    let initialized_vass = vass.init([0, 0], [2, 0]);

    let input = "aaaabb";
    assert!(initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    let input = "aaaab";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));

    
    let input = "b";
    assert!(!initialized_vass.accepts(&input.chars().collect::<Vec<_>>()));
}

