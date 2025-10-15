use vass_reach_lib::{
    automaton::{AutBuild, dfa::node::DfaNode, nfa::NFA},
    validation::same_language::assert_same_language,
};

#[test]
fn test_nfa_to_dfa() {
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_state(DfaNode::non_accepting(0));
    let q1 = nfa.add_state(DfaNode::non_accepting(1));
    let q2 = nfa.add_state(DfaNode::accepting(2));

    nfa.set_start(q0);

    nfa.add_transition(q0, q0, Some('a'));
    nfa.add_transition(q0, q1, Some('b'));

    nfa.add_transition(q1, q2, Some('a'));
    nfa.add_transition(q2, q1, Some('b'));

    nfa.add_transition(q1, q1, Some('a'));
    nfa.add_transition(q1, q1, Some('b'));

    nfa.add_transition(q2, q2, Some('a'));
    nfa.add_transition(q2, q2, Some('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}

#[test]
fn test_nfa_to_dfa_2() {
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_state(DfaNode::non_accepting(0));
    let q1 = nfa.add_state(DfaNode::non_accepting(1));
    let q2 = nfa.add_state(DfaNode::accepting(2));

    nfa.set_start(q0);

    nfa.add_transition(q0, q0, Some('a'));
    nfa.add_transition(q0, q0, Some('b'));

    nfa.add_transition(q0, q1, Some('a'));
    nfa.add_transition(q1, q2, Some('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}

#[test]
fn test_nfa_to_dfa_3() {
    // An NFA that has empty transitions
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_state(DfaNode::non_accepting(0));
    let q1 = nfa.add_state(DfaNode::non_accepting(1));
    let q2 = nfa.add_state(DfaNode::non_accepting(2));
    let q3 = nfa.add_state(DfaNode::non_accepting(3));
    let q4 = nfa.add_state(DfaNode::accepting(4));

    nfa.set_start(q0);

    nfa.add_transition(q0, q1, Some('a'));
    nfa.add_transition(q0, q2, None);

    nfa.add_transition(q1, q2, Some('b'));

    nfa.add_transition(q2, q3, Some('a'));
    nfa.add_transition(q2, q4, None);

    nfa.add_transition(q3, q2, Some('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}
