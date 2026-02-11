use std::vec;

use vass_reach_lib::automaton::{
    Language, ModifiableAutomaton,
    vass::{VASS, VASSEdge},
};

#[test]
fn test_vass() {
    let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);

    vass.add_edge(&q0, &q0, VASSEdge::new('a', vec![1, 0].into()));
    vass.add_edge(&q0, &q1, VASSEdge::new('b', vec![-1, 0].into()));
    vass.add_edge(&q1, &q1, VASSEdge::new('b', vec![-1, 0].into()));

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
    let q0 = vass.add_node(0);
    let q1 = vass.add_node(1);

    vass.add_edge(&q0, &q0, VASSEdge::new('a', vec![1, 0].into()));
    vass.add_edge(&q0, &q1, VASSEdge::new('b', vec![-2, 0].into()));
    vass.add_edge(&q1, &q1, VASSEdge::new('b', vec![-1, 0].into()));

    let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(), q0, q1);

    let _cfg = initialized_vass.to_cfg();
}

// #[test]
// fn test_vass_to_vas_1() {
//     let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
//     let q0 = vass.add_state(0);
//     let q1 = vass.add_state(1);

//     vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
//     vass.add_transition(q0, q1, ('b', vec![-2, 0].into()));
//     vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

//     let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(),
// q0, q1);

//     let initialized_vas = initialized_vass.to_vas();

//     let vass_res = VASSReachSolver::new(
//         &initialized_vass,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     let vas_res = VASSReachSolver::new(
//         &initialized_vas,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     assert_eq!(vass_res.status, vas_res.status);
// }

// #[test]
// fn test_vass_to_vas_2() {
//     let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
//     let q0 = vass.add_state(0);
//     let q1 = vass.add_state(1);

//     vass.add_transition(q0, q0, ('a', vec![1, 0].into()));
//     vass.add_transition(q0, q1, ('b', vec![0, 1].into()));
//     vass.add_transition(q1, q1, ('b', vec![-1, 0].into()));

//     let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(),
// q0, q1);     let initialized_vas = initialized_vass.to_vas();

//     let vass_res = VASSReachSolver::new(
//         &initialized_vass,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     let vas_res = VASSReachSolver::new(
//         &initialized_vas,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     assert_eq!(vass_res.status, vas_res.status);
// }

// #[test]
// fn test_vass_to_vas_3() {
//     let mut vass = VASS::<u32, char>::new(2, vec!['a', 'b']);
//     let q0 = vass.add_state(0);
//     let q1 = vass.add_state(1);

//     vass.add_transition(q0, q1, ('a', vec![-1, 0].into()));
//     vass.add_transition(q1, q1, ('b', vec![1, 0].into()));

//     let initialized_vass = vass.init(vec![0, 0].into(), vec![0, 0].into(),
// q0, q1);

//     let initialized_vas = initialized_vass.to_vas();

//     let vass_res = VASSReachSolver::new(
//         &initialized_vass,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     let vas_res = VASSReachSolver::new(
//         &initialized_vas,
//         // some time that is long enough, but makes the test run in a
// reasonable time         VASSReachConfig::default().
// with_timeout(Some(Duration::from_secs(5))),         None,
//     )
//     .solve();

//     assert_eq!(vass_res.status, vas_res.status);
// }
