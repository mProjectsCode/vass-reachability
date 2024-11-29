use std::{
    collections::{hash_map::Entry, HashMap},
    fmt::Debug,
    vec,
};

use itertools::Itertools;
use petgraph::{
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
    Direction,
};

use super::{path::Path, AutBuild, AutEdge, AutNode, Automaton};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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

    pub fn and<TO: AutNode>(&self, other: &DfaNodeData<TO>) -> DfaNodeData<(T, TO)> {
        DfaNodeData::new(
            self.accepting && other.accepting,
            (self.data.clone(), other.data.clone()),
        )
    }
}

#[derive(Clone)]
pub struct DFA<N: AutNode, E: AutEdge> {
    start: Option<NodeIndex<u32>>,
    graph: DiGraph<DfaNodeData<N>, E>,
    alphabet: Vec<E>,
    is_complete: bool,
}

impl<N: AutNode, E: AutEdge> DFA<N, E> {
    pub fn new(alphabet: Vec<E>) -> Self {
        let graph = DiGraph::new();

        DFA {
            alphabet,
            start: None,
            graph,
            is_complete: false,
        }
    }

    pub fn set_start(&mut self, start: NodeIndex<u32>) {
        self.start = Some(start);
    }

    /// Sets the DFA to be complete. This is useful when we don't want to spend the time to check if the DFA is complete.
    pub fn override_complete(&mut self) {
        self.is_complete = true;
    }

    pub fn state_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn edge_weight(&self, edge: EdgeIndex<u32>) -> &E {
        self.graph.edge_weight(edge).unwrap()
    }

    /// Adds a failure state if needed. This turns the DFA into a complete DFA, which is needed for some algorithms.
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
            let failure_state = self.add_state(DfaNodeData::new(false, data));

            for (state, letter) in failure_transitions {
                self.add_transition(state, failure_state, letter.clone());
            }

            for letter in self.alphabet.clone().iter() {
                self.add_transition(failure_state, failure_state, letter.clone());
            }

            self.is_complete = true;

            return Some(failure_state);
        }

        self.is_complete = true;

        None
    }

    pub fn minimize(&self) -> DFA<N, E> {
        assert!(self.start.is_some(), "Self must have a start state");
        assert!(self.is_complete, "Self must be complete to minimize");

        // first we want to remove unreachable states
        let mut reachable = DFA::new(self.alphabet.clone());

        let mut visited = HashMap::new();
        let mut stack = vec![self.start.unwrap()];
        let new_start = reachable.add_state(self.graph[self.start.unwrap()].clone());
        reachable.set_start(new_start);
        visited.insert(self.start.unwrap(), new_start);

        while let Some(state) = stack.pop() {
            for edge in self.graph.edges_directed(state, Direction::Outgoing) {
                let new_from_state = *visited.get(&state).unwrap();

                if let Entry::Vacant(e) = visited.entry(edge.target()) {
                    // create a new state and add it to the visited map
                    let new_to_state = reachable.add_state(self.graph[edge.target()].clone());
                    e.insert(new_to_state);
                    stack.push(edge.target());

                    reachable.add_transition(new_from_state, new_to_state, edge.weight().clone());
                } else {
                    let new_to_state = *visited.get(&edge.target()).unwrap();
                    reachable.add_transition(new_from_state, new_to_state, edge.weight().clone());
                }
            }
        }

        // second we need to merge equivalent states

        let mut table = DfaMinimizationTable::new(self);

        for node in reachable.graph.node_indices() {
            let mut entry = DfaMinimizationTableEntry::new(
                node,
                &reachable.graph[node].data,
                node == new_start,
                reachable.graph[node].accepting,
            );

            for letter in self.alphabet.iter() {
                let target = reachable
                    .graph
                    .edges_directed(node, Direction::Outgoing)
                    .find(|edge| edge.weight() == letter)
                    .map(|edge| edge.target());

                if target.is_none() {
                    println!("No target for letter {:?} from state {:?}", letter, node);
                    panic!("DFA must be complete to minimize");
                }

                entry.add_transition(target.unwrap());
            }

            table.add_entry(entry);
        }

        table.minimize();

        table.to_dfa()
    }

    /// Inverts self, creating a new DFA where the accepting states are inverted.
    /// The DFA must have a start state and be complete.
    pub fn invert(&self) -> DFA<N, E> {
        assert!(self.start.is_some(), "DFA must have a start state");
        assert!(self.is_complete, "DFA must be complete to invert");

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

    /// Builds an intersection DFA from two DFAs. Both DFAs must have the same alphabet, a start state, and they must be complete.
    pub fn intersect<NO: AutNode>(&self, other: &DFA<NO, E>) -> DFA<(N, NO), E> {
        assert!(self.start.is_some(), "Self must have a start state");
        assert!(other.start.is_some(), "Other must have a start state");

        assert!(self.is_complete, "Self must be complete to intersect");
        assert!(other.is_complete, "Other must be complete to intersect");

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

    pub fn bfs(&self, is_target: impl Fn(NodeIndex<u32>, &DfaNodeData<N>) -> bool) -> Vec<Path> {
        let mut visited = std::collections::HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        let mut paths = Vec::new();

        assert!(self.start.is_some(), "Self must have a start state");
        queue.push_back(Path::new(self.start.unwrap()));

        while let Some(path) = queue.pop_front() {
            let last = path.end;

            if is_target(last, &self.graph[last]) {
                paths.push(path.clone());
            }

            visited.insert(last);

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                if !visited.contains(&edge.target()) {
                    let mut new_path = path.clone();
                    new_path.add_edge(edge.id(), edge.target());
                    queue.push_back(new_path);
                }
            }
        }

        paths
    }

    pub fn bfs_accepting_states(&self) -> Vec<Path> {
        self.bfs(|_, data| data.accepting)
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
    pub fn is_subset_of<NO: AutNode>(&self, other: &DFA<NO, E>) -> bool {
        let inverted = other.clone().invert();
        let intersection = self.intersect(&inverted);
        // dbg!(&intersection);
        // println!("{:?}", Dot::new(&intersection.graph));

        intersection.is_language_empty()
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

    fn alphabet(&self) -> &Vec<E> {
        &self.alphabet
    }
}

impl<N: AutNode, E: AutEdge> Debug for DFA<N, E> {
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

/// Represents the table used in the minimization of a DFA.
/// The table is a vector of entries, where each entry is a state in the DFA.
/// Each entry contains the state, whether it is an initial state, whether it is a final state, and the transitions to other states.
/// The transitions are represented as a vector of states, where the index of the state corresponds to the index of the symbol in the alphabet.
/// The alphabet is a reference to the alphabet of the DFA.
/// The table contains None for states that have been merged with another state, as to minimize reallocations when merging states.
#[derive(Debug, Clone)]
pub struct DfaMinimizationTable<'a, N: AutNode, E: AutEdge> {
    pub table: Vec<Option<DfaMinimizationTableEntry<'a, N>>>,
    pub graph: &'a DFA<N, E>,
}

impl<'a, N: AutNode, E: AutEdge> DfaMinimizationTable<'a, N, E> {
    pub fn new(graph: &'a DFA<N, E>) -> Self {
        DfaMinimizationTable {
            table: vec![],
            graph,
        }
    }

    // Iterate over the table, returning the entries that are not None.
    fn iter_some(&self) -> impl Iterator<Item = &DfaMinimizationTableEntry<'a, N>> {
        self.table.iter().filter_map(|entry| entry.as_ref())
    }

    // Iterate over the table, returning the entries that are not None.
    fn iter_mut_some(&mut self) -> impl Iterator<Item = &mut DfaMinimizationTableEntry<'a, N>> {
        self.table.iter_mut().filter_map(|entry| entry.as_mut())
    }

    /// Add a new entry to the minimization table.
    /// Make sure that the transitions are sorted the same way as the alphabet.
    pub fn add_entry(&mut self, entry: DfaMinimizationTableEntry<'a, N>) {
        self.table.push(Some(entry));
    }

    pub fn minimize(&mut self) {
        for entry in self.iter_some() {
            assert_eq!(
                entry.transitions.len(),
                self.graph.alphabet.len(),
                "All entries must have transitions for all symbols in the alphabet. Entry: {:?}",
                entry
            );
        }

        let equivalent_states = self.find_equivalent_states();

        // dbg!(&equivalent_states);
        // dbg!(&self.table);

        for (i, j) in equivalent_states {
            if self.table[i].is_none() || self.table[j].is_none() {
                continue;
            }

            let entry_b = self.table[j].take().unwrap();
            let entry_a = self.table[i].as_mut().unwrap();

            let state_a = entry_a.state;

            // we need to merge the two entries
            // the new entry will have the state of entry_a, and the initial state and final state will be true if either of the two entries had it set to true
            // since we only merge final with final and non-final with non-final, we can just take the final state of entry_a
            entry_a.is_initial = entry_a.is_initial || entry_b.is_initial;

            // dbg!(&self.table);

            // then we need to change every reference of the state of entry_b to entry_a
            for entry in self.iter_mut_some() {
                for transition in entry.transitions.iter_mut() {
                    if *transition == entry_b.state {
                        *transition = state_a;
                    }
                }
            }
        }

        // dbg!(&self.table);
    }

    pub fn to_dfa(&self) -> DFA<N, E> {
        let mut dfa = DFA::new(self.graph.alphabet.clone());

        let mut state_map = HashMap::new();

        for entry in self.iter_some() {
            let state = dfa.add_state(DfaNodeData::new(entry.is_final, entry.data.clone()));
            if entry.is_initial {
                dfa.set_start(state);
            }

            state_map.insert(entry.state, state);
        }

        for entry in self.iter_some() {
            let from = state_map[&entry.state];

            for (i, symbol) in self.graph.alphabet.iter().enumerate() {
                let target = entry.transitions[i];
                let to = state_map[&target];
                dfa.add_transition(from, to, (*symbol).clone());
            }
        }

        dfa.override_complete();

        dfa
    }

    fn find_equivalent_states(&self) -> Vec<(usize, usize)> {
        let state_count = self.table.len();
        let mut table = vec![vec![false; state_count]; state_count];

        // mark all pairs of states (q1, q2) where q1 is accepting and q2 is not accepting
        for i_data in self.iter_some() {
            for j_data in self.iter_some() {
                let i = i_data.state.index();
                let j = j_data.state.index();

                if i >= j {
                    continue;
                }

                if table[i][j] {
                    continue;
                }

                if i_data.is_final != j_data.is_final {
                    table[i][j] = true;
                }
            }
        }

        // while there is an unmarked pair (q1, q2) in the table and a letter with q1 -> q3 and q2 -> q4 so that (q3, q4) is marked, mark (q1, q2)
        let mut changed = true;
        while changed {
            changed = false;

            for i_data in self.iter_some() {
                for j_data in self.iter_some() {
                    let i = i_data.state.index();
                    let j = j_data.state.index();

                    if i >= j {
                        continue;
                    }

                    if table[i][j] {
                        continue;
                    }

                    for l in 1..self.graph.alphabet.len() {
                        let mut i_target = i_data.transitions[l].index();
                        let mut j_target = j_data.transitions[l].index();

                        if i_target >= j_target {
                            (i_target, j_target) = (j_target, i_target);
                        }

                        if table[i_target][j_target] {
                            table[i][j] = true;
                            changed = true;
                        }
                    }
                }
            }
        }

        // println!("Table:");

        // for i in 0..state_count {
        //     for j in 0..state_count {
        //         if table[i][j] {
        //             print!("x")
        //         }
        //         else {
        //             print!(".")
        //         }
        //     }

        //     println!();
        // }

        // dbg!(&table);

        let mut equivalent_states = vec![];

        for i_data in self.iter_some() {
            for j_data in self.iter_some() {
                let i = i_data.state.index();
                let j = j_data.state.index();

                if i >= j {
                    continue;
                }

                if !table[i][j] {
                    // println!("States {:?} and {:?} are equivalent", i_data.data, j_data.data);
                    equivalent_states.push((i, j));
                }
            }
        }

        equivalent_states
    }
}

#[derive(Debug, Clone)]
pub struct DfaMinimizationTableEntry<'a, N: AutNode> {
    pub state: NodeIndex<u32>,
    pub data: &'a N,
    pub is_initial: bool,
    pub is_final: bool,
    pub transitions: Vec<NodeIndex<u32>>,
}

impl<'a, N: AutNode> DfaMinimizationTableEntry<'a, N> {
    pub fn new(state: NodeIndex<u32>, data: &'a N, initial_state: bool, final_state: bool) -> Self {
        DfaMinimizationTableEntry {
            state,
            data,
            is_initial: initial_state,
            is_final: final_state,
            transitions: vec![],
        }
    }

    pub fn add_transition(&mut self, target: NodeIndex<u32>) {
        self.transitions.push(target);
    }
}
