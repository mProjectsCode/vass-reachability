use vass_reach_lib::{
    automaton::{
        AutBuild, Automaton,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::node::DfaNode,
        lsg::LinearSubGraph,
        path::Path,
    },
    cfg_dec, cfg_inc,
    solver::lsg_reach::LSGReachSolverOptions,
    validation::same_language::assert_same_language,
};

#[test]
fn lgs_1() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_state(DfaNode::non_accepting(()));
    let s1 = cfg.add_state(DfaNode::non_accepting(()));
    let s2 = cfg.add_state(DfaNode::non_accepting(()));
    let s3 = cfg.add_state(DfaNode::accepting(()));

    let e1 = cfg.add_transition(s0, s1, cfg_inc!(0));
    let _e2 = cfg.add_transition(s1, s2, cfg_inc!(0));
    let _e3 = cfg.add_transition(s2, s1, cfg_dec!(0));
    let e4 = cfg.add_transition(s1, s3, cfg_dec!(0));

    let path = Path::new_from_iterator(s0, &[e1, e4], &cfg);

    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    // we assume the lsg has one path part
    assert_eq!(lsg.parts.len(), 1);
    assert!(lsg.parts[0].is_path());
    // we check that the path behaves as expected
    assert!(lsg.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));

    let cfg = lsg.to_cfg();
    assert_same_language(&lsg, &cfg, 8);

    // now we add the node s2
    // we assume that the lgs now contains a subgraph part which allows it to accept
    // a wider range of inputs
    let lsg2 = lsg.add_node(s2);

    assert_eq!(lsg2.parts.len(), 3);
    assert!(lsg2.parts[0].is_path());
    assert!(lsg2.parts[1].is_subgraph());
    assert!(lsg2.parts[2].is_path());

    // we check that the lsg now accepts more inputs
    assert!(lsg2.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));
    assert!(lsg2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0)
    ]));
    assert!(!lsg2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0)
    ]));
    assert!(!lsg2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0),
        cfg_inc!(0)
    ]));

    let cfg2 = lsg2.to_cfg();
    assert_same_language(&lsg2, &cfg2, 8);
}

#[test]
fn lgs_2() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_state(DfaNode::non_accepting(()));
    let s1 = cfg.add_state(DfaNode::non_accepting(()));
    let s2 = cfg.add_state(DfaNode::non_accepting(()));
    let s3 = cfg.add_state(DfaNode::non_accepting(()));
    let s4 = cfg.add_state(DfaNode::accepting(()));

    // direct path "s0 -> s1 -> s4" with a loop in s1 "s1 -> s2 -> s3 -> s1"
    let e1 = cfg.add_transition(s0, s1, cfg_inc!(0));
    let e2 = cfg.add_transition(s1, s4, cfg_dec!(0));
    let _e3 = cfg.add_transition(s1, s2, cfg_inc!(0));
    let _e4 = cfg.add_transition(s2, s3, cfg_inc!(0));
    let _e5 = cfg.add_transition(s3, s1, cfg_dec!(0));

    let path = Path::new_from_iterator(s0, &[e1, e2], &cfg);
    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    // Initial path should have one part
    assert_eq!(lsg.parts.len(), 1);
    assert!(lsg.parts[0].is_path());

    assert!(lsg.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let cfg = lsg.to_cfg();
    assert_same_language(&lsg, &cfg, 8);

    // we add node s2, this should successfully add the node and create a subgraph
    // part but not yet any looping behavior, as the loop requires s3 as well
    let lsg2 = lsg.add_node(s2);

    assert_eq!(lsg2.parts.len(), 3);
    assert!(lsg2.parts[0].is_path());
    assert!(lsg2.parts[1].is_subgraph());
    assert!(lsg2.parts[2].is_path());

    assert!(lsg2.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg2.accepts(&[cfg_inc!(0), cfg_inc!(0)]));

    let cfg2 = lsg2.to_cfg();
    assert_same_language(&lsg2, &cfg2, 8);

    // we add s3 to complete the loop
    let lsg3 = lsg2.add_node(s3);

    assert_eq!(lsg3.parts.len(), 3);
    assert!(lsg3.parts[0].is_path());
    assert!(lsg3.parts[1].is_subgraph());
    assert!(lsg3.parts[2].is_path());

    assert!(lsg3.accepts(&[cfg_inc!(0), cfg_dec!(0)]));

    // loop once: s0 -> s1 -> s2 -> s3 -> s1 -> s4
    assert!(lsg3.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0)
    ]));

    // loop twice: so -> s1 -> s2 -> s3 -> s1 -> s2 -> s3 -> s1 -> s4
    assert!(lsg3.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0)
    ]));

    // we still reject other sequences
    assert!(!lsg3.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0)]));
    assert!(!lsg3.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let cfg3 = lsg3.to_cfg();
    assert_same_language(&lsg3, &cfg3, 8);
}

#[test]
fn lsg_reach() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_state(DfaNode::non_accepting(()));
    let s1 = cfg.add_state(DfaNode::non_accepting(()));
    let s2 = cfg.add_state(DfaNode::non_accepting(()));
    let s3 = cfg.add_state(DfaNode::accepting(()));

    cfg.set_start(s0);

    let e1 = cfg.add_transition(s0, s1, cfg_inc!(0));
    let _e2 = cfg.add_transition(s1, s2, cfg_inc!(0));
    let _e3 = cfg.add_transition(s2, s1, cfg_dec!(0));
    let e4 = cfg.add_transition(s1, s3, cfg_dec!(0));

    let path = Path::new_from_iterator(s0, &[e1, e4], &cfg);

    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![1].into(), &vec![0].into())
        .solve();

    assert!(res.is_failure());

    let lsg2 = lsg.add_node(s2);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg2, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg2, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg2, &vec![1].into(), &vec![0].into())
        .solve();

    assert!(res.is_failure());
}

#[test]
fn lsg_reach2() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_state(DfaNode::non_accepting(()));
    let s1 = cfg.add_state(DfaNode::non_accepting(()));
    let s2 = cfg.add_state(DfaNode::non_accepting(()));
    let s3 = cfg.add_state(DfaNode::non_accepting(()));
    let s4 = cfg.add_state(DfaNode::accepting(()));

    cfg.set_start(s0);

    // direct path "s0 -> s1 -> s4" with a loop in s1 "s1 -> s2 -> s3 -> s1"
    let e1 = cfg.add_transition(s0, s1, cfg_inc!(0));
    let e2 = cfg.add_transition(s1, s4, cfg_dec!(0));
    let _e3 = cfg.add_transition(s1, s2, cfg_inc!(0));
    let _e4 = cfg.add_transition(s2, s3, cfg_inc!(0));
    let _e5 = cfg.add_transition(s3, s1, cfg_dec!(0));

    let path = Path::new_from_iterator(s0, &[e1, e2], &cfg);
    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![1].into())
        .solve();

    assert!(res.is_failure());

    let lsg2 = lsg.add_node(s2).add_node(s3);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg2, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg2, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg2, &vec![0].into(), &vec![1].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg2, false).is_some());
}
