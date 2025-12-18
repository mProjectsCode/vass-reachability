use hashbrown::HashMap;
use petgraph::{
    Direction,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::automaton::{
    Alphabet, Automaton, AutomatonEdge, AutomatonNode, ExplicitEdgeAutomaton, FromLetter, Frozen,
    InitializedAutomaton, Language, ModifiableAutomaton,
    dfa::{DFA, node::DfaNode},
};

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NFAEdge<E: AutomatonEdge> {
    Symbol(E),
    Epsilon,
}

impl<E: AutomatonEdge + FromLetter> NFAEdge<E> {
    pub fn is_epsilon(&self) -> bool {
        matches!(self, NFAEdge::Epsilon)
    }
}

impl<E: AutomatonEdge + FromLetter> From<Option<E>> for NFAEdge<E> {
    fn from(value: Option<E>) -> Self {
        match value {
            Some(e) => NFAEdge::Symbol(e),
            None => NFAEdge::Epsilon,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NFA<N: AutomatonNode, E: AutomatonEdge + FromLetter> {
    start: Option<NodeIndex>,
    pub graph: DiGraph<DfaNode<N>, NFAEdge<E>>,
    alphabet: Vec<E::Letter>,
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> NFA<N, E> {
    pub fn new(alphabet: Vec<E::Letter>) -> Self {
        let graph = DiGraph::new();

        NFA {
            alphabet,
            start: None,
            graph,
        }
    }

    pub fn set_start(&mut self, start: NodeIndex) {
        self.start = Some(start);
    }

    pub fn set_accepting(&mut self, state: NodeIndex) {
        self.graph[state].accepting = true;
    }

    /// Determinizes a NFA to a DFA.
    /// This is done by creating a new DFA where each state is a set of states
    /// from the NFA. This respects epsilon transitions.
    pub fn determinize(&self) -> DFA<(), E> {
        let nfa_start = self.start.expect("NFA must have a start state");
        let mut state_map = HashMap::new();

        let mut dfa = DFA::<(), E>::new(self.alphabet.clone());

        // First we need to create the start state.
        let mut start_state_set = vec![nfa_start];
        self.extend_to_e_closure(&mut start_state_set);
        let dfa_start = dfa.add_node(self.state_from_set(&start_state_set));
        dfa.set_initial(dfa_start);
        state_map.insert(start_state_set.clone(), dfa_start);

        // Second we need an explicit trap state.
        let trap_state_set = vec![];
        let trap_state = dfa.add_node(self.state_from_set(&trap_state_set));
        dfa.graph[trap_state].trap = true;
        state_map.insert(trap_state_set.clone(), trap_state);

        let mut stack = vec![start_state_set, trap_state_set];

        while let Some(state) = stack.pop() {
            for symbol in &self.alphabet {
                let mut target_state = vec![];

                for &node in &state {
                    for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                        if edge.weight().matches(symbol) {
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
                    let new_state = dfa.add_node(self.state_from_set(&target_state));
                    state_map.insert(target_state.clone(), new_state);
                    stack.push(target_state);
                    new_state
                };

                dfa.add_edge(state_map[&state], target_dfa_state, E::from_letter(symbol));
            }
        }

        #[cfg(debug_assertions)]
        dfa.assert_complete();

        dfa.set_complete_unchecked();

        dfa
    }

    /// Calculates the epsilon closure of a set of states.
    /// This set is duplicate free.
    pub fn extend_to_e_closure(&self, states: &mut Vec<NodeIndex>) {
        let mut stack = states.clone();

        while let Some(state) = stack.pop() {
            for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                if edge.weight().is_epsilon() {
                    let target = edge.target();

                    if !states.contains(&target) {
                        states.push(target);
                        stack.push(target);
                    }
                }
            }
        }
    }

    pub fn is_accepting(&self, state: NodeIndex) -> bool {
        self.graph[state].accepting
    }

    /// Checks if a set of states contains an accepting state.
    pub fn is_accepting_set(&self, states: &[NodeIndex]) -> bool {
        states.iter().any(|&x| self.is_accepting(x))
    }

    /// Creates a state from a set of states.
    pub fn state_from_set(&self, states: &[NodeIndex<u32>]) -> DfaNode<()> {
        DfaNode::new(self.is_accepting_set(states), false, ())
    }

    pub fn node_data(&self, node: NodeIndex) -> &N {
        self.graph[node].data()
    }

    /// Maps a set of states to their data.
    pub fn node_data_set(&self, nodes: &[NodeIndex]) -> Vec<N> {
        nodes.iter().map(|&x| self.node_data(x).clone()).collect()
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Alphabet for NFA<N, E> {
    type Letter = E::Letter;

    fn alphabet(&self) -> &[E::Letter] {
        self.alphabet.as_slice()
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Automaton for NFA<N, E> {
    type NIndex = NodeIndex;
    type N = DfaNode<N>;

    fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    fn get_node(&self, index: Self::NIndex) -> Option<&DfaNode<N>> {
        self.graph.node_weight(index)
    }

    fn get_node_unchecked(&self, index: Self::NIndex) -> &DfaNode<N> {
        &self.graph[index]
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> ExplicitEdgeAutomaton for NFA<N, E> {
    type EIndex = EdgeIndex;

    type E = NFAEdge<E>;

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn get_edge(&self, index: Self::EIndex) -> Option<&NFAEdge<E>> {
        self.graph.edge_weight(index)
    }

    fn get_edge_unchecked(&self, index: Self::EIndex) -> &NFAEdge<E> {
        self.graph.edge_weight(index).unwrap()
    }

    fn edge_endpoints(&self, edge: Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)> {
        self.graph.edge_endpoints(edge)
    }

    fn edge_endpoints_unchecked(&self, edge: Self::EIndex) -> (Self::NIndex, Self::NIndex) {
        self.graph.edge_endpoints(edge).unwrap()
    }

    fn outgoing_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, Direction::Outgoing)
            .map(|edge| edge.id())
    }

    fn incoming_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.graph
            .edges_directed(node, Direction::Incoming)
            .map(|edge| edge.id())
    }

    fn connecting_edge_indices(
        &self,
        from: Self::NIndex,
        to: Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex> {
        self.graph.edges_connecting(from, to).map(|edge| edge.id())
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> ModifiableAutomaton for NFA<N, E> {
    fn add_node(&mut self, data: DfaNode<N>) -> Self::NIndex {
        self.graph.add_node(data)
    }

    fn add_edge(
        &mut self,
        from: Self::NIndex,
        to: Self::NIndex,
        label: NFAEdge<E>,
    ) -> Self::EIndex {
        self.graph.add_edge(from, to, label)
    }

    fn remove_node(&mut self, node: Self::NIndex) {
        self.graph.remove_node(node);
    }

    fn remove_edge(&mut self, edge: Self::EIndex) {
        self.graph.remove_edge(edge);
    }

    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool,
    {
        for index in self.iter_node_indices().rev() {
            if !f(Frozen::from(&mut *self), index) {
                self.remove_node(index);
            }
        }
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> InitializedAutomaton for NFA<N, E> {
    fn get_initial(&self) -> Self::NIndex {
        self.start.expect("Self must have a start state")
    }

    fn set_initial(&mut self, node: Self::NIndex) {
        self.start = Some(node);
    }

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        self.get_node(node)
            .map(|n| n.accepting)
            .expect("Node should be part of the NFA")
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Language for NFA<N, E> {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a E::Letter>) -> bool
    where
        E::Letter: 'a + Eq,
    {
        let mut current_states = vec![self.start.expect("NFA must have a start state")];
        self.extend_to_e_closure(&mut current_states);

        for symbol in input {
            let mut next_states = vec![];

            for &state in &current_states {
                for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                    if edge.weight().matches(symbol) {
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
}
