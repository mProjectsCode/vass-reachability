use vass_reach_lib::automaton::{
    ModifiableAutomaton,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    karp_miller::build_karp_miller_coverability_tree,
    vass::omega::OmegaCounter,
};

#[test]
fn test_karp_miller_accelerates_to_omega_on_increasing_cycle() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q0, CFGCounterUpdate::new(0, true));

    let tree = build_karp_miller_coverability_tree(&cfg, &vec![0].into());

    assert_eq!(tree.root(), 0);
    assert_eq!(tree.nodes().len(), 2);

    let child = tree.node(1);
    assert!(child.closed);
    assert_eq!(child.valuation.values(), [OmegaCounter::Omega]);
}

#[test]
fn test_karp_miller_skips_disabled_decrement() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q0, CFGCounterUpdate::new(0, false));

    let tree = build_karp_miller_coverability_tree(&cfg, &vec![0].into());

    assert_eq!(tree.nodes().len(), 1);
    assert!(tree.node(0).children.is_empty());
}

#[test]
fn test_karp_miller_compares_ancestors_with_same_control() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    let q1 = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(q0);

    cfg.add_edge(&q0, &q1, CFGCounterUpdate::new(0, true));
    cfg.add_edge(&q1, &q0, CFGCounterUpdate::new(0, false));

    let tree = build_karp_miller_coverability_tree(&cfg, &vec![0].into());

    assert_eq!(tree.nodes().len(), 3);
    assert!(!tree.node(1).closed);
    assert!(tree.node(2).closed);
    assert_eq!(tree.node(2).valuation.values(), [OmegaCounter::Finite(0)]);
}

#[test]
fn test_karp_miller_to_graphviz_contains_nodes_and_edges() {
    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(1));
    let q0 = cfg.add_node(DfaNode::non_accepting(()));
    cfg.set_initial(q0);
    cfg.add_edge(&q0, &q0, CFGCounterUpdate::new(0, true));

    let tree = build_karp_miller_coverability_tree(&cfg, &vec![0].into());
    let dot = tree.to_graphviz();

    assert!(dot.contains("digraph karp_miller_tree"));
    assert!(dot.contains("n0"));
    assert!(dot.contains("n0 -> n1"));
    assert!(dot.contains("label=\"+c0\""));
}
