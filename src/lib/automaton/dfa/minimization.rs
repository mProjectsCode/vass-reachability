use hashbrown::HashMap;
use petgraph::graph::NodeIndex;

use crate::automaton::{
    AutBuild, AutomatonEdge, AutomatonNode,
    dfa::{DFA, node::DfaNode},
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
}

impl<'a, N: AutomatonNode, E: AutomatonEdge> DfaMinimizationTable<'a, N, E> {
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

        let mut state_map = HashMap::new();

        for entry in self.iter_some() {
            let state = dfa.add_state(DfaNode::new(entry.is_final, entry.data.clone()));
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
pub struct DfaMinimizationTableEntry<'a, N: AutomatonNode> {
    pub state: NodeIndex<u32>,
    pub data: &'a N,
    pub is_initial: bool,
    pub is_final: bool,
    pub transitions: Vec<NodeIndex<u32>>,
}

impl<'a, N: AutomatonNode> DfaMinimizationTableEntry<'a, N> {
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
