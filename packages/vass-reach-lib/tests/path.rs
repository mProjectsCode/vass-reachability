#![allow(clippy::just_underscores_and_digits)]

use std::vec;

use petgraph::graph::{EdgeIndex, NodeIndex};
use vass_reach_lib::automaton::{
    Automaton,
    dfa::cfg::CFGCounterUpdate,
    ltc::translation::LTCTranslation,
    path::{Path, path_like::PathLike},
};

#[test]
fn to_ltc() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add(EdgeIndex::new(0), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(2));
    path.add(EdgeIndex::new(2), NodeIndex::new(3));
    path.add(EdgeIndex::new(3), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(2));
    path.add(EdgeIndex::new(2), NodeIndex::new(3));
    path.add(EdgeIndex::new(3), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(2));
    path.add(EdgeIndex::new(4), NodeIndex::new(4));

    let edge_weight = |edge: EdgeIndex<u32>| CFGCounterUpdate::new(edge.index() as u32, true);
    let word = path.to_word(edge_weight);
    let translation = LTCTranslation::from(&path);

    let ltc = translation.to_ltc(5, edge_weight);
    assert_eq!(ltc.elements.len(), 3);

    let dfa = translation.to_dfa(false, 5, edge_weight);
    assert!(!dfa.accepts(&word));
}

#[test]
fn to_ltc_2() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add(EdgeIndex::new(0), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(1));

    let edge_weight = |edge: EdgeIndex<u32>| CFGCounterUpdate::new(edge.index() as u32, true);
    let word = path.to_word(edge_weight);
    let translation = LTCTranslation::from(&path);

    let ltc = translation.to_ltc(2, edge_weight);
    assert_eq!(ltc.elements.len(), 2);

    let dfa = translation.to_dfa(false, 2, edge_weight);
    assert!(!dfa.accepts(&word));
}

#[test]
fn to_ltc_3() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add(EdgeIndex::new(0), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(1));
    path.add(EdgeIndex::new(1), NodeIndex::new(1));

    let edge_weight = |edge: EdgeIndex<u32>| CFGCounterUpdate::new(edge.index() as u32, true);
    let translation = LTCTranslation::from(&path);

    let ltc = translation.to_ltc(2, edge_weight);
    assert_eq!(ltc.elements.len(), 2);

    let dfa = translation.to_dfa(false, 2, edge_weight);

    let _0 = CFGCounterUpdate::new(0, true);
    let _1 = CFGCounterUpdate::new(1, true);

    assert!(!dfa.accepts(&vec![_0]));
    assert!(!dfa.accepts(&vec![_0, _1]));
    assert!(!dfa.accepts(&vec![_0, _1, _1]));
    assert!(!dfa.accepts(&vec![_0, _1, _1, _1]));
    assert!(dfa.accepts(&vec![_1]));
}
