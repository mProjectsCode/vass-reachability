use std::fmt::Debug;

use petgraph::{
    graph::NodeIndex,
    stable_graph::StableDiGraph,
    visit::{EdgeRef, IntoEdgeReferences},
    Direction,
};

use super::{AutBuild, AutEdge, AutNode, Automaton};

#[derive(Debug, Clone, PartialEq)]
pub struct DfaNodeData<T: AutNode> {
    pub accepting: bool,
    pub data: T,
}

impl<T: AutNode> DfaNodeData<T> {
    pub fn new(accepting: bool, data: T) -> Self {
        DfaNodeData { accepting, data }
    }

    pub fn accepting(&self) -> bool {
        self.accepting
    }

    pub fn data(&self) -> &T {
        &self.data
    }

    pub fn invert(&self) -> Self {
        DfaNodeData::new(!self.accepting, self.data.clone())
    }

    pub fn and(&self, other: &Self) -> DfaNodeData<(T, T)> {
        DfaNodeData::new(
            self.accepting && other.accepting,
            (self.data.clone(), other.data.clone()),
        )
    }
}

#[derive(Debug, Clone)]
pub struct DFA<N: AutNode, E: AutEdge> {
    start: Option<NodeIndex<u32>>,
    graph: StableDiGraph<DfaNodeData<N>, E>,
    alphabet: Vec<E>,
}

impl<N: AutNode, E: AutEdge> DFA<N, E> {
    pub fn new(alphabet: Vec<E>) -> Self {
        let graph = StableDiGraph::new();

        DFA {
            alphabet,
            start: None,
            graph,
        }
    }

    pub fn set_start(&mut self, start: NodeIndex<u32>) {
        self.start = Some(start);
    }

    /// Adds a failure state if needed. This turns the DFA into a complete DFA, which is needed for some algorithms.
    pub fn add_failure_state(&mut self, data: N) -> Option<NodeIndex<u32>> {
        let mut failure_transitions = Vec::new();

        let mut state_map = std::collections::HashSet::new();
        assert!(self.start.is_some(), "DFA must have a start state");
        let mut stack = vec![self.start.unwrap()];

        while let Some(state) = stack.pop() {
            state_map.insert(state);

            for letter in self.alphabet.iter() {
                if self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .any(|edge| edge.weight() == letter)
                {
                    if !state_map.contains(&state) {
                        stack.push(state);
                    }
                } else {
                    failure_transitions.push((state, letter.clone()));
                }
            }
        }

        if !failure_transitions.is_empty() {
            let failure_state = self.add_state(DfaNodeData::new(false, data));

            for (state, letter) in failure_transitions {
                self.add_transition(state, failure_state, letter.clone());
            }

            for letter in self.alphabet.clone().iter() {
                self.add_transition(failure_state, failure_state, letter.clone());
            }

            return Some(failure_state);
        }

        None
    }

    /// Inverts self, creating a new DFA where the accepting states are inverted.
    pub fn invert(&self) -> DFA<N, E> {
        assert!(self.start.is_some(), "DFA must have a start state");

        let mut inverted = DFA::new(self.alphabet.clone());
        for node in self.graph.node_indices() {
            let new_node = inverted.add_state(self.graph[node].invert());

            if node == self.start.unwrap() {
                inverted.set_start(new_node);
            }
        }

        for edge in self.graph.edge_references() {
            inverted.add_transition(edge.source(), edge.target(), edge.weight().clone());
        }

        inverted
    }

    /// Builds an intersection DFA from two DFAs. Both DFAs must have the same alphabet.
    pub fn intersect(&self, other: DFA<N, E>) -> DFA<(N, N), E> {
        assert_eq!(
            self.alphabet, other.alphabet,
            "Alphabets must be the same to intersect DFAs"
        );

        assert!(self.start.is_some(), "Self must have a start state");
        assert!(other.start.is_some(), "Other must have a start state");

        let self_start = self.start.unwrap();
        let other_start = other.start.unwrap();

        // state map to map combinations of states to the new intersected states
        let mut state_map = std::collections::HashMap::new();

        // stack for the state combinations that still need to be processed
        let mut stack = vec![(self_start, other_start)];

        // the intersected DFA
        let mut intersected = DFA::new(self.alphabet.clone());

        let start_state =
            intersected.add_state(self.graph[self_start].and(&other.graph[other_start]));
        intersected.set_start(start_state);

        state_map.insert((self_start, other_start), intersected.start.unwrap());

        while let Some((state1, state2)) = stack.pop() {
            let new_state = state_map[&(state1, state2)];

            for edge1 in self.graph.edges_directed(state1, Direction::Outgoing) {
                for edge2 in other.graph.edges_directed(state2, Direction::Outgoing) {
                    if edge1.weight() == edge2.weight() {
                        let next_state = state_map
                            .entry((edge1.target(), edge2.target()))
                            .or_insert_with(|| {
                                let new_state = intersected.add_state(
                                    self.graph[edge1.target()].and(&other.graph[edge2.target()]),
                                );
                                stack.push((edge1.target(), edge2.target()));
                                new_state
                            });

                        intersected.add_transition(new_state, *next_state, edge1.weight().clone());
                    }
                }
            }
        }

        intersected
    }

    /// Not sure about this algorithm, but we first check if the graph has any accepting states. If it doesn't, we can return false immediately.
    /// Then we do a simple DFS from the start state, and if we find an accepting state, we return true.
    /// (the second part is probably not necessary, as there should only be one connected component and if that contains an accepting state,
    /// we should be able to reach it from the start state with some input, so we could always return true)
    pub fn has_accepting_run(&self) -> bool {
        if self
            .graph
            .node_indices()
            .all(|node| !self.graph[node].accepting)
        {
            return false;
        }

        let mut visited = std::collections::HashSet::new();
        assert!(self.start.is_some(), "Self must have a start state");
        let mut stack = vec![self.start.unwrap()];

        while let Some(state) = stack.pop() {
            if self.graph[state].accepting {
                return true;
            }

            visited.insert(state);

            for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                if !visited.contains(&edge.target()) {
                    stack.push(edge.target());
                }
            }
        }

        false
    }

    /// Checks if `L(Self) = ∅` by checking if there is no accepting run in the DFA.
    pub fn is_language_empty(&self) -> bool {
        !self.has_accepting_run()
    }

    /// Checks if self is a subset of other. Both must be complete DFAs with the same alphabet.
    ///
    /// The inclusion holds if there is no accepting run in the intersection of self and the inverse of other.
    /// `L(Self) ⊆ L(Other) iff L(Self) ∩ L(invert(Other)) = ∅`
    pub fn is_subset_of(&self, other: &DFA<N, E>) -> bool {
        let inverted = other.clone().invert();
        self.intersect(inverted).is_language_empty()
    }
}

impl<N: AutNode, E: AutEdge> AutBuild<NodeIndex, DfaNodeData<N>, E> for DFA<N, E> {
    fn add_state(&mut self, data: DfaNodeData<N>) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(&mut self, from: NodeIndex<u32>, to: NodeIndex<u32>, label: E) {
        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!("Transition conflict, adding the new transition causes this automaton to no longer be a DFA. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}", from, label, target, from, label, to);
            }
        }

        self.graph.add_edge(from, to, label);
    }
}

impl<N: AutNode, E: AutEdge> Automaton<E> for DFA<N, E> {
    fn accepts(&self, input: &[E]) -> bool {
        assert!(self.start.is_some(), "Self must have a start state");

        let mut current_state = Some(self.start.unwrap());
        for symbol in input {
            assert!(
                self.alphabet.contains(symbol),
                "Symbol {:?} not in alphabet",
                symbol
            );

            if let Some(state) = current_state {
                let next_state = self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|neighbor| neighbor.weight() == symbol)
                    .map(|edge| edge.target());
                current_state = next_state;
            } else {
                return false;
            }
        }

        match current_state.and_then(|state| self.graph.node_weight(state)) {
            Some(data) => data.accepting,
            None => false,
        }
    }
}
