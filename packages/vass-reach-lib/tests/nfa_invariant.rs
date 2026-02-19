use vass_reach_lib::{
    automaton::{
        ModifiableAutomaton,
        dfa::node::DfaNode,
        nfa::{NFA, NFAEdge},
    },
    validation::same_language::assert_same_language,
};

#[test]
fn test_nfa_determinize_invariant_to_node_insertion_order() {
    // build the same NFA but insert nodes in a different order
    // original ordering
    let mut nfa_a = NFA::<u32, char>::new(vec!['a', 'b']);
    let a_q0 = nfa_a.add_node(DfaNode::non_accepting(0));
    let a_q1 = nfa_a.add_node(DfaNode::non_accepting(1));
    let a_q2 = nfa_a.add_node(DfaNode::non_accepting(2));
    let a_q3 = nfa_a.add_node(DfaNode::non_accepting(3));
    let a_q4 = nfa_a.add_node(DfaNode::accepting(4));
    nfa_a.set_initial(a_q0);

    nfa_a.add_edge(&a_q0, &a_q1, NFAEdge::Symbol('a'));
    nfa_a.add_edge(&a_q0, &a_q2, NFAEdge::Epsilon);
    nfa_a.add_edge(&a_q1, &a_q2, NFAEdge::Symbol('b'));
    nfa_a.add_edge(&a_q2, &a_q3, NFAEdge::Symbol('a'));
    nfa_a.add_edge(&a_q2, &a_q4, NFAEdge::Epsilon);
    nfa_a.add_edge(&a_q3, &a_q2, NFAEdge::Symbol('b'));

    // different insertion order
    let mut nfa_b = NFA::<u32, char>::new(vec!['a', 'b']);
    let b_q0 = nfa_b.add_node(DfaNode::non_accepting(0));
    let b_q2 = nfa_b.add_node(DfaNode::non_accepting(2));
    let b_q1 = nfa_b.add_node(DfaNode::non_accepting(1));
    let b_q3 = nfa_b.add_node(DfaNode::non_accepting(3));
    let b_q4 = nfa_b.add_node(DfaNode::accepting(4));
    nfa_b.set_initial(b_q0);

    nfa_b.add_edge(&b_q0, &b_q1, NFAEdge::Symbol('a'));
    nfa_b.add_edge(&b_q0, &b_q2, NFAEdge::Epsilon);
    nfa_b.add_edge(&b_q1, &b_q2, NFAEdge::Symbol('b'));
    nfa_b.add_edge(&b_q2, &b_q3, NFAEdge::Symbol('a'));
    nfa_b.add_edge(&b_q2, &b_q4, NFAEdge::Epsilon);
    nfa_b.add_edge(&b_q3, &b_q2, NFAEdge::Symbol('b'));

    let dfa_a = nfa_a.determinize();
    let dfa_b = nfa_b.determinize();

    // sizes and languages should match
    assert_eq!(dfa_a.graph.node_count(), dfa_b.graph.node_count());
    assert_eq!(dfa_a.graph.edge_count(), dfa_b.graph.edge_count());
    assert_same_language(&nfa_a, &dfa_a, 6);
    assert_same_language(&nfa_b, &dfa_b, 6);
}
