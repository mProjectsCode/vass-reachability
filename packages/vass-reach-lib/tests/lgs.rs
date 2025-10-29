use vass_reach_lib::{
    automaton::{
        AutBuild, Automaton,
        dfa::{
            cfg::{CFGCounterUpdate, VASSCFG},
            node::DfaNode,
        },
        lsg::LinearSubGraph,
        path::Path,
    },
    cfg_dec, cfg_inc,
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

    let path = Path::new_from_sequence(s0, &[e1, e4], &cfg);

    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    // we assume the lsg has one path part
    assert_eq!(lsg.parts.len(), 1);
    assert!(lsg.parts[0].is_path());
    // we check that the path behaves as expected
    assert!(lsg.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0), cfg_dec!(0)]));

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

    let path = Path::new_from_sequence(s0, &[e1, e2], &cfg);
    let lsg = LinearSubGraph::from_path(path, &cfg, 1);

    // Initial path should have one part
    assert_eq!(lsg.parts.len(), 1);
    assert!(lsg.parts[0].is_path());

    assert!(lsg.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg.accepts(&[cfg_inc!(0), cfg_inc!(0), cfg_dec!(0)]));

    // we add node s2, this should successfully add the node and create a subgraph
    // part but not yet any looping behavior, as the loop requires s3 as well
    let lsg2 = lsg.add_node(s2);

    assert_eq!(lsg2.parts.len(), 3);
    assert!(lsg2.parts[0].is_path());
    assert!(lsg2.parts[1].is_subgraph());
    assert!(lsg2.parts[2].is_path());

    assert!(lsg2.accepts(&[cfg_inc!(0), cfg_dec!(0)]));
    assert!(!lsg2.accepts(&[cfg_inc!(0), cfg_inc!(0)]));

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
}
