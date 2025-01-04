use hashbrown::HashMap;

use petgraph::{
    graph::{DiGraph, NodeIndex},
    visit::EdgeRef,
    Direction,
};

use super::{
    dfa::{DfaNodeData, DFA},
    AutBuild, AutEdge, AutNode, Automaton,
};

#[derive(Debug, Clone)]
pub struct NFA<N: AutNode, E: AutEdge> {
    start: Option<NodeIndex<u32>>,
    pub graph: DiGraph<DfaNodeData<N>, Option<E>>,
    alphabet: Vec<E>,
}

impl<N: AutNode, E: AutEdge> NFA<N, E> {
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

    /// Determinizes a NFA to a DFA.
    /// This is done by creating a new DFA where each state is a set of states from the NFA.
    /// This respects epsilon transitions.
    pub fn determinize(&self) -> DFA<Vec<N>, E> {
        let nfa_start = self.start.expect("NFA must have a start state");
        let mut state_map = HashMap::new();

        let mut dfa = DFA::new(self.alphabet.clone());

        let start_state_set = self.e_closure(nfa_start);
        let dfa_start = dfa.add_state(self.state_from_set(&start_state_set));
        dfa.set_start(dfa_start);

        state_map.insert(start_state_set.clone(), dfa_start);

        let mut stack = vec![start_state_set];

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

                if target_state.is_empty() {
                    continue;
                }

                target_state.sort();
                target_state.dedup();

                let target_dfa_state = if let Some(&x) = state_map.get(&target_state) {
                    x
                } else {
                    let new_state = dfa.add_state(self.state_from_set(&target_state));
                    state_map.insert(target_state.clone(), new_state);
                    stack.push(target_state);
                    new_state
                };

                dfa.add_transition(state_map[&state], target_dfa_state, symbol.clone());
            }
        }

        dfa.override_complete();

        dfa
    }

    /// Determinizes a NFA to a DFA.
    /// This is done by creating a new DFA where each state is a set of states from the NFA.
    /// This respects epsilon transitions.
    pub fn determinize_no_state_data(&self) -> DFA<(), E> {
        let nfa_start = self.start.expect("NFA must have a start state");
        let mut state_map = HashMap::new();

        let mut dfa = DFA::new(self.alphabet.clone());

        let start_state_set = self.e_closure(nfa_start);
        let dfa_start = dfa.add_state(self.state_from_set_no_data(&start_state_set));
        dfa.set_start(dfa_start);

        state_map.insert(start_state_set.clone(), dfa_start);

        let mut stack = vec![start_state_set];

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

                if target_state.is_empty() {
                    continue;
                }

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

        dfa.override_complete();

        dfa
    }

    /// Calculates the epsilon closure of a state.
    /// Meaning all states reachable only by epsilon transitions.
    pub fn e_closure(&self, state: NodeIndex<u32>) -> Vec<NodeIndex<u32>> {
        let mut stack = vec![state];
        let mut closure = vec![state];

        while let Some(state) = stack.pop() {
            for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                if edge.weight().is_none() {
                    let target = edge.target();

                    if !closure.contains(&target) {
                        closure.push(target);
                        stack.push(target);
                    }
                }
            }
        }

        closure
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
        self.graph[state].accepting()
    }

    // checks if a set of states contains an accepting state
    pub fn is_accepting_set(&self, states: &[NodeIndex<u32>]) -> bool {
        states.iter().any(|&x| self.is_accepting(x))
    }

    // creates a state from a set of states
    pub fn state_from_set(&self, states: &[NodeIndex<u32>]) -> DfaNodeData<Vec<N>> {
        DfaNodeData::new(self.is_accepting_set(states), self.node_data_set(states))
    }

    // creates a state from a set of states
    pub fn state_from_set_no_data(&self, states: &[NodeIndex<u32>]) -> DfaNodeData<()> {
        DfaNodeData::new(self.is_accepting_set(states), ())
    }

    pub fn node_data(&self, node: NodeIndex<u32>) -> &N {
        self.graph[node].data()
    }

    // maps a set of states to their data
    pub fn node_data_set(&self, nodes: &[NodeIndex<u32>]) -> Vec<N> {
        nodes.iter().map(|&x| self.node_data(x).clone()).collect()
    }
}

impl<N: AutNode, E: AutEdge> AutBuild<NodeIndex, DfaNodeData<N>, Option<E>> for NFA<N, E> {
    fn add_state(&mut self, data: DfaNodeData<N>) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(&mut self, from: NodeIndex<u32>, to: NodeIndex<u32>, label: Option<E>) {
        self.graph.add_edge(from, to, label);
    }
}

impl<N: AutNode, E: AutEdge> Automaton<E> for NFA<N, E> {
    fn accepts(&self, input: &[E]) -> bool {
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
