use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        dfa::node::DfaNode,
        nfa::{NFA, NFAEdge},
    },
    validation::same_language::assert_same_language,
};

#[test]
fn test_nfa_to_dfa() {
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_node(DfaNode::non_accepting(0));
    let q1 = nfa.add_node(DfaNode::non_accepting(1));
    let q2 = nfa.add_node(DfaNode::accepting(2));

    nfa.set_initial(q0);

    nfa.add_edge(&q0, &q0, NFAEdge::Symbol('a'));
    nfa.add_edge(&q0, &q1, NFAEdge::Symbol('b'));

    nfa.add_edge(&q1, &q2, NFAEdge::Symbol('a'));
    nfa.add_edge(&q2, &q1, NFAEdge::Symbol('b'));

    nfa.add_edge(&q1, &q1, NFAEdge::Symbol('a'));
    nfa.add_edge(&q1, &q1, NFAEdge::Symbol('b'));

    nfa.add_edge(&q2, &q2, NFAEdge::Symbol('a'));
    nfa.add_edge(&q2, &q2, NFAEdge::Symbol('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}

#[test]
fn test_nfa_to_dfa_2() {
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_node(DfaNode::non_accepting(0));
    let q1 = nfa.add_node(DfaNode::non_accepting(1));
    let q2 = nfa.add_node(DfaNode::accepting(2));

    nfa.set_initial(q0);

    nfa.add_edge(&q0, &q0, NFAEdge::Symbol('a'));
    nfa.add_edge(&q0, &q0, NFAEdge::Symbol('b'));

    nfa.add_edge(&q0, &q1, NFAEdge::Symbol('a'));
    nfa.add_edge(&q1, &q2, NFAEdge::Symbol('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}

#[test]
fn test_nfa_to_dfa_3() {
    // An NFA that has empty transitions
    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b']);
    let q0 = nfa.add_node(DfaNode::non_accepting(0));
    let q1 = nfa.add_node(DfaNode::non_accepting(1));
    let q2 = nfa.add_node(DfaNode::non_accepting(2));
    let q3 = nfa.add_node(DfaNode::non_accepting(3));
    let q4 = nfa.add_node(DfaNode::accepting(4));

    nfa.set_initial(q0);

    nfa.add_edge(&q0, &q1, NFAEdge::Symbol('a'));
    nfa.add_edge(&q0, &q2, NFAEdge::Epsilon);

    nfa.add_edge(&q1, &q2, NFAEdge::Symbol('b'));

    nfa.add_edge(&q2, &q3, NFAEdge::Symbol('a'));
    nfa.add_edge(&q2, &q4, NFAEdge::Epsilon);

    nfa.add_edge(&q3, &q2, NFAEdge::Symbol('b'));

    let dfa = nfa.determinize();

    assert_same_language(&nfa, &dfa, 6);

    // dbg!(&dfa);
}
