use serde::de;
use vass_reach_lib::{
    automaton::{
        Automaton, ModifiableAutomaton,
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

#[test]
fn test_nfa_to_dfa_4() {
    // We want to mirror the situation where we have the same SCC twice, directly
    // consecutive, in the MGTS.

    let mut nfa = NFA::<u32, char>::new(vec!['a', 'b', 'c']);
    // path before
    let q0 = nfa.add_node(DfaNode::non_accepting(0));
    let q1 = nfa.add_node(DfaNode::non_accepting(1));

    // scc 1
    let q2 = nfa.add_node(DfaNode::non_accepting(2));
    let q3 = nfa.add_node(DfaNode::non_accepting(3));

    // connecting state
    let q4 = nfa.add_node(DfaNode::non_accepting(4));

    // scc 2
    let q5 = nfa.add_node(DfaNode::non_accepting(5));
    let q6 = nfa.add_node(DfaNode::non_accepting(6));

    // path after
    let q7 = nfa.add_node(DfaNode::non_accepting(7));
    let q8 = nfa.add_node(DfaNode::accepting(8));

    nfa.set_initial(q0);

    nfa.add_edge(&q0, &q1, NFAEdge::Symbol('c'));
    // to the first scc
    nfa.add_edge(&q1, &q2, NFAEdge::Epsilon);

    // first scc
    nfa.add_edge(&q2, &q3, NFAEdge::Symbol('a'));
    nfa.add_edge(&q3, &q2, NFAEdge::Symbol('b'));

    // first scc down to connecting state
    nfa.add_edge(&q2, &q4, NFAEdge::Epsilon);

    // to the second scc
    nfa.add_edge(&q4, &q5, NFAEdge::Epsilon);

    // second scc
    nfa.add_edge(&q5, &q6, NFAEdge::Symbol('a'));
    nfa.add_edge(&q6, &q5, NFAEdge::Symbol('b'));

    // from the second scc to the path after
    nfa.add_edge(&q5, &q7, NFAEdge::Epsilon);
    nfa.add_edge(&q7, &q8, NFAEdge::Symbol('c'));

    let dfa = nfa.determinize();

    assert!(
        dfa.node_count() <= nfa.node_count(),
        "The DFA should have less states, since the determinization should be able to merge the two SCCs together. DFA node count: {}, NFA node count: {}",
        dfa.node_count(),
        nfa.node_count()
    );

    assert_same_language(&nfa, &dfa, 8);
}
