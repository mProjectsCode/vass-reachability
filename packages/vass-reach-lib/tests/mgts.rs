use itertools::Itertools;
use petgraph::graph::NodeIndex;
use vass_reach_lib::{
    automaton::{
        Language, ModifiableAutomaton,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::node::DfaNode,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        mgts::{MGTS, extender::MGTSExtender, part::MarkedGraph},
        path::Path,
        vass::{VASS, VASSEdge},
    },
    cfg_dec, cfg_inc,
    solver::mgts_reach::MGTSReachSolverOptions,
    validation::same_language::assert_same_language,
};

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

fn assert_mgts_is_unreachable(mgts: &MGTS<'_, MultiGraphState, ImplicitCFGProduct>) {
    let res = MGTSReachSolverOptions::default()
        .to_solver(
            mgts,
            &mgts.automaton.initial_valuation,
            &mgts.automaton.final_valuation,
        )
        .solve();

    assert!(res.is_failure());
}

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
    let path = MultiGraphPath::from_word(product.initial(), &[cfg_inc!(0), cfg_dec!(0)], &product)
        .unwrap();
    let mgts = MGTS::from_path(path, &product, 1);

    // we assume the MGTS has one path part
    assert_eq!(mgts.sequence.len(), 1);
    assert!(mgts.sequence[0].is_path());
    // we check that the path behaves as expected
    assert!(mgts.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!mgts.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));

    let cfg = mgts.to_cfg();
    assert_same_language(&mgts, &cfg, 8);

    // now we add the node s2
    // we assume that the lgs now contains a graph part which allows it to accept
    // a wider range of inputs
    let mgts2 = mgts.add_node(s2.into());

    assert_eq!(mgts2.sequence.len(), 3);
    assert!(mgts2.sequence[0].is_path());
    assert!(mgts2.sequence[1].is_graph());
    assert!(mgts2.sequence[2].is_path());

    // we check that the MGTS now accepts more inputs
    assert!(mgts2.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));
    assert!(mgts2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0)
    ]));
    assert!(!mgts2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0)
    ]));
    assert!(!mgts2.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0),
        cfg_inc!(0)
    ]));

    let cfg2 = mgts2.to_cfg();
    assert_same_language(&mgts2, &cfg2, 8);
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
    let path = MultiGraphPath::from_word(product.initial(), &[cfg_inc!(0), cfg_dec!(0)], &product)
        .unwrap();
    let mgts = MGTS::from_path(path, &product, 1);

    // Initial path should have one part
    assert_eq!(mgts.sequence.len(), 1);
    assert!(mgts.sequence[0].is_path());

    assert!(mgts.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!mgts.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let cfg = mgts.to_cfg();
    assert_same_language(&mgts, &cfg, 8);

    // we add node s2, this should successfully add the node and create a graph
    // part but not yet any looping behavior, as the loop requires s3 as well
    let mgts2 = mgts.add_node(s2.into());

    assert_eq!(mgts2.sequence.len(), 3);
    assert!(mgts2.sequence[0].is_path());
    assert!(mgts2.sequence[1].is_graph());
    assert!(mgts2.sequence[2].is_path());

    assert!(mgts2.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!mgts2.accepts(&[cfg_inc!(0), cfg_inc!(0)]));

    let cfg2 = mgts2.to_cfg();
    assert_same_language(&mgts2, &cfg2, 8);

    // we add s3 to complete the loop
    let mgts3 = mgts2.add_node(s3.into());

    assert_eq!(mgts3.sequence.len(), 3);
    assert!(mgts3.sequence[0].is_path());
    assert!(mgts3.sequence[1].is_graph());
    assert!(mgts3.sequence[2].is_path());

    assert!(mgts3.accepts(&[cfg_inc!(0), cfg_dec!(0)]));

    // loop once: s0 -> s1 -> s2 -> s3 -> s1 -> s4
    assert!(mgts3.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0)
    ]));

    // loop twice: so -> s1 -> s2 -> s3 -> s1 -> s2 -> s3 -> s1 -> s4
    assert!(mgts3.accepts(&[
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
    assert!(!mgts3.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0)]));
    assert!(!mgts3.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let cfg3 = mgts3.to_cfg();
    assert_same_language(&mgts3, &cfg3, 8);
}

#[test]
fn mgts_3() {
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
    let path = MultiGraphPath::from_word(product.initial(), &word, &product).unwrap();
    let mgts = MGTS::from_path(path, &product, 2);

    mgts.add_node(NodeIndex::from(15).into());
    // In the crash, this panic-ed
    mgts.add_node(NodeIndex::from(11).into());

    assert!(mgts.accepts(&word));
}

#[test]
fn mgts_reach() {
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
    let path = MultiGraphPath::from_word(product.initial(), &[cfg_inc!(0), cfg_dec!(0)], &product)
        .unwrap();

    let mgts = MGTS::from_path(path, &product, 1);

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&mgts, false).is_some());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts, &vec![1].into(), &vec![0].into())
        .solve();

    assert!(res.is_failure());

    let mgts2 = mgts.add_node(s2.into());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts2, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&mgts2, false).is_some());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts2, &vec![1].into(), &vec![0].into())
        .solve();

    assert!(res.is_failure());
}

#[test]
fn add_scc_around_position_keeps_parts_connected() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    let _e0 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e1 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s2, &s1, cfg_dec!(0));
    let _e3 = cfg.add_edge(&s1, &s3, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let path = MultiGraphPath::from_word(product.initial(), &[cfg_inc!(0), cfg_dec!(0)], &product)
        .unwrap();
    let mgts = MGTS::from_path(path, &product, 1);

    let refined = mgts.add_scc_around_position(0, 1);
    refined.assert_consistent();

    assert_eq!(refined.sequence.len(), 3);
    assert!(refined.sequence[0].is_path());
    assert!(refined.sequence[1].is_graph());
    assert!(refined.sequence[2].is_path());

    assert!(refined.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(refined.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));

    let cfg = refined.to_cfg();
    assert_same_language(&refined, &cfg, 8);
}

#[test]
fn mgts_reach2() {
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
    let path = MultiGraphPath::from_word(product.initial(), &[cfg_inc!(0), cfg_dec!(0)], &product)
        .unwrap();

    let mgts = MGTS::from_path(path, &product, 1);

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&mgts, false).is_some());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts, &vec![0].into(), &vec![1].into())
        .solve();

    assert!(res.is_failure());

    let mgts2 = mgts.add_node(s2.into()).add_node(s3.into());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts2, &vec![0].into(), &vec![0].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&mgts2, false).is_some());

    let res = MGTSReachSolverOptions::default()
        .to_solver(&mgts2, &vec![0].into(), &vec![1].into())
        .solve();

    assert!(res.is_success());
    assert!(res.unwrap_success().build_run(&mgts2, false).is_some());
}

#[test]
fn mgts_determinize_invariant_to_scc_node_order() {
    // build a small CFG with an SCC (s1 <-> s2)
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

    // pick a node that lies inside the SCC (s1)
    let node: MultiGraphState = NodeIndex::from(1).into();
    let scc_set = product.find_scc_surrounding(node.clone());

    let scc_vec1: Vec<_> = scc_set.iter().cloned().collect_vec();
    let mut scc_vec2 = scc_vec1.clone();
    scc_vec2.reverse(); // different insertion order

    let g1 = MarkedGraph::from_subset(&product, &scc_vec1, node.clone(), node.clone());
    let g2 = MarkedGraph::from_subset(&product, &scc_vec2, node.clone(), node.clone());

    let mut l1 = MGTS::empty(&product, 1);
    l1.add_graph(g1.clone());
    let mut l2 = MGTS::empty(&product, 1);
    l2.add_graph(g2.clone());

    let nfa1 = l1.to_nfa();
    let nfa2 = l2.to_nfa();

    let cfg1 = nfa1.determinize();
    let cfg2 = nfa2.determinize();

    // determinized CFG/DFA sizes must be the same and languages equal
    assert_eq!(cfg1.graph.node_count(), cfg2.graph.node_count());
    assert_eq!(cfg1.graph.edge_count(), cfg2.graph.edge_count());

    assert_same_language(&cfg1, &cfg2, 8);
}

#[test]
fn mgts_from_path_roll_up_branch_specific() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));
    let s4 = cfg.add_node(DfaNode::non_accepting(()));
    let s5 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    let _e0 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e1 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s2, &s1, cfg_dec!(0));
    let _e3 = cfg.add_edge(&s1, &s3, cfg_dec!(0));
    let _e4 = cfg.add_edge(&s2, &s4, cfg_inc!(0));
    let _e5 = cfg.add_edge(&s4, &s4, cfg_inc!(0));
    let _e6 = cfg.add_edge(&s4, &s5, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let first_word = [cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)];
    let first_path = MultiGraphPath::from_word(product.initial(), &first_word, &product).unwrap();
    let first = MGTS::from_path_roll_up(first_path, &product, 1);

    assert_eq!(first.sequence.len(), 3);
    assert!(first.sequence[0].is_path());
    assert!(first.sequence[1].is_graph());
    assert!(first.sequence[2].is_path());
    assert!(first.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(first.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));
    assert!(!first.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    let second_word = [
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
    ];
    let second_path = MultiGraphPath::from_word(product.initial(), &second_word, &product).unwrap();
    let second = MGTS::from_path_roll_up(second_path, &product, 1);

    assert_eq!(second.sequence.len(), 5);
    assert!(second.sequence[0].is_path());
    assert!(second.sequence[1].is_graph());
    assert!(second.sequence[2].is_path());
    assert!(second.sequence[3].is_graph());
    assert!(second.sequence[4].is_path());
    assert!(second.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));
    assert!(second.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0)
    ]));
    assert!(!second.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
}

#[test]
fn mgts_extender_selects_full_scc_when_unreachable() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    cfg.add_edge(&s0, &s1, cfg_inc!(0));
    cfg.add_edge(&s1, &s3, cfg_dec!(0));
    cfg.add_edge(&s1, &s2, cfg_inc!(0));
    cfg.add_edge(&s2, &s1, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![1].into(), cfg);
    let word = [cfg_inc!(0), cfg_dec!(0)];
    let path = MultiGraphPath::from_word(product.initial(), &word, &product).unwrap();

    let mut extender = MGTSExtender::from_cfg_product(path, &product, 10);
    let mgts = extender.run_mgts();

    assert_mgts_is_unreachable(&mgts);
    assert!(mgts.accepts(&word));
    assert!(
        mgts.iter_graph_parts()
            .any(|graph| graph.graph.node_count() == 2)
    );
}

#[test]
fn mgts_extender_rejects_full_scc_when_reachable() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    cfg.add_edge(&s0, &s1, cfg_inc!(0));
    cfg.add_edge(&s1, &s3, cfg_dec!(0));
    cfg.add_edge(&s1, &s2, cfg_inc!(0));
    cfg.add_edge(&s2, &s1, cfg_inc!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![2].into(), cfg);
    let word = [cfg_inc!(0), cfg_dec!(0)];
    let path = MultiGraphPath::from_word(product.initial(), &word, &product).unwrap();

    let mut extender = MGTSExtender::from_cfg_product(path, &product, 10);
    let mgts = extender.run_mgts();

    assert_mgts_is_unreachable(&mgts);
    assert!(mgts.accepts(&word));
    assert!(mgts.iter_graph_parts().next().is_none());
}

#[test]
fn mgts_extender_drops_auxiliary_paths_with_different_dag_route() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(2));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    cfg.add_edge(&s0, &s1, cfg_inc!(0));
    cfg.add_edge(&s1, &s3, cfg_dec!(0));
    cfg.add_edge(&s0, &s2, cfg_inc!(1));
    cfg.add_edge(&s2, &s3, cfg_dec!(1));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(2, vec![0, 0].into(), vec![1, 0].into(), cfg);
    let first_word = [cfg_inc!(0), cfg_dec!(0)];
    let second_word = [cfg_inc!(1), cfg_dec!(1)];
    let first = MultiGraphPath::from_word(product.initial(), &first_word, &product).unwrap();
    let second = MultiGraphPath::from_word(product.initial(), &second_word, &product).unwrap();

    let mut extender = MGTSExtender::from_cfg_product_paths(vec![first, second], &product, 10);
    let mgts = extender.run_mgts();

    assert_mgts_is_unreachable(&mgts);
    assert!(mgts.accepts(&first_word));
    assert!(!mgts.accepts(&second_word));
    assert!(mgts.contains_state(&MultiGraphState::from(s1)));
    assert!(!mgts.contains_state(&MultiGraphState::from(s2)));
    assert!(mgts.iter_graph_parts().next().is_none());
}

#[test]
fn mgts_extender_merges_auxiliary_paths_on_same_dag_route() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(3));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let entry = cfg.add_node(DfaNode::non_accepting(()));
    let seed_extra = cfg.add_node(DfaNode::non_accepting(()));
    let full_extra = cfg.add_node(DfaNode::non_accepting(()));
    let accepting = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    cfg.add_edge(&s0, &entry, cfg_inc!(0));
    cfg.add_edge(&entry, &accepting, cfg_dec!(0));
    cfg.add_edge(&entry, &seed_extra, cfg_inc!(1));
    cfg.add_edge(&seed_extra, &entry, cfg_dec!(1));
    cfg.add_edge(&entry, &full_extra, cfg_inc!(2));
    cfg.add_edge(&full_extra, &entry, cfg_inc!(2));

    let product = ImplicitCFGProduct::new_without_counting_cfgs(
        3,
        vec![0, 0, 0].into(),
        vec![0, 0, 2].into(),
        cfg,
    );
    let primary_word = [cfg_inc!(0), cfg_dec!(0)];
    let auxiliary_word = [cfg_inc!(0), cfg_inc!(1), cfg_dec!(1), cfg_dec!(0)];
    let full_only_word = [cfg_inc!(0), cfg_inc!(2), cfg_inc!(2), cfg_dec!(0)];
    let primary = MultiGraphPath::from_word(product.initial(), &primary_word, &product).unwrap();
    let auxiliary =
        MultiGraphPath::from_word(product.initial(), &auxiliary_word, &product).unwrap();

    let mut extender = MGTSExtender::from_cfg_product_paths(vec![primary, auxiliary], &product, 10);
    let mgts = extender.run_mgts();

    assert_mgts_is_unreachable(&mgts);
    assert!(mgts.accepts(&primary_word));
    assert!(mgts.accepts(&auxiliary_word));
    assert!(!mgts.accepts(&full_only_word));
    assert!(mgts.contains_state(&MultiGraphState::from(seed_extra)));
    assert!(!mgts.contains_state(&MultiGraphState::from(full_extra)));
}

#[test]
fn mgts_extender_drops_auxiliary_paths_with_different_scc_sequence() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(2));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let primary_entry = cfg.add_node(DfaNode::non_accepting(()));
    let primary_extra = cfg.add_node(DfaNode::non_accepting(()));
    let auxiliary_entry = cfg.add_node(DfaNode::non_accepting(()));
    let auxiliary_extra = cfg.add_node(DfaNode::non_accepting(()));
    let accepting = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    cfg.add_edge(&s0, &primary_entry, cfg_inc!(0));
    cfg.add_edge(&primary_entry, &accepting, cfg_dec!(0));
    cfg.add_edge(&primary_entry, &primary_extra, cfg_inc!(0));
    cfg.add_edge(&primary_extra, &primary_entry, cfg_dec!(0));

    cfg.add_edge(&s0, &auxiliary_entry, cfg_inc!(1));
    cfg.add_edge(&auxiliary_entry, &accepting, cfg_dec!(1));
    cfg.add_edge(&auxiliary_entry, &auxiliary_extra, cfg_inc!(1));
    cfg.add_edge(&auxiliary_extra, &auxiliary_entry, cfg_dec!(1));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(2, vec![0, 0].into(), vec![1, 0].into(), cfg);
    let primary_word = [cfg_inc!(0), cfg_dec!(0)];
    let auxiliary_word = [cfg_inc!(1), cfg_dec!(1)];
    let primary = MultiGraphPath::from_word(product.initial(), &primary_word, &product).unwrap();
    let auxiliary =
        MultiGraphPath::from_word(product.initial(), &auxiliary_word, &product).unwrap();

    let mut extender =
        MGTSExtender::from_cfg_product_primary_path(primary, vec![auxiliary], &product, 10);
    let mgts = extender.run_mgts();

    assert_mgts_is_unreachable(&mgts);
    assert!(mgts.accepts(&primary_word));
    assert!(mgts.contains_state(&MultiGraphState::from(primary_entry)));
    assert!(mgts.contains_state(&MultiGraphState::from(primary_extra)));
    assert!(!mgts.contains_state(&MultiGraphState::from(auxiliary_entry)));
    assert!(!mgts.contains_state(&MultiGraphState::from(auxiliary_extra)));
}

#[test]
fn mgts_from_path_roll_up() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(1));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::non_accepting(()));
    let s3 = cfg.add_node(DfaNode::non_accepting(()));
    let s4 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);

    let _e1 = cfg.add_edge(&s0, &s1, cfg_inc!(0));
    let _e2 = cfg.add_edge(&s1, &s4, cfg_dec!(0));
    let _e3 = cfg.add_edge(&s1, &s2, cfg_inc!(0));
    let _e4 = cfg.add_edge(&s2, &s3, cfg_inc!(0));
    let _e5 = cfg.add_edge(&s3, &s1, cfg_dec!(0));

    let product =
        ImplicitCFGProduct::new_without_counting_cfgs(1, vec![0].into(), vec![0].into(), cfg);
    let word = [
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0),
    ];
    let path = MultiGraphPath::from_word(product.initial(), &word, &product).unwrap();

    let mgts = MGTS::from_path_roll_up(path, &product, 1);

    assert_eq!(mgts.sequence.len(), 3);
    assert!(mgts.sequence[0].is_path());
    assert!(mgts.sequence[1].is_graph());
    assert!(mgts.sequence[2].is_path());

    assert!(mgts.accepts(&word));
    assert!(mgts.accepts(&[
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_inc!(0),
        cfg_inc!(0),
        cfg_dec!(0),
        cfg_dec!(0),
    ]));
    assert!(mgts.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!mgts.accepts(&[cfg_inc!(0), cfg_inc!(0)]));
}

#[test]
fn mgts_from_path_roll_up_with_disabled_bounded_counting_keeps_trivial_path_states() {
    let mut cfg = VASSCFG::<()>::new(CFGCounterUpdate::alphabet(2));
    let s0 = cfg.add_node(DfaNode::non_accepting(()));
    let s1 = cfg.add_node(DfaNode::non_accepting(()));
    let s2 = cfg.add_node(DfaNode::accepting(()));

    cfg.set_initial(s0);
    cfg.add_edge(&s0, &s1, cfg_inc!(0));
    cfg.add_edge(&s1, &s2, cfg_dec!(0));

    let product = ImplicitCFGProduct::new(2, vec![0, 0].into(), vec![0, 0].into(), cfg, false);
    let word = [cfg_inc!(0), cfg_dec!(0)];
    let path = MultiGraphPath::from_word(product.initial(), &word, &product).unwrap();

    let mgts = MGTS::from_path_roll_up(path, &product, 2);

    assert_eq!(mgts.sequence.len(), 1);
    assert!(mgts.sequence[0].is_path());
    assert!(mgts.accepts(&word));
}
