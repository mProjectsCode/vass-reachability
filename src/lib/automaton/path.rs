use itertools::Itertools;
use petgraph::{graph::EdgeIndex, graph::NodeIndex};

use super::{
    dfa::{DfaNodeData, DFA},
    vass::dimension_to_cfg_alphabet,
    AutBuild, Automaton,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub transitions: Vec<(EdgeIndex<u32>, NodeIndex<u32>)>,
    pub start: NodeIndex<u32>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            transitions: Vec::new(),
            start: start_index,
        }
    }

    /// Take an edge to a new node
    pub fn add_edge(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.transitions.push((edge, node));
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        let mut visited = vec![];

        for edge in &self.transitions {
            if visited.contains(&edge) {
                return true;
            }

            visited.push(edge);
        }

        false
    }

    pub fn end(&self) -> NodeIndex<u32> {
        self.transitions.last().map(|x| x.1).unwrap_or(self.start)
    }

    pub fn slice(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions[..=index].to_vec(),
            start: self.start,
        }
    }

    pub fn simple_to_dfa(
        &self,
        trap: bool,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> DFA<(), i32> {
        let mut dfa = DFA::<(), i32>::new(dimension_to_cfg_alphabet(dimension));

        let mut current = dfa.add_state(DfaNodeData::new(false, ()));
        dfa.set_start(current);

        for edge in &self.transitions {
            let new = dfa.add_state(DfaNodeData::new(false, ()));
            dfa.add_transition(current, new, get_edge_weight(edge.0));
            current = new;
        }

        dfa.graph[current].accepting = true;

        if trap {
            for letter in dfa.alphabet().clone() {
                dfa.add_transition(current, current, letter);
            }
        }

        dfa.add_failure_state(());

        dfa.invert()
    }

    pub fn to_word(&self, get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32) -> Vec<i32> {
        self.transitions
            .iter()
            .map(|&edge| get_edge_weight(edge.0))
            .collect_vec()
    }

    pub fn is_n_reaching(
        &self,
        initial_valuation: &[i32],
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> PathNReaching {
        let mut counters = initial_valuation.to_vec();

        for (i, edge) in self.transitions.iter().enumerate() {
            let weight = get_edge_weight(edge.0);

            if weight > 0 {
                counters[(weight - 1) as usize] += 1;
            } else {
                counters[(-weight - 1) as usize] -= 1;
            }

            if counters.iter().any(|&x| x < 0) {
                return PathNReaching::Negative(i);
            }
        }

        match counters == final_valuation {
            true => PathNReaching::True,
            false => PathNReaching::False,
        }
    }

    pub fn simple_print(&self, get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32) -> String {
        format!(
            "{:?} {}",
            self.start.index(),
            self.transitions
                .iter()
                .map(|x| format!(
                    "--({:?}, {:?})-> {:?}",
                    x.0.index(),
                    get_edge_weight(x.0),
                    x.1.index()
                ))
                .join(" ")
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathNReaching {
    Negative(usize),
    False,
    True,
}
