use hashbrown::HashMap;
use itertools::Itertools;
use petgraph::{
    Direction,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use super::{AutBuild, Automaton, AutomatonEdge, AutomatonNode, dfa::DFA};
use crate::automaton::{dfa::node::DfaNode, path::path_like::EdgeListLike};

#[derive(Debug, Clone)]
pub struct NFA<N: AutomatonNode, E: AutomatonEdge> {
    start: Option<NodeIndex<u32>>,
    pub graph: DiGraph<DfaNode<N>, Option<E>>,
    alphabet: Vec<E>,
}

impl<N: AutomatonNode, E: AutomatonEdge> NFA<N, E> {
    pub fn new(alphabet: Vec<E>) -> Self {
        let graph = DiGraph::new();

        NFA {
            alphabet,
            start: None,
            graph,
        }
    }

    pub fn set_start(&mut self, start: NodeIndex<u32>) {
        self.start = Some(start);
    }

    pub fn set_accepting(&mut self, state: NodeIndex<u32>) {
        self.graph[state].accepting = true;
    }

    /// Determinizes a NFA to a DFA.
    /// This is done by creating a new DFA where each state is a set of states
    /// from the NFA. This respects epsilon transitions.
    pub fn determinize(&self) -> DFA<(), E> {
        let nfa_start = self.start.expect("NFA must have a start state");
        let mut state_map = HashMap::new();

        let mut dfa = DFA::new(self.alphabet.clone());

        // First we need to create the start state.
        let mut start_state_set = vec![nfa_start];
        self.extend_to_e_closure(&mut start_state_set);
        let dfa_start = dfa.add_state(self.state_from_set_no_data(&start_state_set));
        dfa.set_start(dfa_start);
        state_map.insert(start_state_set.clone(), dfa_start);

        // Second we need an explicit trap state.
        let trap_state_set = vec![];
        let trap_state = dfa.add_state(self.state_from_set_no_data(&trap_state_set));
        dfa.graph[trap_state].trap = true;
        state_map.insert(trap_state_set.clone(), trap_state);

        let mut stack = vec![start_state_set, trap_state_set];

        while let Some(state) = stack.pop() {
            for symbol in &self.alphabet {
                let mut target_state = vec![];

                for &node in &state {
                    for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                        if edge.weight().as_ref() == Some(symbol) {
                            target_state.push(edge.target());
                        }
                    }
                }

                self.extend_to_e_closure(&mut target_state);

                target_state.sort();
                target_state.dedup();

                let target_dfa_state = if let Some(&x) = state_map.get(&target_state) {
                    x
                } else {
                    let new_state = dfa.add_state(self.state_from_set_no_data(&target_state));
                    state_map.insert(target_state.clone(), new_state);
                    stack.push(target_state);
                    new_state
                };

                dfa.add_transition(state_map[&state], target_dfa_state, symbol.clone());
            }
        }

        #[cfg(debug_assertions)]
        dfa.assert_complete();

        dfa.override_complete();

        dfa
    }

    /// Calculates the epsilon closure of a set of states.
    /// This set is duplicate free.
    pub fn extend_to_e_closure(&self, states: &mut Vec<NodeIndex<u32>>) {
        let mut stack = states.clone();

        while let Some(state) = stack.pop() {
            for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                if edge.weight().is_none() {
                    let target = edge.target();

                    if !states.contains(&target) {
                        states.push(target);
                        stack.push(target);
                    }
                }
            }
        }
    }

    pub fn is_accepting(&self, state: NodeIndex<u32>) -> bool {
        self.graph[state].accepting
    }

    /// Checks if a set of states contains an accepting state.
    pub fn is_accepting_set(&self, states: &[NodeIndex<u32>]) -> bool {
        states.iter().any(|&x| self.is_accepting(x))
    }

    /// Creates a state from a set of states.
    pub fn state_from_set(&self, states: &[NodeIndex<u32>]) -> DfaNode<Vec<N>> {
        DfaNode::new(
            self.is_accepting_set(states),
            false,
            self.node_data_set(states),
        )
    }

    /// Creates a state from a set of states.
    pub fn state_from_set_no_data(&self, states: &[NodeIndex<u32>]) -> DfaNode<()> {
        DfaNode::new(self.is_accepting_set(states), false, ())
    }

    pub fn node_data(&self, node: NodeIndex<u32>) -> &N {
        self.graph[node].data()
    }

    /// Maps a set of states to their data.
    pub fn node_data_set(&self, nodes: &[NodeIndex<u32>]) -> Vec<N> {
        nodes.iter().map(|&x| self.node_data(x).clone()).collect()
    }

    pub fn to_graphviz(&self, edges: Option<impl EdgeListLike>) -> String {
        let mut dot = String::new();
        dot.push_str("digraph finite_state_machine {\n");
        dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
        dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("rankdir=LR;\n");
        dot.push_str("node [shape=point,label=\"\"]START\n");

        let accepting_states = self
            .graph
            .node_indices()
            .filter(|node| self.graph[*node].accepting)
            .collect::<Vec<_>>();

        dot.push_str(&format!(
            "node [shape = doublecircle]; {};\n",
            accepting_states
                .iter()
                .map(|node| node.index().to_string())
                .join(" ")
        ));
        dot.push_str("node [shape = circle];\n");

        if let Some(start) = self.start {
            dot.push_str(&format!("START -> {:?};\n", start.index()));
        }

        for edge in self.graph.edge_references() {
            let mut attrs = vec![(
                "label",
                format!("\"{:?} ({})\"", edge.weight(), edge.id().index()),
            )];

            if let Some(edges) = &edges
                && edges.has_edge(edge.id())
            {
                attrs.push(("color", "red".to_string()));
            }
            dot.push_str(&format!(
                "{:?} -> {:?} [ {} ];\n",
                edge.source().index(),
                edge.target().index(),
                attrs.iter().map(|(k, v)| format!("{}={}", k, v)).join(" ")
            ));
        }

        dot.push_str("}\n");

        dot
    }
}

impl<N: AutomatonNode, E: AutomatonEdge> AutBuild<NodeIndex, EdgeIndex, DfaNode<N>, Option<E>>
    for NFA<N, E>
{
    fn add_state(&mut self, data: DfaNode<N>) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(
        &mut self,
        from: NodeIndex<u32>,
        to: NodeIndex<u32>,
        label: Option<E>,
    ) -> EdgeIndex<u32> {
        self.graph.add_edge(from, to, label)
    }
}

impl<N: AutomatonNode, E: AutomatonEdge> Automaton<E> for NFA<N, E> {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a E>) -> bool
    where
        E: 'a,
    {
        let mut current_states = vec![self.start.expect("NFA must have a start state")];
        self.extend_to_e_closure(&mut current_states);

        for symbol in input {
            let mut next_states = vec![];

            for &state in &current_states {
                for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                    if edge.weight().as_ref() == Some(symbol) {
                        next_states.push(edge.target());
                    }
                }
            }

            if next_states.is_empty() {
                return false;
            }

            self.extend_to_e_closure(&mut next_states);

            current_states = next_states;
        }

        self.is_accepting_set(&current_states)
    }

    fn alphabet(&self) -> &Vec<E> {
        &self.alphabet
    }
}
