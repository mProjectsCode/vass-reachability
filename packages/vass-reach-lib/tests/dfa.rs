use itertools::Itertools;
use vass_reach_lib::{
    automaton::{
        Automaton, InitializedAutomaton, Language,
        dfa::{DFA, minimization::Minimizable, node::DfaNode},
        path::{Path, path_like::IndexPath},
    },
    validation::same_language::{assert_inverse_language, assert_same_language, same_language},
};

#[test]
fn test_dfa() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));
    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q1, q2, 'b');
    dfa.add_edge(q2, q1, 'a');

    dfa.make_complete(3);

    let input = "ababab";
    let chars = input.chars().collect_vec();
    assert!(dfa.accepts(&chars));

    let input = "ababa";
    let chars = input.chars().collect_vec();
    assert!(!dfa.accepts(&chars));
}

#[test]
fn test_dfa_inversion() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));
    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q1, q2, 'b');
    dfa.add_edge(q2, q1, 'a');

    dfa.make_complete(3);

    let inverted = dfa.invert();

    assert_inverse_language(&dfa, &inverted, 6);

    let double_inverted = inverted.invert();

    assert_same_language(&dfa, &double_inverted, 6);

    // let input = "ababab";
    // assert!(dfa.accepts(&input.chars().collect_vec()));

    // let input = "ababa";
    // assert!(!dfa.accepts(&input.chars().collect_vec()));

    // let input = "ababab";
    // assert!(!inverted.accepts(&input.chars().collect_vec()));

    // let input = "ababa";
    // assert!(inverted.accepts(&input.chars().collect_vec()));
}

#[test]
fn test_dfa_intersection() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_node(DfaNode::non_accepting(0));
    let q1 = dfa1.add_node(DfaNode::accepting(1));
    dfa1.set_initial(q0);

    // a* b b*
    dfa1.add_edge(q0, q0, 'a');
    dfa1.add_edge(q0, q1, 'b');
    dfa1.add_edge(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_node(DfaNode::non_accepting(0));
    let q1 = dfa2.add_node(DfaNode::accepting(1));
    dfa2.set_initial(q0);

    // a b*
    dfa2.add_edge(q0, q1, 'a');
    dfa2.add_edge(q1, q1, 'b');

    dfa1.make_complete(2);
    dfa2.make_complete(2);

    // we want to figure out if L2 is a subset of L1
    // so (a b*) is a subset of (a* b b*)
    // which is wrong, since "a" is not in (a* b b*)
    // the inclusion holds if there is no accepting run in the intersection of L2
    // and inv L1 A ⊆ B iff A ∩ inv(B) = ∅

    assert!(!dfa2.is_subset_of(&dfa1));
}

#[test]
fn test_dfa_intersection_2() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_node(DfaNode::non_accepting(0));
    let q1 = dfa1.add_node(DfaNode::accepting(1));
    dfa1.set_initial(q0);

    // a* b b*
    dfa1.add_edge(q0, q0, 'a');
    dfa1.add_edge(q0, q1, 'b');
    dfa1.add_edge(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_node(DfaNode::non_accepting(0));
    let q1 = dfa2.add_node(DfaNode::accepting(1));
    dfa2.set_initial(q0);

    // a b*
    dfa2.add_edge(q0, q1, 'a');
    dfa2.add_edge(q1, q1, 'b');

    dfa1.make_complete(2);
    dfa2.make_complete(2);

    // we want to figure out if L1 is a subset of L2
    // which is wrong, since "b b*" is not in (a b*)

    assert!(!dfa1.is_subset_of(&dfa2));
}

#[test]
fn test_dfa_intersection_3() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_node(DfaNode::non_accepting(0));
    let q1 = dfa1.add_node(DfaNode::accepting(1));
    dfa1.set_initial(q0);

    // a* b b*
    dfa1.add_edge(q0, q0, 'a');
    dfa1.add_edge(q0, q1, 'b');
    dfa1.add_edge(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_node(DfaNode::non_accepting(0));
    let q1 = dfa2.add_node(DfaNode::accepting(1));
    dfa2.set_initial(q0);

    // a* b
    dfa2.add_edge(q0, q0, 'a');
    dfa2.add_edge(q0, q1, 'b');

    dfa1.make_complete(2);
    dfa2.make_complete(2);

    // we want to figure out if L2 is a subset of L1
    // which should be true

    assert!(dfa2.is_subset_of(&dfa1));
}

#[test]
fn minimize_1() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));
    let q3 = dfa.add_node(DfaNode::accepting(3));
    let q4 = dfa.add_node(DfaNode::non_accepting(4));
    let q5 = dfa.add_node(DfaNode::accepting(5));
    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q0, q3, 'b');
    dfa.add_edge(q1, q0, 'a');
    dfa.add_edge(q1, q3, 'b');
    dfa.add_edge(q2, q1, 'a');
    dfa.add_edge(q2, q4, 'b');
    dfa.add_edge(q3, q5, 'a');
    dfa.add_edge(q3, q5, 'b');
    dfa.add_edge(q4, q3, 'a');
    dfa.add_edge(q4, q3, 'b');
    dfa.add_edge(q5, q5, 'a');
    dfa.add_edge(q5, q5, 'b');

    dfa.set_complete_unchecked();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 10));
    assert_eq!(minimized.node_count(), 2);
}

#[test]
fn minimize_2() {
    // example:  https://en.wikipedia.org/wiki/DFA_minimization
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));
    let q3 = dfa.add_node(DfaNode::accepting(3));
    let q4 = dfa.add_node(DfaNode::accepting(4));
    let q5 = dfa.add_node(DfaNode::non_accepting(5));
    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q0, q2, 'b');
    dfa.add_edge(q1, q0, 'a');
    dfa.add_edge(q1, q3, 'b');
    dfa.add_edge(q2, q4, 'a');
    dfa.add_edge(q2, q5, 'b');
    dfa.add_edge(q3, q4, 'a');
    dfa.add_edge(q3, q5, 'b');
    dfa.add_edge(q4, q4, 'a');
    dfa.add_edge(q4, q5, 'b');
    dfa.add_edge(q5, q5, 'a');
    dfa.add_edge(q5, q5, 'b');

    dfa.set_complete_unchecked();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 10));
    assert_eq!(minimized.node_count(), 3);
}

#[test]
fn minimize_3() {
    let mut dfa = DFA::<u32, char>::new(vec!['a']);

    let q0 = dfa.add_node(DfaNode::accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));
    let q3 = dfa.add_node(DfaNode::non_accepting(3));

    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q1, q2, 'a');
    dfa.add_edge(q2, q3, 'a');
    dfa.add_edge(q3, q0, 'a');

    dfa.set_complete_unchecked();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 10));
    assert_eq!(minimized.node_count(), 2);
}

#[test]
fn minimize_4() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b', 'c', 'd']);

    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));

    dfa.set_initial(q0);

    dfa.add_edge(q0, q0, 'a');
    dfa.add_edge(q0, q1, 'b');
    dfa.add_edge(q0, q2, 'c');
    dfa.add_edge(q0, q2, 'd');
    dfa.add_edge(q1, q2, 'a');
    dfa.add_edge(q1, q2, 'b');
    dfa.add_edge(q1, q1, 'c');
    dfa.add_edge(q1, q2, 'd');
    dfa.add_edge(q2, q2, 'a');
    dfa.add_edge(q2, q2, 'b');
    dfa.add_edge(q2, q2, 'c');
    dfa.add_edge(q2, q2, 'd');

    dfa.set_complete_unchecked();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 8));
}

#[test]
fn minimize_5() {
    let mut dfa = DFA::<u32, i32>::new(vec![1, 2, -1, -2]);

    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));
    let q3 = dfa.add_node(DfaNode::non_accepting(3));
    let q4 = dfa.add_node(DfaNode::non_accepting(4));
    let q5 = dfa.add_node(DfaNode::accepting(5));
    let q6 = dfa.add_node(DfaNode::non_accepting(6));
    let q7 = dfa.add_node(DfaNode::non_accepting(7));
    let q8 = dfa.add_node(DfaNode::non_accepting(8));

    dfa.set_initial(q0);

    dfa.add_edge(q0, q1, -2);
    dfa.add_edge(q0, q2, -1);
    dfa.add_edge(q0, q3, 1);
    dfa.add_edge(q0, q1, 2);

    dfa.add_edge(q1, q1, -2);
    dfa.add_edge(q1, q1, -1);
    dfa.add_edge(q1, q3, 1);
    dfa.add_edge(q1, q1, 2);

    dfa.add_edge(q2, q2, -2);
    dfa.add_edge(q2, q2, -1);
    dfa.add_edge(q2, q6, 1);
    dfa.add_edge(q2, q2, 2);

    dfa.add_edge(q3, q1, -2);
    dfa.add_edge(q3, q4, -1);
    dfa.add_edge(q3, q3, 1);
    dfa.add_edge(q3, q1, 2);

    dfa.add_edge(q4, q1, -2);
    dfa.add_edge(q4, q5, -1);
    dfa.add_edge(q4, q1, 1);
    dfa.add_edge(q4, q1, 2);

    dfa.add_edge(q5, q1, -2);
    dfa.add_edge(q5, q5, -1);
    dfa.add_edge(q5, q1, 1);
    dfa.add_edge(q5, q1, 2);

    dfa.add_edge(q6, q2, -2);
    dfa.add_edge(q6, q7, -1);
    dfa.add_edge(q6, q6, 1);
    dfa.add_edge(q6, q2, 2);

    dfa.add_edge(q7, q2, -2);
    dfa.add_edge(q7, q8, -1);
    dfa.add_edge(q7, q2, 1);
    dfa.add_edge(q7, q2, 2);

    dfa.add_edge(q8, q2, -2);
    dfa.add_edge(q8, q8, -1);
    dfa.add_edge(q8, q2, 1);
    dfa.add_edge(q8, q2, 2);

    dfa.set_complete_unchecked();

    let minimized = dfa.minimize();

    assert_eq!(minimized.node_count(), 6);
    // dbg!(&minimized);

    assert_same_language(&dfa, &minimized, 8);
}

#[test]
fn find_loop_1() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);

    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));
    let q3 = dfa.add_node(DfaNode::accepting(3));

    dfa.set_initial(q0);

    let e1 = dfa.add_edge(q0, q1, 'a');
    let e2 = dfa.add_edge(q1, q2, 'b');
    let e3 = dfa.add_edge(q2, q3, 'a');
    let e4 = dfa.add_edge(q3, q0, 'b');

    dfa.make_complete(4);

    // loop in q0
    let loop_ = dfa.find_loop_rooted_in_node(q0);
    assert!(loop_.is_some());

    let mut path = Path::new(q0);
    path.add(e1, q1);
    path.add(e2, q2);
    path.add(e3, q3);
    path.add(e4, q0);

    assert_eq!(loop_.unwrap(), path);

    // loop in q1
    let loop_ = dfa.find_loop_rooted_in_node(q1);
    assert!(loop_.is_some());

    let mut path = Path::new(q1);
    path.add(e2, q2);
    path.add(e3, q3);
    path.add(e4, q0);
    path.add(e1, q1);

    assert_eq!(loop_.unwrap(), path);
}

#[test]
fn find_loop_2() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b', 'c']);

    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));
    let q3 = dfa.add_node(DfaNode::non_accepting(3));
    let q4 = dfa.add_node(DfaNode::non_accepting(4));
    let q5 = dfa.add_node(DfaNode::accepting(5));

    dfa.set_initial(q0);

    let e1 = dfa.add_edge(q0, q1, 'a');
    let e2 = dfa.add_edge(q1, q0, 'b');
    let _e3 = dfa.add_edge(q0, q2, 'b');
    let _e4 = dfa.add_edge(q2, q3, 'a');
    let _e5 = dfa.add_edge(q3, q0, 'a');
    let _e6 = dfa.add_edge(q0, q4, 'c');
    let e6 = dfa.add_edge(q4, q5, 'b');
    let e7 = dfa.add_edge(q5, q5, 'a');
    let e8 = dfa.add_edge(q5, q4, 'b');

    dfa.make_complete(6);

    // loop in q0
    let loop_ = dfa.find_loop_rooted_in_node(q0);
    assert!(loop_.is_some());

    let mut path = Path::new(q0);
    path.add(e1, q1);
    path.add(e2, q0);

    assert_eq!(loop_.unwrap(), path);

    // loop in q4
    let loop_ = dfa.find_loop_rooted_in_node(q4);
    assert!(loop_.is_some());

    let mut path = Path::new(q4);
    path.add(e6, q5);
    path.add(e8, q4);

    assert_eq!(loop_.unwrap(), path);

    // loop in q5
    let loop_ = dfa.find_loop_rooted_in_node(q5);
    assert!(loop_.is_some());

    let mut path = Path::new(q5);
    path.add(e7, q5);

    assert_eq!(loop_.unwrap(), path);
}

#[test]
fn find_loops_1() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b', 'c']);

    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::non_accepting(2));
    let q3 = dfa.add_node(DfaNode::non_accepting(3));
    let q4 = dfa.add_node(DfaNode::non_accepting(4));
    let q5 = dfa.add_node(DfaNode::accepting(5));

    dfa.set_initial(q0);

    let _e1 = dfa.add_edge(q0, q1, 'a');
    let _e2 = dfa.add_edge(q1, q0, 'b');
    let _e3 = dfa.add_edge(q0, q2, 'b');
    let _e4 = dfa.add_edge(q2, q3, 'a');
    let _e5 = dfa.add_edge(q3, q0, 'a');
    let _e6 = dfa.add_edge(q0, q4, 'c');
    let _e6 = dfa.add_edge(q4, q5, 'b');
    let _e7 = dfa.add_edge(q5, q5, 'a');
    let _e8 = dfa.add_edge(q5, q4, 'b');

    dfa.make_complete(6);

    // loop in q0
    let loops = dfa.find_loops_rooted_in_node(q0, None);
    assert_eq!(loops.len(), 2);
    for loop_ in loops {
        assert_eq!(loop_.start(), q0);
        assert_eq!(loop_.end(), q0);
    }

    // loop in q4
    let loops = dfa.find_loops_rooted_in_node(q4, None);
    assert_eq!(loops.len(), 1);
    for loop_ in loops {
        assert_eq!(loop_.start(), q4);
        assert_eq!(loop_.end(), q4);
    }

    // loop in q5
    let loops = dfa.find_loops_rooted_in_node(q5, None);
    assert_eq!(loops.len(), 2);
    for loop_ in loops {
        assert_eq!(loop_.start(), q5);
        assert_eq!(loop_.end(), q5);
    }
}

#[test]
fn reverse_1() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));

    dfa.set_initial(q0);
    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q1, q2, 'b');

    dfa.make_complete(3);

    let reversed = dfa.reverse();

    assert!(dfa.accepts(['a', 'b'].iter()));
    assert!(!dfa.accepts(['b', 'a'].iter()));

    assert!(reversed.accepts(['b', 'a'].iter()));
    assert!(!reversed.accepts(['a', 'b'].iter()));
}

#[test]
fn reverse_2() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b', 'c']);
    let q0 = dfa.add_node(DfaNode::non_accepting(0));
    let q1 = dfa.add_node(DfaNode::non_accepting(1));
    let q2 = dfa.add_node(DfaNode::accepting(2));

    dfa.set_initial(q0);
    dfa.add_edge(q0, q1, 'a');
    dfa.add_edge(q1, q2, 'b');
    dfa.add_edge(q1, q2, 'c');

    dfa.make_complete(3);

    let reversed = dfa.reverse();

    assert!(dfa.accepts(['a', 'b'].iter()));
    assert!(dfa.accepts(['a', 'c'].iter()));
    assert!(!dfa.accepts(['b', 'a'].iter()));

    assert!(reversed.accepts(['b', 'a'].iter()));
    assert!(reversed.accepts(['c', 'a'].iter()));
    assert!(!reversed.accepts(['a', 'b'].iter()));
}
