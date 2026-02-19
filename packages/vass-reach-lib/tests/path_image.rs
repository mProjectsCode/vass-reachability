use petgraph::graph::NodeIndex;
use vass_reach_lib::automaton::{
    cfg::update::CFGCounterUpdate,
    implicit_cfg_product::{path::MultiGraphPath, state::MultiGraphState},
    path::{Path, parikh_image::ParikhImage},
};

#[test]
fn test_path_basics() {
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
    assert_eq!(path.end(), &NodeIndex::from(2u32));
    assert_eq!(path.get_letter(0), &'a');
    assert_eq!(path.get_node(0), &NodeIndex::from(1u32));
    assert_eq!(path.get_letter(1), &'b');
    assert_eq!(path.get_node(1), &NodeIndex::from(2u32));
}

#[test]
fn test_path_more() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));
    path.add('c', NodeIndex::from(1u32));

    assert!(path.has_loop());
    assert!(path.has_node(&NodeIndex::from(0u32)));
    assert!(path.has_node(&NodeIndex::from(1u32)));
    assert!(path.has_node(&NodeIndex::from(2u32)));
    assert!(!path.has_node(&NodeIndex::from(3u32)));

    assert_eq!(
        path.to_fancy_string(),
        "NodeIndex(0) --('a')-> NodeIndex(1) --('b')-> NodeIndex(2) --('c')-> NodeIndex(1)"
    );

    let nodes: Vec<_> = path.iter_nodes().collect();
    assert_eq!(
        nodes,
        vec![
            &NodeIndex::from(0u32),
            &NodeIndex::from(1u32),
            &NodeIndex::from(2u32),
            &NodeIndex::from(1u32)
        ]
    );

    let letters: Vec<_> = path.iter_letters().collect();
    assert_eq!(letters, vec![&'a', &'b', &'c']);

    // split_at_nodes
    let split_nodes = vec![NodeIndex::from(1u32)];
    let parts = path.clone().split_at_nodes(&split_nodes);
    assert_eq!(parts.len(), 3);
}

#[test]
fn test_path_slice() {
    let mut path = Path::new(NodeIndex::from(0u32));
    path.add('a', NodeIndex::from(1u32));
    path.add('b', NodeIndex::from(2u32));
    path.add('c', NodeIndex::from(3u32));

    let s1 = path.slice(1); // indices 0..=1 -> (a,1), (b,2)
    assert_eq!(s1.len(), 2);
    assert_eq!(s1.get_letter(0), &'a');
    assert_eq!(s1.get_letter(1), &'b');

    let s2 = path.slice_end(1); // indices 1.. -> (b,2), (c,3)
    assert_eq!(s2.len(), 2);
    assert_eq!(s2.get_letter(0), &'b');
    assert_eq!(s2.get_letter(1), &'c');
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

    pi.sub_from(1, 10); // should saturate at 0
    assert_eq!(pi.get(1), 0);

    pi.set_max(2, 5);
    assert_eq!(pi.get(2), 5);
    pi.set_max(2, 3);
    assert_eq!(pi.get(2), 5);
}

#[test]
fn test_parikh_image_more() {
    let mut pi1 = ParikhImage::<usize>::empty(3);
    pi1.set(0, 5);
    pi1.set(1, 10);

    let mut pi2 = ParikhImage::<usize>::empty(3);
    pi2.set(1, 5);
    pi2.set(2, 20);

    // iter
    let items: Vec<_> = pi1.iter().collect();
    assert_eq!(items.len(), 2);
    assert!(items.contains(&(0, 5)));
    assert!(items.contains(&(1, 10)));

    let edges: Vec<_> = pi1.iter_edges().collect();
    assert_eq!(edges.len(), 2);
    assert!(edges.contains(&0));
    assert!(edges.contains(&1));
}

#[test]
fn test_multi_graph_path_basics() {
    let s0 = MultiGraphState::from(vec![NodeIndex::from(0u32), NodeIndex::from(0u32)]);
    let s1 = MultiGraphState::from(vec![NodeIndex::from(1u32), NodeIndex::from(0u32)]);
    let s2 = MultiGraphState::from(vec![NodeIndex::from(1u32), NodeIndex::from(1u32)]);

    let mut mgp = MultiGraphPath::new(s0.clone());
    assert_eq!(mgp.len(), 0);
    assert_eq!(mgp.start(), &s0);
    assert_eq!(mgp.end(), &s0);

    let u1 = CFGCounterUpdate::new(0, true);
    let u2 = CFGCounterUpdate::new(1, true);

    mgp.add(u1, s1.clone());
    assert_eq!(mgp.len(), 1);
    assert_eq!(mgp.end(), &s1);

    mgp.add(u2, s2.clone());
    assert_eq!(mgp.len(), 2);
    assert_eq!(mgp.end(), &s2);

    assert_eq!(mgp.state_len(), 3);
}

#[test]
fn test_multi_graph_path_more() {
    let s0 = MultiGraphState::from(vec![NodeIndex::from(0u32)]);
    let s1 = MultiGraphState::from(vec![NodeIndex::from(1u32)]);
    let s2 = MultiGraphState::from(vec![NodeIndex::from(2u32)]);

    let mut mgp = MultiGraphPath::new(s0.clone());
    mgp.add(CFGCounterUpdate::new(0, true), s1.clone());
    mgp.add(CFGCounterUpdate::new(1, true), s2.clone());

    assert_eq!(mgp.to_fancy_string(), "+c0 +c1");
    assert!(mgp.contains_state(&s1));
    assert!(!mgp.contains_state(&MultiGraphState::from(vec![NodeIndex::from(3u32)])));

    let updates: Vec<_> = mgp.iter().collect();
    assert_eq!(updates.len(), 2);

    let states: Vec<_> = mgp.iter_states().collect();
    assert_eq!(states.len(), 3);

    // concat
    let mut mgp2 = MultiGraphPath::new(s2.clone());
    mgp2.add(CFGCounterUpdate::new(0, false), s0.clone());
    mgp.concat(mgp2);
    assert_eq!(mgp.len(), 3);
    assert_eq!(mgp.end(), &s0);

    // slice
    let sub = mgp.slice(1..3);
    assert_eq!(sub.len(), 2);
    assert_eq!(sub.start(), &s1);
    assert_eq!(sub.end(), &s0);
}

#[test]
fn test_multi_graph_path_pumping() {
    let s0 = MultiGraphState::from(vec![
        NodeIndex::from(0u32),
        NodeIndex::from(0u32),
        NodeIndex::from(0u32),
        NodeIndex::from(0u32),
    ]);
    let s1 = MultiGraphState::from(vec![
        NodeIndex::from(1u32),
        NodeIndex::from(0u32),
        NodeIndex::from(0u32),
        NodeIndex::from(0u32),
    ]);

    let mut mgp = MultiGraphPath::new(s0.clone());
    mgp.add(CFGCounterUpdate::new(0, true), s1.clone());
    mgp.add(CFGCounterUpdate::new(0, true), s0.clone());
    mgp.add(CFGCounterUpdate::new(0, true), s1.clone());
    mgp.add(CFGCounterUpdate::new(0, true), s0.clone());

    assert!(mgp.is_counter_forwards_pumped(1, 0.into(), 1));
}
