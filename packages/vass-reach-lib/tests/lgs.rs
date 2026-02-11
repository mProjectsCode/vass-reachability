use petgraph::graph::NodeIndex;
use vass_reach_lib::{
    automaton::{
        Language, ModifiableAutomaton,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::node::DfaNode,
        implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath},
        lsg::LinearSubGraph,
        vass::{VASS, VASSEdge},
    },
    cfg_dec, cfg_inc,
    solver::lsg_reach::LSGReachSolverOptions,
    validation::same_language::assert_same_language,
};

#[test]
fn lgs_1() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    let _e1 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e3 = cfg.add_edge(&s2, &s1, cfg_dec!(0));
    let _e4 = cfg.add_edge(&s1, &s3, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), [cfg_inc!(0), cfg_dec!(0)], &product);
    let lsg = LinearSubGraph::from_path(path, &product, 1);

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
    let lsg2 = lsg.add_node(s2.into());

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
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::non_accepting(()));
    let s4 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    // direct path "s0 -> s1 -> s4" with a loop in s1 "s1 -> s2 -> s3 -> s1"
    let _e1 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s1, &s4, cfg_dec!(0));
    let _e3 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e4 = cfg.add_edge(&s2, &s3, cfg_inc!(0));
    let _e5 = cfg.add_edge(&s3, &s1, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), [cfg_inc!(0), cfg_dec!(0)], &product);
    let lsg = LinearSubGraph::from_path(path, &product, 1);

    // Initial path should have one part
    assert_eq!(lsg.parts.len(), 1);
    assert!(lsg.parts[0].is_path());

    assert!(lsg.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let cfg = lsg.to_cfg();
    assert_same_language(&lsg, &cfg, 8);

    // we add node s2, this should successfully add the node and create a subgraph
    // part but not yet any looping behavior, as the loop requires s3 as well
    let lsg2 = lsg.add_node(s2.into());

    assert_eq!(lsg2.parts.len(), 3);
    assert!(lsg2.parts[0].is_path());
    assert!(lsg2.parts[1].is_subgraph());
    assert!(lsg2.parts[2].is_path());

    assert!(lsg2.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg2.accepts(&[cfg_inc!(0), cfg_inc!(0)]));

    let cfg2 = lsg2.to_cfg();
    assert_same_language(&lsg2, &cfg2, 8);

    // we add s3 to complete the loop
    let lsg3 = lsg2.add_node(s3.into());

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
fn lsg_3() {
    // Note: this test is from a crash
    let mut vass = VASS::new(2, (0..10).collect());

    let s0 = vass.add_node(());
    let s1 = vass.add_node(());
    let s2 = vass.add_node(());
    let s3 = vass.add_node(());

    let _e0 = vass.add_edge(&s0, &s1, VASSEdge::new(0, vec![6, 0].into()));

    let _e1 = vass.add_edge(&s1, &s1, VASSEdge::new(1, vec![1, 1].into()));
    let _e2 = vass.add_edge(&s1, &s1, VASSEdge::new(2, vec![-1, -1].into()));
    let _e3 = vass.add_edge(&s1, &s1, VASSEdge::new(3, vec![1, 0].into()));

    let _e4 = vass.add_edge(&s1, &s2, VASSEdge::new(4, vec![0, 0].into()));

    let _e5 = vass.add_edge(&s2, &s2, VASSEdge::new(5, vec![1, 2].into()));
    let _e6 = vass.add_edge(&s2, &s2, VASSEdge::new(6, vec![-1, -2].into()));

    let _e7 = vass.add_edge(&s2, &s3, VASSEdge::new(7, vec![0, 0].into()));

    let _e8 = vass.add_edge(&s3, &s3, VASSEdge::new(8, vec![0, 1].into()));
    let _e9 = vass.add_edge(&s3, &s3, VASSEdge::new(9, vec![0, -1].into()));

    let initialized = vass.init(vec![0, 0].into(), vec![0, 0].into(), s0, s3);

    let cfg = initialized.to_cfg();
    let word = CFGCounterUpdate::from_str_to_vec("+c0 +c0 +c0 +c0 +c0 +c0 +c0 +c0 +c0 +c0 +c1 +c0 +c1 +c0 +c1 +c0 +c1 -c0 -c1 -c0 -c1 -c0 -c1 -c1").unwrap();

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(2, vec![0, 0].into(), vec![0, 0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), word.clone(), &product);
    let lsg = LinearSubGraph::from_path(path, &product, 2);

    lsg.add_node(NodeIndex::from(15).into());
    // In the crash, this panic-ed
    lsg.add_node(NodeIndex::from(11).into());

    assert!(lsg.accepts(&word));
}

#[test]
fn lsg_reach() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    let _e1 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e3 = cfg.add_edge(&s2, &s1, cfg_dec!(0));
    let _e4 = cfg.add_edge(&s1, &s3, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), [cfg_inc!(0), cfg_dec!(0)], &product);

    let lsg = LinearSubGraph::from_path(path, &product, 1);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![1].into(), &vec![0].into())
        .solve();

    assert!(res.is_failure());

    let lsg2 = lsg.add_node(s2.into());

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
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::non_accepting(()));
    let s4 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    // direct path "s0 -> s1 -> s4" with a loop in s1 "s1 -> s2 -> s3 -> s1"
    let _e1 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s1, &s4, cfg_dec!(0));
    let _e3 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e4 = cfg.add_edge(&s2, &s3, cfg_inc!(0));
    let _e5 = cfg.add_edge(&s3, &s1, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), [cfg_inc!(0), cfg_dec!(0)], &product);

    let lsg = LinearSubGraph::from_path(path, &product, 1);

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&lsg, false).is_some());

    let res = LSGReachSolverOptions::default()
        .to_solver(&lsg, &vec![0].into(), &vec![1].into())
        .solve();

    assert!(res.is_failure());

    let lsg2 = lsg.add_node(s2.into()).add_node(s3.into());

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
