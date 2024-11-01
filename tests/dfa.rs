use itertools::Itertools;
use vass_reachability::{
    automaton::{
        dfa::{DfaNodeData, DFA},
        AutBuild, Automaton,
    },
    validation::same_language::same_language,
};

#[test]
fn test_dfa() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa.add_state(DfaNodeData::new(false, 1));
    let q2 = dfa.add_state(DfaNodeData::new(true, 2));
    dfa.set_start(q0);

    dfa.add_transition(q0, q1, 'a');
    dfa.add_transition(q1, q2, 'b');
    dfa.add_transition(q2, q1, 'a');

    dfa.add_failure_state(3);

    let input = "ababab";
    assert!(dfa.accepts(&input.chars().collect_vec()));

    let input = "ababa";
    assert!(!dfa.accepts(&input.chars().collect_vec()));
}

#[test]
fn test_dfa_inversion() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa.add_state(DfaNodeData::new(false, 1));
    let q2 = dfa.add_state(DfaNodeData::new(true, 2));
    dfa.set_start(q0);

    dfa.add_transition(q0, q1, 'a');
    dfa.add_transition(q1, q2, 'b');
    dfa.add_transition(q2, q1, 'a');

    dfa.add_failure_state(3);

    let input = "ababab";
    assert!(dfa.accepts(&input.chars().collect_vec()));

    let input = "ababa";
    assert!(!dfa.accepts(&input.chars().collect_vec()));

    let inverted = dfa.invert();

    let input = "ababab";
    assert!(!inverted.accepts(&input.chars().collect_vec()));

    let input = "ababa";
    assert!(inverted.accepts(&input.chars().collect_vec()));
}

#[test]
fn test_dfa_intersection() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa1.add_state(DfaNodeData::new(true, 1));
    dfa1.set_start(q0);

    // a* b b*
    dfa1.add_transition(q0, q0, 'a');
    dfa1.add_transition(q0, q1, 'b');
    dfa1.add_transition(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa2.add_state(DfaNodeData::new(true, 1));
    dfa2.set_start(q0);

    // a b*
    dfa2.add_transition(q0, q1, 'a');
    dfa2.add_transition(q1, q1, 'b');

    dfa1.add_failure_state(2);
    dfa2.add_failure_state(2);

    // we want to figure out if L2 is a subset of L1
    // so (a b*) is a subset of (a* b b*)
    // which is wrong, since "a" is not in (a* b b*)
    // the inclusion holds if there is no accepting run in the intersection of L2 and inv L1
    // A ⊆ B iff A ∩ inv(B) = ∅

    assert!(!dfa2.is_subset_of(&dfa1));
}

#[test]
fn test_dfa_intersection_2() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa1.add_state(DfaNodeData::new(true, 1));
    dfa1.set_start(q0);

    // a* b b*
    dfa1.add_transition(q0, q0, 'a');
    dfa1.add_transition(q0, q1, 'b');
    dfa1.add_transition(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa2.add_state(DfaNodeData::new(true, 1));
    dfa2.set_start(q0);

    // a b*
    dfa2.add_transition(q0, q1, 'a');
    dfa2.add_transition(q1, q1, 'b');

    dfa1.add_failure_state(2);
    dfa2.add_failure_state(2);

    // we want to figure out if L1 is a subset of L2
    // which is wrong, since "b b*" is not in (a b*)

    assert!(!dfa1.is_subset_of(&dfa2));
}

#[test]
fn test_dfa_intersection_3() {
    let mut dfa1 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa1.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa1.add_state(DfaNodeData::new(true, 1));
    dfa1.set_start(q0);

    // a* b b*
    dfa1.add_transition(q0, q0, 'a');
    dfa1.add_transition(q0, q1, 'b');
    dfa1.add_transition(q1, q1, 'b');

    let mut dfa2 = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa2.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa2.add_state(DfaNodeData::new(true, 1));
    dfa2.set_start(q0);

    // a* b
    dfa2.add_transition(q0, q0, 'a');
    dfa2.add_transition(q0, q1, 'b');

    dfa1.add_failure_state(2);
    dfa2.add_failure_state(2);

    // we want to figure out if L2 is a subset of L1
    // which should be true

    assert!(dfa2.is_subset_of(&dfa1));
}

#[test]
fn minimize_1() {
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa.add_state(DfaNodeData::new(false, 1));
    let q2 = dfa.add_state(DfaNodeData::new(false, 2));
    let q3 = dfa.add_state(DfaNodeData::new(true, 3));
    let q4 = dfa.add_state(DfaNodeData::new(false, 4));
    let q5 = dfa.add_state(DfaNodeData::new(true, 5));
    dfa.set_start(q0);

    dfa.add_transition(q0, q1, 'a');
    dfa.add_transition(q0, q3, 'b');
    dfa.add_transition(q1, q0, 'a');
    dfa.add_transition(q1, q3, 'b');
    dfa.add_transition(q2, q1, 'a');
    dfa.add_transition(q2, q4, 'b');
    dfa.add_transition(q3, q5, 'a');
    dfa.add_transition(q3, q5, 'b');
    dfa.add_transition(q4, q3, 'a');
    dfa.add_transition(q4, q3, 'b');
    dfa.add_transition(q5, q5, 'a');
    dfa.add_transition(q5, q5, 'b');

    dfa.override_complete();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 10));
    assert_eq!(minimized.state_count(), 3);
}

#[test]
fn minimize_2() {
    // example:  https://en.wikipedia.org/wiki/DFA_minimization
    let mut dfa = DFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = dfa.add_state(DfaNodeData::new(false, 0));
    let q1 = dfa.add_state(DfaNodeData::new(false, 1));
    let q2 = dfa.add_state(DfaNodeData::new(true, 2));
    let q3 = dfa.add_state(DfaNodeData::new(true, 3));
    let q4 = dfa.add_state(DfaNodeData::new(true, 4));
    let q5 = dfa.add_state(DfaNodeData::new(false, 5));
    dfa.set_start(q0);

    dfa.add_transition(q0, q1, 'a');
    dfa.add_transition(q0, q2, 'b');
    dfa.add_transition(q1, q0, 'a');
    dfa.add_transition(q1, q3, 'b');
    dfa.add_transition(q2, q4, 'a');
    dfa.add_transition(q2, q5, 'b');
    dfa.add_transition(q3, q4, 'a');
    dfa.add_transition(q3, q5, 'b');
    dfa.add_transition(q4, q4, 'a');
    dfa.add_transition(q4, q5, 'b');
    dfa.add_transition(q5, q5, 'a');
    dfa.add_transition(q5, q5, 'b');

    dfa.override_complete();

    let minimized = dfa.minimize();

    assert!(same_language(&dfa, &minimized, 10));
    // TODO: currently the minimized has 4 states, but it should have 3
    assert_eq!(minimized.state_count(), 3);
    dbg!(&minimized);
}
