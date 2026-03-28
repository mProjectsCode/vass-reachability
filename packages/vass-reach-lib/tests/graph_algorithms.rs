use petgraph::graph::NodeIndex;
use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        algorithms::AutomatonAlgorithms,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        dfa::node::DfaNode,
        vass::{VASS, VASSEdge},
    },
    cfg_dec, cfg_inc,
};

#[test]
fn find_scc_1() {
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

    let mut cfg = initialized.to_cfg();
    cfg.remove_trapping_states();

    let mut scc0 = cfg.find_scc_surrounding(NodeIndex::new(12));
    scc0.sort();

    // println!("{}", cfg.to_graphviz(Some(scc0), None));

    assert_eq!(
        scc0,
        vec![
            NodeIndex::new(12),
            NodeIndex::new(13),
            NodeIndex::new(14),
            NodeIndex::new(15),
            NodeIndex::new(16)
        ]
    );
}

#[test]
fn find_scc_tree_1() {
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

    let tree = cfg.find_scc_tree();

    assert_eq!(tree.scc.nodes, vec![s0]);
    assert!(tree.scc.is_trivial());
    assert!(tree.scc.accepting_nodes.is_empty());
    assert_eq!(tree.children.len(), 1);
    assert_eq!(tree.children[0].path.transitions, vec![cfg_inc!(0)]);
    assert_eq!(tree.children[0].path.states, vec![s0, s1]);

    let first_scc = &tree.children[0].child;
    assert_eq!(first_scc.scc.nodes, vec![s1, s2]);
    assert!(!first_scc.scc.is_trivial());
    assert_eq!(first_scc.children.len(), 2);

    assert_eq!(first_scc.children[0].path.transitions, vec![cfg_dec!(0)]);
    assert_eq!(first_scc.children[0].path.states, vec![s1, s3]);
    assert_eq!(first_scc.children[0].child.scc.nodes, vec![s3]);
    assert_eq!(first_scc.children[0].child.scc.accepting_nodes, vec![s3]);

    assert_eq!(first_scc.children[1].path.transitions, vec![cfg_inc!(0)]);
    assert_eq!(first_scc.children[1].path.states, vec![s2, s4]);
    assert_eq!(first_scc.children[1].child.scc.nodes, vec![s4]);
    assert!(!first_scc.children[1].child.scc.is_trivial());
    assert_eq!(first_scc.children[1].child.children.len(), 1);
    assert_eq!(
        first_scc.children[1].child.children[0].path.transitions,
        vec![cfg_dec!(0)]
    );
    assert_eq!(
        first_scc.children[1].child.children[0].path.states,
        vec![s4, s5]
    );
    assert_eq!(
        first_scc.children[1].child.children[0]
            .child
            .scc
            .accepting_nodes,
        vec![s5]
    );
}
