use vass_reachability::{
    automaton::{
        AutBuild,
        petri_net::{PetriNet, initialized::InitializedPetriNet},
        vass::VASS,
    },
    boxed_slice,
    solver::vass_z_reach::VASSZReachSolverOptions,
    validation::test_parikh_image,
};

#[test]
fn test_vass_z_reach_1() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', boxed_slice![1, 0]));
    vass.add_transition(q0, q1, ('b', boxed_slice![-2, 0]));
    vass.add_transition(q1, q1, ('b', boxed_slice![-1, 0]));

    let initialized_vass = vass.init(boxed_slice![0, 0], boxed_slice![0, 0], q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolverOptions::default()
        .with_time_limit(std::time::Duration::from_secs(5))
        .to_solver(
            cfg.clone(),
            initialized_vass.initial_valuation.clone(),
            initialized_vass.final_valuation.clone(),
        )
        .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(res.can_build_z_run(
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    ));
}

#[test]
fn test_vass_z_reach_2() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q0, ('a', boxed_slice![2, 0]));
    vass.add_transition(q0, q1, ('b', boxed_slice![-2, 0]));
    vass.add_transition(q1, q1, ('b', boxed_slice![-2, 0]));

    let initialized_vass = vass.init(boxed_slice![0, 0], boxed_slice![1, 0], q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolverOptions::default()
        .with_time_limit(std::time::Duration::from_secs(5))
        .to_solver(
            cfg.clone(),
            initialized_vass.initial_valuation.clone(),
            initialized_vass.final_valuation.clone(),
        )
        .solve();

    assert!(res.is_failure());
}

#[test]
fn test_vass_z_reach_3() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_state(0);
    let q1 = vass.add_state(1);

    vass.add_transition(q0, q1, ('a', boxed_slice![-1, 0]));
    vass.add_transition(q1, q1, ('b', boxed_slice![1, 0]));

    let initialized_vass = vass.init(boxed_slice![0, 0], boxed_slice![0, 0], q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolverOptions::default()
        .with_time_limit(std::time::Duration::from_secs(5))
        .to_solver(
            cfg.clone(),
            initialized_vass.initial_valuation.clone(),
            initialized_vass.final_valuation.clone(),
        )
        .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(res.can_build_z_run(
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    ));
}

#[test]
fn test_vass_z_reach_4() {
    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);

    let initialized_petri_net = petri_net.init(boxed_slice![1, 0, 0, 0], boxed_slice![0, 1, 0, 3]);

    let initialized_vass = initialized_petri_net.to_vass();
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolverOptions::default()
        .with_time_limit(std::time::Duration::from_secs(5))
        .to_solver(
            cfg.clone(),
            initialized_vass.initial_valuation.clone(),
            initialized_vass.final_valuation.clone(),
        )
        .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(res.can_build_z_run(
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    ));
}

#[test]
fn test_vass_z_reach_5() {
    let initialized_vass =
        InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json").to_vass();
    let mut cfg = initialized_vass.to_cfg();
    cfg.add_failure_state(());
    cfg = cfg.minimize();

    let res = VASSZReachSolverOptions::default()
        .with_time_limit(std::time::Duration::from_secs(5))
        .to_solver(
            cfg.clone(),
            initialized_vass.initial_valuation.clone(),
            initialized_vass.final_valuation.clone(),
        )
        .solve();

    assert!(res.is_failure());
}
