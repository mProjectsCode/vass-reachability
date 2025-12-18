use petgraph::graph::NodeIndex;
use vass_reach_lib::automaton::{
    ModifiableAutomaton,
    algorithms::AutomatonAlgorithms,
    vass::{VASS, VASSEdge},
};

#[test]
fn find_scc_1() {
    let mut vass = VASS::new(2, (0..10).collect());

    let s0 = vass.add_node(());
    let s1 = vass.add_node(());
    let s2 = vass.add_node(());
    let s3 = vass.add_node(());

    let _e0 = vass.add_edge(s0, s1, VASSEdge::new(0, vec![6, 0].into()));

    let _e1 = vass.add_edge(s1, s1, VASSEdge::new(1, vec![1, 1].into()));
    let _e2 = vass.add_edge(s1, s1, VASSEdge::new(2, vec![-1, -1].into()));
    let _e3 = vass.add_edge(s1, s1, VASSEdge::new(3, vec![1, 0].into()));

    let _e4 = vass.add_edge(s1, s2, VASSEdge::new(4, vec![0, 0].into()));

    let _e5 = vass.add_edge(s2, s2, VASSEdge::new(5, vec![1, 2].into()));
    let _e6 = vass.add_edge(s2, s2, VASSEdge::new(6, vec![-1, -2].into()));

    let _e7 = vass.add_edge(s2, s3, VASSEdge::new(7, vec![0, 0].into()));

    let _e8 = vass.add_edge(s3, s3, VASSEdge::new(8, vec![0, 1].into()));
    let _e9 = vass.add_edge(s3, s3, VASSEdge::new(9, vec![0, -1].into()));

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
