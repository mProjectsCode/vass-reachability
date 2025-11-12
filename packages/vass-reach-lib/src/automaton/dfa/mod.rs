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
    AutBuild, Automaton, AutomatonEdge, AutomatonNode,
    index_map::IndexMap,
    nfa::NFA,
    path::{
        Path,
        path_like::{EdgeListLike, PathLike},
    },
};

pub mod minimization;
pub mod node;

#[derive(Clone)]
pub struct DFA<N: AutomatonNode, E: AutomatonEdge> {
    start: Option<NodeIndex<u32>>,
    pub graph: DiGraph<DfaNode<N>, E>,
    alphabet: Vec<E>,
    complete: bool,
}

impl<N: AutomatonNode, E: AutomatonEdge> DFA<N, E> {
    pub fn new(alphabet: Vec<E>) -> Self {
        let graph = DiGraph::new();

        DFA {
            alphabet,
            start: None,
            graph,
            complete: false,
        }
    }

    pub fn set_start(&mut self, start: NodeIndex<u32>) {
        self.start = Some(start);
    }

    pub fn get_start(&self) -> Option<NodeIndex<u32>> {
        self.start
    }

    pub fn is_complete(&self) -> bool {
        self.complete
    }

    /// Sets the DFA to be complete. This is useful when we don't want to spend
    /// the time to check if the DFA is complete.
    pub fn override_complete(&mut self) {
        self.complete = true;
    }

    pub fn state_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_weight(&self, edge: EdgeIndex<u32>) -> &E {
        self.graph.edge_weight(edge).unwrap()
    }

    pub fn get_edge(&self, from: NodeIndex, to: NodeIndex, weight: &E) -> Option<EdgeIndex> {
        for edge in self.graph.edges_connecting(from, to) {
            if edge.weight() == weight {
                return Some(edge.id());
            }
        }

        None
    }

    /// Adds a failure state if needed. This turns the DFA into a complete DFA,
    /// which is needed for some algorithms.
    pub fn add_failure_state(&mut self, data: N) -> Option<NodeIndex<u32>> {
        let mut failure_transitions = Vec::new();

        for state in self.graph.node_indices() {
            for letter in self.alphabet.iter() {
                let edge = self
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|edge| edge.weight() == letter);

                if edge.is_none() {
                    failure_transitions.push((state, letter.clone()));
                }
            }
        }

        if !failure_transitions.is_empty() {
            let failure_state = self.add_state(DfaNode::new(false, true, data));

            for (state, letter) in failure_transitions {
                self.add_transition(state, failure_state, letter.clone());
            }

            for letter in self.alphabet.clone().iter() {
                self.add_transition(failure_state, failure_state, letter.clone());
            }

            self.complete = true;

            return Some(failure_state);
        }

        self.complete = true;

        None
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
                    .find(|edge| edge.weight() == letter);

                assert!(
                    edge.is_some(),
                    "DFA is not complete. State {:?} does not have a transition for letter {:?}",
                    state,
                    letter
                );
            }
        }
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

        for node in trapping {
            self.graph.remove_node(node);
        }
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
            let new_node = inverted.add_state(self.graph[node].invert());

            if node == self.start.unwrap() {
                inverted.set_start(new_node);
            }
        }

        for edge in self.graph.edge_references() {
            inverted.add_transition(edge.source(), edge.target(), edge.weight().clone());
        }

        inverted.override_complete();

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

        let mut reversed = NFA::new(self.alphabet.clone());
        let start = reversed.add_state(DfaNode::default());
        reversed.set_start(start);

        let mut node_hash = IndexMap::new(self.state_count());

        for node in self.graph.node_indices() {
            let new_node = reversed.add_state(DfaNode::default());
            node_hash.insert(node, new_node);

            if node == self.start.unwrap() {
                reversed.set_accepting(new_node);
            }

            if self.graph[node].accepting {
                reversed.add_transition(start, new_node, None);
            }
        }

        for edge in self.graph.edge_references() {
            let source = node_hash.get(edge.target());
            let target = node_hash.get(edge.source());

            reversed.add_transition(*source, *target, Some(edge.weight().clone()));
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
            intersected.add_state(self.graph[self_start].join_left(&other.graph[other_start]));
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
                                    self.graph[edge1.target()]
                                        .join_left(&other.graph[edge2.target()]),
                                );
                                stack.push((edge1.target(), edge2.target()));
                                new_state
                            });

                        intersected.add_transition(new_state, *next_state, edge1.weight().clone());
                    }
                }
            }
        }

        intersected.override_complete();

        intersected
    }

    pub fn bfs(
        &self,
        start: Option<NodeIndex>,
        is_target: impl Fn(NodeIndex<u32>, &DfaNode<N>) -> bool,
    ) -> Vec<Path> {
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

    pub fn bfs_accepting_states(&self, start: Option<NodeIndex>) -> Vec<Path> {
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

    pub fn find_loop_rooted_in_node(&self, node: NodeIndex<u32>) -> Option<Path> {
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
    ) -> Vec<Path> {
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

impl<N: AutomatonNode, E: AutomatonEdge> AutBuild<NodeIndex, EdgeIndex, DfaNode<N>, E>
    for DFA<N, E>
{
    fn add_state(&mut self, data: DfaNode<N>) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(
        &mut self,
        from: NodeIndex<u32>,
        to: NodeIndex<u32>,
        label: E,
    ) -> EdgeIndex<u32> {
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
}

impl<N: AutomatonNode, E: AutomatonEdge> Automaton<E> for DFA<N, E> {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a E>) -> bool
    where
        E: 'a,
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

    fn alphabet(&self) -> &Vec<E> {
        &self.alphabet
    }
}

impl<N: AutomatonNode, E: AutomatonEdge> Debug for DFA<N, E> {
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
