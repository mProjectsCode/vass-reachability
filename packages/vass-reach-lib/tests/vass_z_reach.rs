use std::time::Duration;

use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        dfa::minimization::Minimizable,
        petri_net::{PetriNet, initialized::InitializedPetriNet},
        vass::{VASS, VASSEdge},
    },
    config::VASSZReachConfig,
    solver::vass_z_reach::VASSZReachSolver,
    validation::test_parikh_image,
};

#[test]
fn test_vass_z_reach_1() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);

    vass.add_edge(&q0, &q0, VASSEdge::new('a', vec![1, 0].into()));
    vass.add_edge(&q0, &q1, VASSEdge::new('b', vec![-2, 0].into()));
    vass.add_edge(&q1, &q1, VASSEdge::new('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolver::new(
        &cfg,
        initialized_vass.initial_valuation.clone(),
        initialized_vass.final_valuation.clone(),
        VASSZReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    )
    .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(
        res.build_run(
            &cfg,
            &initialized_vass.initial_valuation,
            &initialized_vass.final_valuation,
            false,
        )
        .is_some()
    );
}

#[test]
fn test_vass_z_reach_2() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);

    vass.add_edge(&q0, &q0, VASSEdge::new('a', vec![2, 0].into()));
    vass.add_edge(&q0, &q1, VASSEdge::new('b', vec![-2, 0].into()));
    vass.add_edge(&q1, &q1, VASSEdge::new('b', vec![-2, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![1, 0].into(), q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolver::new(
        &cfg,
        initialized_vass.initial_valuation.clone(),
        initialized_vass.final_valuation.clone(),
        VASSZReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    )
    .solve();

    assert!(res.is_failure());
}

#[test]
fn test_vass_z_reach_3() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);

    vass.add_edge(&q0, &q1, VASSEdge::new('a', vec![-1, 0].into()));
    vass.add_edge(&q1, &q1, VASSEdge::new('b', vec![1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolver::new(
        &cfg,
        initialized_vass.initial_valuation.clone(),
        initialized_vass.final_valuation.clone(),
        VASSZReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    )
    .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(
        res.build_run(
            &cfg,
            &initialized_vass.initial_valuation,
            &initialized_vass.final_valuation,
            false,
        )
        .is_some()
    );
}

#[test]
fn test_vass_z_reach_4() {
    let mut petri_net = PetriNet::new(4);

    petri_net.add_transition(vec![(1, 1)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 3)], vec![(1, 2)]);
    petri_net.add_transition(vec![(1, 2)], vec![(1, 3), (1, 4)]);

    let initialized_petri_net = petri_net.init(vec![1, 0, 0, 0].into(), vec![0, 1, 0, 3].into());

    let initialized_vass = initialized_petri_net.to_vass();
    let cfg = initialized_vass.to_cfg();

    let res = VASSZReachSolver::new(
        &cfg,
        initialized_vass.initial_valuation.clone(),
        initialized_vass.final_valuation.clone(),
        VASSZReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    )
    .solve();

    assert!(res.is_success());

    test_parikh_image(
        res.get_parikh_image().unwrap(),
        &cfg,
        &initialized_vass.initial_valuation,
        &initialized_vass.final_valuation,
    );

    assert!(
        res.build_run(
            &cfg,
            &initialized_vass.initial_valuation,
            &initialized_vass.final_valuation,
            false,
        )
        .is_some()
    );
}

#[test]
fn test_vass_z_reach_5() {
    let initialized_vass = InitializedPetriNet::from_file("test_data/petri_nets/3/unknown_2.json")
        .unwrap()
        .to_vass();
    let mut cfg = initialized_vass.to_cfg();
    cfg.make_complete(());
    cfg = cfg.minimize();

    let res = VASSZReachSolver::new(
        &cfg,
        initialized_vass.initial_valuation.clone(),
        initialized_vass.final_valuation.clone(),
        VASSZReachConfig::default().with_timeout(Some(Duration::from_secs(5))),
    )
    .solve();

    assert!(res.is_failure());
}
