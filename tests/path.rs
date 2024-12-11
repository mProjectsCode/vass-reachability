use petgraph::graph::{EdgeIndex, NodeIndex};
use vass_reachability::automaton::{ltc::LTCTranslation, path::Path, Automaton};

#[test]
fn to_ltc() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add_edge(EdgeIndex::new(0), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(2));
    path.add_edge(EdgeIndex::new(2), NodeIndex::new(3));
    path.add_edge(EdgeIndex::new(3), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(2));
    path.add_edge(EdgeIndex::new(2), NodeIndex::new(3));
    path.add_edge(EdgeIndex::new(3), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(2));
    path.add_edge(EdgeIndex::new(4), NodeIndex::new(4));

    let edge_weight = |edge: EdgeIndex<u32>| edge.index() as i32 + 1;
    let word = path.to_word(edge_weight);
    let translation = LTCTranslation::from_path(&path);

    let ltc = translation.to_ltc(5, edge_weight);
    assert_eq!(ltc.elements.len(), 3);

    let dfa = translation.to_dfa(5, edge_weight);
    assert!(!dfa.accepts(&word));
}

#[test]
fn to_ltc_2() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add_edge(EdgeIndex::new(0), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(1));

    let edge_weight = |edge: EdgeIndex<u32>| edge.index() as i32 + 1;
    let word = path.to_word(edge_weight);
    let translation = LTCTranslation::from_path(&path);

    let ltc = translation.to_ltc(2, edge_weight);
    assert_eq!(ltc.elements.len(), 2);

    let dfa = translation.to_dfa(2, edge_weight);
    assert!(!dfa.accepts(&word));
}

#[test]
fn to_ltc_3() {
    let mut path = Path::new(NodeIndex::new(0));
    path.add_edge(EdgeIndex::new(0), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(1));
    path.add_edge(EdgeIndex::new(1), NodeIndex::new(1));

    let edge_weight = |edge: EdgeIndex<u32>| edge.index() as i32 + 1;
    let word = path.to_word(edge_weight);
    let translation = LTCTranslation::from_path(&path);

    dbg!(&translation);

    let ltc = translation.to_ltc(2, edge_weight);
    assert_eq!(ltc.elements.len(), 2);

    let dfa = translation.to_dfa(2, edge_weight);
    assert!(!dfa.accepts(&word));
}
