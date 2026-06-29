use std::time::Duration;

use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        vass::{VASS, VASSEdge},
    },
    config::{PreprocessingConfig, ShortWitnessConfig, VASSReachConfig},
    solver::{SolverStatus, vass_reach::VASSReachSolver},
};

#[test]
fn preprocessing_reuses_concrete_linear_graph_run() {
    let mut vass = VASS::new(1, (0..2).collect());
    let initial = vass.add_node(());
    let middle = vass.add_node(());
    let target = vass.add_node(());
    vass.add_edge(&initial, &middle, VASSEdge::new(0, vec![1].into()));
    vass.add_edge(&middle, &target, VASSEdge::new(1, vec![-1].into()));
    let instance = vass.init(vec![0].into(), vec![0].into(), initial, target);

    let result = VASSReachSolver::new(
        &instance,
        VASSReachConfig::default()
            .with_max_iterations(Some(0))
            .with_short_witness(ShortWitnessConfig::default().with_enabled(false))
            .with_preprocessing(PreprocessingConfig::default().with_enabled(true)),
    )
    .solve();

    assert!(result.is_success(), "{:?}", result.status);
    assert_eq!(result.statistics.step_count, 0);
}

#[test]
fn preprocessing_obeys_global_timeout() {
    let mut vass = VASS::new(1, (0..2).collect());
    let initial = vass.add_node(());
    let target = vass.add_node(());
    vass.add_edge(&initial, &target, VASSEdge::new(0, vec![1].into()));
    let instance = vass.init(vec![0].into(), vec![1].into(), initial, target);

    let result = VASSReachSolver::new(
        &instance,
        VASSReachConfig::default()
            .with_timeout(Some(Duration::ZERO))
            .with_short_witness(ShortWitnessConfig::default().with_enabled(false))
            .with_preprocessing(PreprocessingConfig::default().with_enabled(true)),
    )
    .solve();

    assert!(matches!(
        result.status,
        SolverStatus::Unknown(vass_reach_lib::solver::vass_reach::VASSReachSolverError::Timeout)
    ));
    assert_eq!(result.statistics.step_count, 0);
}

#[test]
fn preprocessing_disabled_still_solves_reachable_instance() {
    let mut vass = VASS::<u32, char>::new(1, vec!['a']);
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);
    vass.add_edge(&q0, &q1, VASSEdge::new('a', vec![0].into()));

    let instance = vass.init(vec![0].into(), vec![0].into(), q0, q1);
    let result = VASSReachSolver::new(
        &instance,
        VASSReachConfig::default()
            .with_timeout(Some(Duration::from_secs(5)))
            .with_preprocessing(PreprocessingConfig::default().with_enabled(false)),
    )
    .solve();

    assert!(result.is_success(), "{:?}", result.status);
}

#[test]
fn preprocessing_proves_difficult_instance_unreachable() {
    let mut vass = VASS::new(2, (0..10).collect());

    let s0 = vass.add_node(());
    let s1 = vass.add_node(());
    let s2 = vass.add_node(());
    let s3 = vass.add_node(());

    vass.add_edge(&s0, &s1, VASSEdge::new(0, vec![6, 0].into()));

    vass.add_edge(&s1, &s1, VASSEdge::new(1, vec![1, 1].into()));
    vass.add_edge(&s1, &s1, VASSEdge::new(2, vec![-1, -1].into()));
    vass.add_edge(&s1, &s1, VASSEdge::new(3, vec![1, 0].into()));

    vass.add_edge(&s1, &s2, VASSEdge::new(4, vec![0, 0].into()));

    vass.add_edge(&s2, &s2, VASSEdge::new(5, vec![1, 2].into()));
    vass.add_edge(&s2, &s2, VASSEdge::new(6, vec![-1, -2].into()));

    vass.add_edge(&s2, &s3, VASSEdge::new(7, vec![0, 0].into()));

    vass.add_edge(&s3, &s3, VASSEdge::new(8, vec![0, 1].into()));
    vass.add_edge(&s3, &s3, VASSEdge::new(9, vec![0, -1].into()));

    let instance = vass.init(vec![0, 0].into(), vec![0, 0].into(), s0, s3);
    let result = VASSReachSolver::new(
        &instance,
        VASSReachConfig::default()
            .with_timeout(Some(Duration::from_secs(5)))
            .with_preprocessing(PreprocessingConfig::default().with_enabled(true)),
    )
    .solve();

    assert!(result.is_failure(), "{:?}", result.status);
}
