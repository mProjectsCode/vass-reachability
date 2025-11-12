use petgraph::{Direction, graph::NodeIndex, visit::EdgeRef};

use crate::automaton::{
    AutBuild, AutomatonEdge, AutomatonNode,
    dfa::{DFA, node::DfaNode},
    index_map::{IndexMap, IndexSet},
    path::Path,
};

/// Represents the table used in the minimization of a DFA.
/// The table is a vector of entries, where each entry is a state in the DFA.
/// Each entry contains the state, whether it is an initial state, whether it is
/// a final state, and the transitions to other states. The transitions are
/// represented as a vector of states, where the index of the state corresponds
/// to the index of the symbol in the alphabet. The alphabet is a reference to
/// the alphabet of the DFA. The table contains None for states that have been
/// merged with another state, as to minimize reallocations when merging states.
#[derive(Debug, Clone)]
pub struct DfaMinimizationTable<'a, N: AutomatonNode, E: AutomatonEdge> {
    pub table: Vec<Option<DfaMinimizationTableEntry<'a, N>>>,
    pub graph: &'a DFA<N, E>,
    pub highest_state_index: usize,
}

impl<'a, N: AutomatonNode, E: AutomatonEdge> DfaMinimizationTable<'a, N, E> {
    pub fn new(graph: &'a DFA<N, E>) -> Self {
        let highest_state_index = graph
            .graph
            .node_indices()
            .map(|n| n.index())
            .max()
            .unwrap_or(0);

        DfaMinimizationTable {
            table: vec![],
            graph,
            highest_state_index,
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
            // the new entry will have the state of entry_a, and the initial state and final
            // state will be true if either of the two entries had it set to true
            // since we only merge final with final and non-final with non-final, we can
            // just take the final state of entry_a
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

        let mut state_map = IndexMap::new(self.graph.state_count());

        for entry in self.iter_some() {
            let state = dfa.add_state(DfaNode::new(entry.is_final, false, entry.data.clone()));
            if entry.is_initial {
                dfa.set_start(state);
            }

            state_map.insert(entry.state, state);
        }

        // now we add the transitions
        // if a node has only self-loops, we mark it as a trap node
        for entry in self.iter_some() {
            let from = state_map[entry.state];
            let mut trap = true;

            for (i, symbol) in self.graph.alphabet.iter().enumerate() {
                let target = entry.transitions[i];
                let to = state_map[target];
                dfa.add_transition(from, to, (*symbol).clone());
                if from != to {
                    trap = false;
                }
            }

            if trap {
                dfa.graph[from].trap = true;
            }
        }

        dfa.override_complete();

        dfa
    }

    fn find_equivalent_states(&self) -> Vec<(usize, usize)> {
        let mut table =
            vec![vec![false; self.highest_state_index + 1]; self.highest_state_index + 1];

        // mark all pairs of states (q1, q2) where q1 is accepting and q2 is not
        // accepting
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

        // while there is an unmarked pair (q1, q2) in the table and a letter with q1 ->
        // q3 and q2 -> q4 so that (q3, q4) is marked, mark (q1, q2)
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

                    for l in 0..self.graph.alphabet.len() {
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

        for (i_entry_index, i_data) in self.iter_some().enumerate() {
            for (j_entry_index, j_data) in self.iter_some().enumerate() {
                let i = i_data.state.index();
                let j = j_data.state.index();

                if i >= j {
                    continue;
                }

                if !table[i][j] {
                    // println!("States {:?} and {:?} are equivalent", i_data.data, j_data.data);
                    equivalent_states.push((i_entry_index, j_entry_index));
                }
            }
        }

        equivalent_states
    }
}

#[derive(Debug, Clone)]
pub struct DfaMinimizationTableEntry<'a, N: AutomatonNode> {
    pub state: NodeIndex<u32>,
    pub data: &'a N,
    pub is_initial: bool,
    pub is_final: bool,
    pub transitions: Vec<NodeIndex<u32>>,
}

impl<'a, N: AutomatonNode> DfaMinimizationTableEntry<'a, N> {
    pub fn new(state: NodeIndex<u32>, data: &'a N, is_initial: bool, is_final: bool) -> Self {
        DfaMinimizationTableEntry {
            state,
            data,
            is_initial,
            is_final,
            transitions: vec![],
        }
    }

    pub fn add_transition(&mut self, target: NodeIndex<u32>) {
        self.transitions.push(target);
    }
}

pub trait Minimizable {
    fn minimize(&self) -> Self;
}

impl<N: AutomatonNode, E: AutomatonEdge> Minimizable for DFA<N, E> {
    fn minimize(&self) -> Self {
        assert!(self.start.is_some(), "Self must have a start state");
        assert!(self.is_complete(), "Self must be complete to minimize");

        let mut table = DfaMinimizationTable::new(self);

        let start = self.start.unwrap();
        let mut visited = IndexSet::new(self.state_count());
        let mut stack = vec![start];
        visited.insert(start);

        while let Some(node) = stack.pop() {
            let mut entry = DfaMinimizationTableEntry::new(
                node,
                &self.graph[node].data,
                node == start,
                self.graph[node].accepting,
            );

            for letter in self.alphabet.iter() {
                let target = self
                    .graph
                    .edges_directed(node, Direction::Outgoing)
                    .find(|edge| edge.weight() == letter)
                    .map(|edge| edge.target());

                if target.is_none() {
                    println!("No target for letter {:?} from state {:?}", letter, node);
                    println!("{}", self.to_graphviz(None as Option<Path>));
                    panic!("DFA must be complete to minimize");
                }

                let target = target.unwrap();

                entry.add_transition(target);

                if visited.insert(target) {
                    stack.push(target);
                }
            }

            table.add_entry(entry);
        }

        table.minimize();

        table.to_dfa()
    }
}
