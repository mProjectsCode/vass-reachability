use petgraph::graph::{EdgeIndex, NodeIndex};
use vass_reachability::automaton::{path::Path, Automaton};

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

    let word = path.to_word(|edge| edge.index() as i32 + 1);

    let (ltc, dfa) = path.to_ltc(5, |edge| edge.index() as i32 + 1);

    // dbg!(&ltc);

    assert_eq!(ltc.elements.len(), 3);
    assert!(!dfa.accepts(&word));
}
