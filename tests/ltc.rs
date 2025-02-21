use vass_reachability::{
    automaton::{
        ltc::{LTCTranslation, LTC},
        petri_net::InitializedPetriNet,
    },
    boxed_slice,
    validation::same_language::assert_subset_language,
};

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

#[test]
fn ltc_language_1() {
    let initialized_vass =
        InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json").to_vass();
    let cfg = initialized_vass.to_cfg();

    let path = cfg.modulo_reach(
        4,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );
    assert!(path.is_some());
    let path = path.unwrap();

    let translation = LTCTranslation::from_path(&path);
    let non_expanded_dfa = translation
        .to_dfa(false, initialized_vass.dimension(), |edge| {
            *cfg.edge_weight(edge)
        })
        .invert();
    let expanded_translation = translation.expand(&cfg);
    let expanded_dfa = expanded_translation
        .to_dfa(false, initialized_vass.dimension(), |edge| {
            *cfg.edge_weight(edge)
        })
        .invert();

    assert_subset_language(&non_expanded_dfa, &cfg, 6);
    assert_subset_language(&expanded_dfa, &cfg, 6);
    assert_subset_language(&non_expanded_dfa, &expanded_dfa, 6);
}

#[test]
fn ltc_language_2() {
    let initialized_vass =
        InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json").to_vass();
    let cfg = initialized_vass.to_cfg();

    dbg!(&cfg);

    let path = cfg.modulo_reach(
        6,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );
    assert!(path.is_some());
    let path = path.unwrap();

    let translation = LTCTranslation::from_path(&path);
    let non_expanded_dfa = translation
        .to_dfa(false, initialized_vass.dimension(), |edge| {
            *cfg.edge_weight(edge)
        })
        .invert();
    let expanded_translation = translation.expand(&cfg);
    let expanded_dfa = expanded_translation
        .to_dfa(false, initialized_vass.dimension(), |edge| {
            *cfg.edge_weight(edge)
        })
        .invert();

    assert_subset_language(&non_expanded_dfa, &cfg, 6);
    assert_subset_language(&expanded_dfa, &cfg, 6);
    assert_subset_language(&non_expanded_dfa, &expanded_dfa, 6);
}
