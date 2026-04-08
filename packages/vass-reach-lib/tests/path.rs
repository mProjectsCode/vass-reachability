use petgraph::graph::{EdgeIndex, NodeIndex};
use vass_reach_lib::automaton::{
    ModifiableAutomaton,
    cfg::update::CFGCounterUpdate,
    dfa::{DFA, node::DfaNode},
    path::{Path, parikh_image::ParikhImage},
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

#[test]
fn test_path_basic_manipulation() {
    let mut path = Path::new(NodeIndex::from(0u32));
    assert_eq!(path.len(), 0);
    assert!(path.is_empty());
    assert_eq!(path.start(), &NodeIndex::from(0u32));
    assert_eq!(path.end(), &NodeIndex::from(0u32));

    path.add('a', NodeIndex::from(1u32));
    assert_eq!(path.len(), 1);
    assert!(!path.is_empty());
    assert_eq!(path.end(), &NodeIndex::from(1u32));
    assert!(path.contains_node(&NodeIndex::from(0u32)));
    assert!(path.contains_node(&NodeIndex::from(1u32)));

    path.add('b', NodeIndex::from(2u32));
    assert_eq!(path.len(), 2);
    assert_eq!(path.get_letter(0), &'a');
    assert_eq!(path.get_node(0), &NodeIndex::from(1u32));
    assert_eq!(path.get_letter(1), &'b');
    assert_eq!(path.get_node(1), &NodeIndex::from(2u32));
}

#[test]
fn test_path_from_word_and_take_edge() {
    let mut dfa = DFA::new(vec!['a', 'b']);
    let n0 = dfa.add_node(DfaNode::new(false, true, ()));
    let n1 = dfa.add_node(DfaNode::new(false, false, ()));
    dfa.add_edge(&n0, &n1, 'a');
    dfa.set_initial(n0);

    let path = Path::from_word(n0, &vec!['a'], &dfa).unwrap();
    assert_eq!(path.len(), 1);
    assert_eq!(path.end(), &n1);

    let mut path2 = Path::new(n0);
    path2.take_edge('a', &dfa).unwrap();
    assert_eq!(path2.end(), &n1);

    assert!(path2.take_edge('b', &dfa).is_err());
}

#[test]
fn test_path_loop_detection_and_formatting() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));

    assert!(!path.has_loop());
    path.add('c', NodeIndex::from(1u32));
    assert!(path.has_loop());

    assert_eq!(
        path.to_fancy_string(),
        "NodeIndex(0) --('a')-> NodeIndex(1) --('b')-> NodeIndex(2) --('c')-> NodeIndex(1)"
    );
}

#[test]
fn test_path_iterators() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));

    let nodes: Vec<_> = path.iter_nodes().collect();
    assert_eq!(
        nodes,
        vec![
            &NodeIndex::from(0u32),
            &NodeIndex::from(1u32),
            &NodeIndex::from(2u32)
        ]
    );

    let letters: Vec<_> = path.iter_letters().collect();
    assert_eq!(letters, vec![&'a', &'b']);

    let combined: Vec<_> = path.iter().collect();
    assert_eq!(
        combined,
        vec![
            (&'a', &NodeIndex::from(1u32)),
            (&'b', &NodeIndex::from(2u32))
        ]
    );

    let rev: Vec<_> = path.iter().rev().collect();
    assert_eq!(
        rev,
        vec![
            (&'b', &NodeIndex::from(2u32)),
            (&'a', &NodeIndex::from(1u32))
        ]
    );
}

#[test]
fn test_path_concat_and_slicing() {
    let mut p1 = Path::new(NodeIndex::from(0u32));
    p1.add('a', NodeIndex::from(1u32));

    let mut p2 = Path::new(NodeIndex::from(1u32));
    p2.add('b', NodeIndex::from(2u32));

    p1.concat(p2);
    assert_eq!(p1.len(), 2);
    assert_eq!(p1.start(), &NodeIndex::from(0u32));
    assert_eq!(p1.end(), &NodeIndex::from(2u32));

    let s1 = p1.slice(0..1);
    assert_eq!(s1.len(), 1);
    assert_eq!(s1.get_letter(0), &'a');

    let mut p_split = p1.clone();
    let p_off = p_split.split_off(1);
    assert_eq!(p_split.len(), 1);
    assert_eq!(p_off.len(), 1);
    assert_eq!(p_split.start(), &NodeIndex::from(0u32));
    assert_eq!(p_split.end(), &NodeIndex::from(1u32));
    assert_eq!(p_off.start(), &NodeIndex::from(1u32));
    assert_eq!(p_off.end(), &NodeIndex::from(2u32));
}

#[test]
fn test_path_splits() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));
    path.add('c', NodeIndex::from(1u32));
    path.add('d', NodeIndex::from(3u32));

    // split_at_node
    let parts = path.clone().split_at_node(&NodeIndex::from(1u32));
    assert_eq!(parts.len(), 3);
    assert_eq!(parts[0].end(), &NodeIndex::from(1u32));
    assert_eq!(parts[1].end(), &NodeIndex::from(1u32));

    // split_at_nodes
    let parts_multi = path
        .clone()
        .split_at_nodes(&[NodeIndex::from(1u32), NodeIndex::from(2u32)]);
    assert_eq!(parts_multi.len(), 4);

    // split_at predicate
    let parts_fn = path.clone().split_at(|n, _| n.index() == 2);
    assert_eq!(parts_fn.len(), 2);
    assert_eq!(parts_fn[0].end(), &NodeIndex::from(2u32));
}

#[test]
fn test_vass_path_reaching_and_valuations() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add(CFGCounterUpdate::new(0, true), NodeIndex::from(1u32));
    path.add(CFGCounterUpdate::new(1, false), NodeIndex::from(2u32));

    let initial = VASSCounterValuation::from(vec![0, 1]);
    let final_target = VASSCounterValuation::from(vec![1, 0]);
    assert!(path.is_n_reaching(&initial, &final_target));

    assert_eq!(
        path.get_path_final_valuation(&initial),
        VASSCounterValuation::from(vec![1, 0])
    );

    // Test negative counter detection
    let initial_zero = VASSCounterValuation::from(vec![0, 0]);
    assert!(!path.is_n_reaching(&initial_zero, &VASSCounterValuation::from(vec![1, -1])));
    assert_eq!(
        path.find_negative_counter_forward(&initial_zero),
        Some((VASSCounterIndex::from(1), 1))
    );

    // Test backward negative detection
    let mut path_back = Path::new(NodeIndex::from(0u32));
    path_back.add(CFGCounterUpdate::new(0, true), NodeIndex::from(1u32));
    // Final [0]. Backward: rev(+c0) = -c0. [0] -> [-1].
    assert_eq!(
        path_back.find_negative_counter_backward(&VASSCounterValuation::from(vec![0])),
        Some((VASSCounterIndex::from(0), 0))
    );
}

#[test]
fn test_vass_path_max_values() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add(CFGCounterUpdate::new(0, true), NodeIndex::from(1u32));
    path.add(CFGCounterUpdate::new(0, true), NodeIndex::from(2u32));
    path.add(CFGCounterUpdate::new(0, false), NodeIndex::from(3u32));

    let initial = VASSCounterValuation::from(vec![5]);
    // 5 -> 6 -> 7 -> 6. Max is 7.
    assert_eq!(
        path.max_counter_value(&initial, VASSCounterIndex::from(0)),
        7
    );

    let final_val = VASSCounterValuation::from(vec![5]);
    // Path: +c0, -c0. Final 5.
    // Backward: rev(-c0) = +c0 (5->6), rev(+c0) = -c0 (6->5). Max 6.
    let mut path2 = Path::new(NodeIndex::from(0u32));
    path2.add(CFGCounterUpdate::new(0, true), NodeIndex::from(1u32));
    path2.add(CFGCounterUpdate::new(0, false), NodeIndex::from(2u32));
    assert_eq!(
        path2.max_counter_value_from_back(&final_val, VASSCounterIndex::from(0)),
        6
    );
}

#[test]
fn test_parikh_image_basics() {
    let mut pi = ParikhImage::<usize>::empty(5);
    assert!(pi.is_empty());

    pi.set(1, 10);
    assert_eq!(pi.get(1), 10);
    assert!(!pi.is_empty());

    pi.add_to(1, 5);
    assert_eq!(pi.get(1), 15);

    pi.sub_from(1, 10);
    assert_eq!(pi.get(1), 5);

    pi.sub_from(1, 10); // saturates at 0
    assert_eq!(pi.get(1), 0);
    assert!(pi.is_empty());

    pi.set_max(2, 5);
    assert_eq!(pi.get(2), 5);
    pi.set_max(2, 3);
    assert_eq!(pi.get(2), 5);
}

#[test]
fn test_parikh_image_iterators() {
    let mut pi = ParikhImage::<usize>::empty(3);
    pi.set(0, 5);
    pi.set(2, 10);

    let items: Vec<_> = pi.iter().collect();
    assert_eq!(items.len(), 2);
    assert!(items.contains(&(0, 5)));
    assert!(items.contains(&(2, 10)));

    let edges: Vec<_> = pi.iter_edges().collect();
    assert_eq!(edges.len(), 2);
    assert!(edges.contains(&0));
    assert!(edges.contains(&2));
}

#[test]
fn test_parikh_image_from_path() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add(EdgeIndex::from(0), NodeIndex::from(1u32));
    path.add(EdgeIndex::from(1), NodeIndex::from(2u32));
    path.add(EdgeIndex::from(0), NodeIndex::from(3u32));

    let pi = ParikhImage::from_path(&path, 5);
    assert_eq!(pi.get(EdgeIndex::from(0)), 2);
    assert_eq!(pi.get(EdgeIndex::from(1)), 1);
    assert_eq!(pi.get(EdgeIndex::from(2)), 0);
}

#[test]
fn test_path_into_iterator() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));

    let collected: Vec<_> = path.into_iter().collect();
    assert_eq!(collected.len(), 2);
    assert_eq!(collected[0], ('a', NodeIndex::from(1u32)));
    assert_eq!(collected[1], ('b', NodeIndex::from(2u32)));
}

#[test]
fn test_path_first_last() {
    let mut path = Path::new(NodeIndex::from(0u32));
    assert!(path.first().is_none());
    assert!(path.last().is_none());

    path.add('a', NodeIndex::from(1u32));
    assert_eq!(path.first(), Some((&'a', &NodeIndex::from(1u32))));
    assert_eq!(path.last(), Some((&'a', &NodeIndex::from(1u32))));

    path.add('b', NodeIndex::from(2u32));
    assert_eq!(path.first(), Some((&'a', &NodeIndex::from(1u32))));
    assert_eq!(path.last(), Some((&'b', &NodeIndex::from(2u32))));
}
