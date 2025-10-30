use vass_reach_lib::{
    automaton::{
        ltc::{LTC, translation::LTCTranslation},
        petri_net::initialized::InitializedPetriNet,
    },
    validation::same_language::assert_subset_language,
};

#[test]
fn ltc_n_reach_1() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(vec![0, 0].into(), vec![1, 0].into());
    ltc.add_loop(vec![0, 0].into(), vec![0, 2].into());
    ltc.add_transition(vec![1, 6].into(), vec![0, 0].into());

    // this one should be reachable in N and Z, and the loop should be taken three
    // times

    assert!(ltc.reach_n(&vec![0, 0].into(), &vec![0, 0].into()).result);
    assert!(ltc.reach_z(&vec![0, 0].into(), &vec![0, 0].into()).result);
}

#[test]
fn ltc_n_reach_2() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(vec![0, 1].into(), vec![0, 1].into());

    // this one should not be reachable in N, but should be in Z

    assert!(!ltc.reach_n(&vec![0, 0].into(), &vec![0, 0].into()).result);
    assert!(ltc.reach_z(&vec![0, 0].into(), &vec![0, 0].into()).result);
}

#[test]
fn ltc_n_reach_3() {
    let mut ltc = LTC::new(2);
    ltc.add_transition(vec![0, 0].into(), vec![1, 0].into());
    ltc.add_loop(vec![0, 0].into(), vec![0, 2].into());
    ltc.add_transition(vec![1, 5].into(), vec![0, 0].into());

    // this one should not be reachable in N and Z, as the loop can only produce
    // even numbers on counter two

    assert!(!ltc.reach_n(&vec![0, 0].into(), &vec![0, 0].into()).result);
    assert!(!ltc.reach_z(&vec![0, 0].into(), &vec![0, 0].into()).result);
}

#[test]
fn ltc_n_reach_4() {
    let mut ltc = LTC::new(1);
    ltc.add_loop(vec![1].into(), vec![2].into());

    assert!(!ltc.reach_n(&vec![0].into(), &vec![2].into()).result);
    assert!(ltc.reach_z(&vec![0].into(), &vec![2].into()).result);
}

#[test]
fn ltc_n_reach_5() {
    let mut ltc = LTC::new(1);
    ltc.add_loop(vec![2].into(), vec![1].into());

    assert!(!ltc.reach_n(&vec![2].into(), &vec![0].into()).result);
    assert!(ltc.reach_z(&vec![2].into(), &vec![0].into()).result);
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

    let translation: LTCTranslation = (&path).into();
    let non_expanded_dfa = translation
        .to_dfa(&cfg, initialized_vass.dimension(), false)
        .invert();
    let expanded_translation = translation.expand(&cfg);
    let expanded_dfa = expanded_translation
        .to_dfa(&cfg, initialized_vass.dimension(), false)
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

    let path = cfg.modulo_reach(
        6,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );
    assert!(path.is_some());
    let path = path.unwrap();

    let translation: LTCTranslation = (&path).into();
    let non_expanded_dfa = translation
        .to_dfa(&cfg, initialized_vass.dimension(), false)
        .invert();

    let expanded_translation = translation.expand(&cfg);
    let expanded_dfa = expanded_translation
        .to_dfa(&cfg, initialized_vass.dimension(), false)
        .invert();

    assert_subset_language(&non_expanded_dfa, &cfg, 6);
    assert_subset_language(&expanded_dfa, &cfg, 6);
    assert_subset_language(&non_expanded_dfa, &expanded_dfa, 6);
}
