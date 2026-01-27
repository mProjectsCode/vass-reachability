use std::{collections::VecDeque, fmt::Debug, vec};

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;
use node::DfaNode;
use petgraph::{
    Direction,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use crate::automaton::{
    Alphabet, Automaton, AutomatonEdge, AutomatonNode, Deterministic, ExplicitEdgeAutomaton,
    FromLetter, Frozen, InitializedAutomaton, Language, ModifiableAutomaton,
    index_map::IndexMap,
    nfa::{NFA, NFAEdge},
    path::Path,
};

pub mod minimization;
pub mod node;

#[derive(Clone)]
pub struct DFA<N: AutomatonNode, E: AutomatonEdge + FromLetter> {
    start: Option<NodeIndex>,
    pub graph: DiGraph<DfaNode<N>, E>,
    alphabet: Vec<E::Letter>,
    complete: bool,
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> DFA<N, E> {
    pub fn new(alphabet: Vec<E::Letter>) -> Self {
        let graph = DiGraph::new();

        DFA {
            alphabet,
            start: None,
            graph,
            complete: false,
        }
    }

    pub fn set_initial(&mut self, node: NodeIndex) {
        self.start = Some(node);
    }

    pub fn is_trap(&self, node: NodeIndex) -> bool {
        self.graph[node].trap
    }

    pub fn is_complete(&self) -> bool {
        #[cfg(not(debug_assertions))]
        return self.complete;

        #[cfg(debug_assertions)]
        {
            let is_complete = self.check_complete();
            assert_eq!(is_complete, self.complete);
            return is_complete;
        }
    }

    /// Sets the DFA to be complete. This is useful when we don't want to spend
    /// the time to check if the DFA is complete.
    pub fn set_complete_unchecked(&mut self) {
        #[cfg(debug_assertions)]
        {
            assert!(self.check_complete());
        }

        self.complete = true;
    }

    /// Check if the DFA is complete.
    /// This means that every state has a transition for every letter in the
    /// alphabet.
    pub fn check_complete(&self) -> bool {
        for state in self.graph.node_indices() {
            for letter in self.alphabet.iter() {
                let edge = self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|edge| edge.weight().matches(letter));

                if edge.is_none() {
                    return false;
                }
            }
        }

        true
    }

    /// Assert that the DFA is complete.
    /// This means that every state has a transition for every letter in the
    /// alphabet.
    ///
    /// If the DFA is not complete, this function will panic.
    pub fn assert_complete(&self) {
        for state in self.graph.node_indices() {
            for letter in self.alphabet.iter() {
                let edge = self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|edge| edge.weight().matches(letter));

                assert!(
                    edge.is_some(),
                    "DFA is not complete. State {:?} does not have a transition for letter {:?}",
                    state,
                    letter
                );
            }
        }
    }

    /// Adds a failure state if needed. This turns the DFA into a complete DFA,
    /// which is needed for some algorithms.
    pub fn make_complete(&mut self, data: N) -> Option<NodeIndex<u32>> {
        let mut failure_transitions = Vec::new();

        for state in self.graph.node_indices() {
            for letter in self.alphabet.iter() {
                let edge = self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|edge| edge.weight().matches(letter));

                if edge.is_none() {
                    failure_transitions.push((state, letter.clone()));
                }
            }
        }

        if !failure_transitions.is_empty() {
            let failure_state = self.add_node(DfaNode::new(false, true, data));

            for (state, letter) in failure_transitions {
                self.add_edge(state, failure_state, E::from_letter(&letter));
            }

            for letter in self.alphabet.clone().iter() {
                self.add_edge(failure_state, failure_state, E::from_letter(letter));
            }

            self.complete = true;

            return Some(failure_state);
        }

        self.complete = true;

        None
    }

    /// Remove all trapping states from the DFA. A trapping state is a state
    /// from which a final state can never be reached.
    pub fn remove_trapping_states(&mut self) {
        let mut trapping = HashSet::new();
        let mut non_trapping = HashSet::new();

        for node in self.graph.node_indices() {
            if trapping.contains(&node) || non_trapping.contains(&node) {
                continue;
            }

            let paths = self.bfs(Some(node), |index, data| {
                data.accepting || non_trapping.contains(&index)
            });

            if paths.is_empty() {
                trapping.insert(node);
            } else {
                for path in paths {
                    for (_, node) in path {
                        non_trapping.insert(node);
                    }
                }
            }
        }

        self.graph.retain_nodes(|_, n| !trapping.contains(&n));
    }

    /// Inverts self, creating a new DFA where the accepting states are
    /// inverted. The DFA must have a start state and be complete.
    ///
    /// See [`DFA::invert_mut`] for a version that modifies the DFA in place.
    pub fn invert(&self) -> DFA<N, E> {
        assert!(self.start.is_some(), "DFA must have a start state");
        assert!(self.complete, "DFA must be complete to invert");

        let mut inverted = DFA::new(self.alphabet.clone());
        for node in self.graph.node_indices() {
            let new_node = inverted.add_node(self.graph[node].invert());

            if node == self.start.unwrap() {
                inverted.set_initial(new_node);
            }
        }

        for edge in self.graph.edge_references() {
            inverted.add_edge(edge.source(), edge.target(), edge.weight().clone());
        }

        inverted.set_complete_unchecked();

        inverted
    }

    /// Inverts self, modifying the DFA in place. The DFA must be complete.
    ///
    /// See [`DFA::invert`] for a version that creates a new DFA.
    pub fn invert_mut(&mut self) {
        assert!(self.complete, "DFA must be complete to invert");

        for node in self.graph.node_indices() {
            self.graph[node].invert_mut();
        }
    }

    pub fn reverse_nfa(&self) -> NFA<(), E> {
        assert!(self.start.is_some(), "DFA must have a start state");
        assert!(self.complete, "DFA must be complete to reverse");

        let mut reversed = NFA::<(), E>::new(self.alphabet.clone());
        let start = reversed.add_node(DfaNode::default());
        reversed.set_initial(start);

        let mut node_hash = IndexMap::new(self.node_count());

        for node in self.graph.node_indices() {
            let new_node = reversed.add_node(DfaNode::default());
            node_hash.insert(node, new_node);

            if node == self.start.unwrap() {
                reversed.set_accepting(new_node);
            }

            if self.graph[node].accepting {
                reversed.add_edge(start, new_node, NFAEdge::Epsilon);
            }
        }

        for edge in self.graph.edge_references() {
            let source = node_hash.get(edge.target());
            let target = node_hash.get(edge.source());

            reversed.add_edge(*source, *target, NFAEdge::Symbol(edge.weight().clone()));
        }

        reversed
    }

    pub fn reverse(&self) -> DFA<(), E> {
        self.reverse_nfa().determinize()
    }

    /// Builds an intersection DFA from two DFAs. Both DFAs must have the same
    /// alphabet, a start state, and they must be complete.
    pub fn intersect<NO: AutomatonNode>(&self, other: &DFA<NO, E>) -> DFA<N, E> {
        assert!(self.start.is_some(), "Self must have a start state");
        assert!(other.start.is_some(), "Other must have a start state");

        assert!(self.complete, "Self must be complete to intersect");
        assert!(other.complete, "Other must be complete to intersect");

        // println!("Checking self completeness");
        // self.assert_complete();
        // println!("Checking other completeness");
        // other.assert_complete();

        let mut alphabet_cl = self.alphabet.clone();
        let mut other_alphabet_cl = other.alphabet.clone();

        alphabet_cl.sort();
        other_alphabet_cl.sort();

        assert_eq!(
            alphabet_cl, other_alphabet_cl,
            "Alphabets must be the same to intersect DFAs"
        );

        let self_start = self.start.unwrap();
        let other_start = other.start.unwrap();

        // state map to map combinations of states to the new intersected states
        let mut state_map = HashMap::new();

        // stack for the state combinations that still need to be processed
        let mut stack = vec![(self_start, other_start)];

        // the intersected DFA
        let mut intersected = DFA::new(self.alphabet.clone());

        let start_state =
            intersected.add_node(self.graph[self_start].join_left(&other.graph[other_start]));
        intersected.set_initial(start_state);

        state_map.insert((self_start, other_start), intersected.start.unwrap());

        while let Some((state1, state2)) = stack.pop() {
            let new_state = state_map[&(state1, state2)];

            for edge1 in self.graph.edges_directed(state1, Direction::Outgoing) {
                for edge2 in other.graph.edges_directed(state2, Direction::Outgoing) {
                    if edge1.weight() == edge2.weight() {
                        let next_state = state_map
                            .entry((edge1.target(), edge2.target()))
                            .or_insert_with(|| {
                                let new_state = intersected.add_node(
                                    self.graph[edge1.target()]
                                        .join_left(&other.graph[edge2.target()]),
                                );
                                stack.push((edge1.target(), edge2.target()));
                                new_state
                            });

                        intersected.add_edge(new_state, *next_state, edge1.weight().clone());
                    }
                }
            }
        }

        intersected.set_complete_unchecked();

        intersected
    }

    pub fn bfs(
        &self,
        start: Option<NodeIndex>,
        is_target: impl Fn(NodeIndex, &DfaNode<N>) -> bool,
    ) -> Vec<Path<NodeIndex, EdgeIndex>> {
        let mut visited = HashSet::new();
        let mut queue = VecDeque::new();
        let mut paths = Vec::new();

        let start = start.unwrap_or(self.start.expect("Self must have a start state"));
        queue.push_back(Path::new(start));

        while let Some(path) = queue.pop_front() {
            let last = path.end();

            if is_target(last, &self.graph[last]) {
                paths.push(path.clone());
            }

            visited.insert(last);

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                if !visited.contains(&edge.target()) {
                    let mut new_path = path.clone();
                    new_path.add(edge.id(), edge.target());
                    queue.push_back(new_path);
                }
            }
        }

        paths
    }

    pub fn bfs_accepting_states(
        &self,
        start: Option<NodeIndex>,
    ) -> Vec<Path<NodeIndex, EdgeIndex>> {
        self.bfs(start, |_, data| data.accepting)
    }

    /// Not sure about this algorithm, but we first check if the graph has any
    /// accepting states. If it doesn't, we can return false immediately.
    /// Then we do a simple DFS from the start state, and if we find an
    /// accepting state, we return true. (the second part is probably not
    /// necessary, as there should only be one connected component and if that
    /// contains an accepting state, we should be able to reach it from the
    /// start state with some input, so we could always return true)
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

    /// Checks if `L(Self) = ∅` by checking if there is no accepting run in the
    /// DFA.
    pub fn is_language_empty(&self) -> bool {
        !self.has_accepting_run()
    }

    /// Checks if self is a subset of other. Both must be complete DFAs with the
    /// same alphabet.
    ///
    /// The inclusion holds if there is no accepting run in the intersection of
    /// self and the inverse of other. `L(Self) ⊆ L(Other) iff L(Self) ∩
    /// L(invert(Other)) = ∅`
    pub fn is_subset_of<NO: AutomatonNode>(&self, other: &DFA<NO, E>) -> bool {
        let mut inverted = other.clone();
        inverted.invert_mut();
        let intersection = self.intersect(&inverted);
        // dbg!(&intersection);
        // println!("{:?}", Dot::new(&intersection.graph));

        intersection.is_language_empty()
    }

    pub fn find_loop_rooted_in_node(
        &self,
        node: NodeIndex<u32>,
    ) -> Option<Path<NodeIndex, EdgeIndex>> {
        let mut visited = HashSet::new();
        let mut stack = VecDeque::new();
        stack.push_back(Path::new(node));

        while let Some(mut path) = stack.pop_front() {
            let last = path.end();

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                let target = edge.target();

                if path.start() == target {
                    path.add(edge.id(), target);
                    return Some(path);
                }

                match visited.entry(target) {
                    hashbrown::hash_set::Entry::Occupied(_) => {}
                    hashbrown::hash_set::Entry::Vacant(entry) => {
                        entry.insert();
                        let mut new_path = path.clone();
                        new_path.add(edge.id(), target);
                        stack.push_back(new_path);
                    }
                }
            }
        }

        None
    }

    pub fn find_loops_rooted_in_node(
        &self,
        node: NodeIndex<u32>,
        length_limit: Option<usize>,
    ) -> Vec<Path<NodeIndex, EdgeIndex>> {
        let mut stack = VecDeque::new();
        let mut loops = Vec::new();
        stack.push_back(Path::new(node));

        while let Some(mut path) = stack.pop_front() {
            let last = path.end();

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                let target = edge.target();

                if path.start() == target {
                    path.add(edge.id(), target);
                    loops.push(path.clone());
                    continue;
                }

                if !path.transitions_contain_node(target)
                    && path.len() < length_limit.unwrap_or(usize::MAX)
                {
                    let mut new_path = path.clone();
                    new_path.add(edge.id(), target);
                    stack.push_back(new_path);
                }
            }
        }

        loops
    }

    pub fn subgraph(&self, nodes: &[NodeIndex]) -> DFA<N, E>
    where
        N: Clone,
        E: Clone,
    {
        let mut sub_dfa = DFA::new(self.alphabet.clone());
        // We could use an index map here, but we map from self to the new DFA, where
        // the new DFA might be a lot smaller, so an index map might be wasteful.
        let mut node_map = HashMap::new();

        for &node in nodes {
            let new_node = sub_dfa.add_node(self.graph[node].clone());
            node_map.insert(node, new_node);
        }

        for &node in nodes {
            for edge in self.graph.edges_directed(node, Direction::Outgoing) {
                if nodes.contains(&edge.target()) {
                    let from = node_map[&node];
                    let to = node_map[&edge.target()];
                    sub_dfa.add_edge(from, to, edge.weight().clone());
                }
            }
        }

        if let Some(start) = self.start {
            if nodes.contains(&start) {
                sub_dfa.set_initial(node_map[&start]);
            }
        }

        sub_dfa
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Alphabet for DFA<N, E> {
    type Letter = E::Letter;

    fn alphabet(&self) -> &[Self::Letter] {
        &self.alphabet
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Automaton<Deterministic> for DFA<N, E> {
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

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> ExplicitEdgeAutomaton<Deterministic>
    for DFA<N, E>
{
    type EIndex = EdgeIndex;
    type E = E;

    fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    fn get_edge(&self, index: Self::EIndex) -> Option<&E> {
        self.graph.edge_weight(index)
    }

    fn get_edge_unchecked(&self, index: Self::EIndex) -> &E {
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

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> ModifiableAutomaton<Deterministic>
    for DFA<N, E>
{
    fn add_node(&mut self, data: DfaNode<N>) -> Self::NIndex {
        self.graph.add_node(data)
    }

    fn add_edge(&mut self, from: Self::NIndex, to: Self::NIndex, label: E) -> Self::EIndex {
        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!(
                    "Transition conflict, adding the new transition causes this automaton to no longer be a DFA. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}",
                    from, label, target, from, label, to
                );
            }
        }

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

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> InitializedAutomaton<Deterministic>
    for DFA<N, E>
{
    fn get_initial(&self) -> Self::NIndex {
        self.start.expect("Self must have a start state")
    }

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        self.get_node_unchecked(node).accepting
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Language for DFA<N, E> {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a Self::Letter>) -> bool
    where
        Self::Letter: 'a,
    {
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
                    .find(|neighbor| neighbor.weight().matches(symbol))
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

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Debug for DFA<N, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DFA")
            .field("alphabet", &self.alphabet)
            .field("state_count", &self.graph.node_count())
            .field(
                "states",
                &self
                    .graph
                    .node_indices()
                    .map(|node| (&self.graph[node].data, node))
                    .collect_vec(),
            )
            .field("initial_state", &self.start)
            .field(
                "final_states",
                &self
                    .graph
                    .node_indices()
                    .filter(|node| self.graph[*node].accepting)
                    .collect_vec(),
            )
            .field("edge_count", &self.graph.edge_count())
            .field(
                "edges",
                &self
                    .graph
                    .edge_references()
                    .map(|edge| {
                        (
                            format!(
                                "{:?} --- {:?} --> {:?}",
                                edge.source(),
                                edge.weight(),
                                edge.target()
                            ),
                            edge.id(),
                        )
                    })
                    .collect_vec(),
            )
            .finish()
    }
}
