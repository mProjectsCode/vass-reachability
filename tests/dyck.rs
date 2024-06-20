use vass_reachability::automaton::{dyck::DyckVASS, Automaton};

#[test]
fn test_dyck() {
    let dyck = DyckVASS::<2>::new();

    let input = vec![1, 2, -2, -1];
    assert!(dyck.accepts(&input));

    let input = vec![1, 2, -2, -1, 1, 2, -2, -1];
    assert!(dyck.accepts(&input));

    let input = vec![1, -1, 2, -1, -2];
    assert!(!dyck.accepts(&input));
}
