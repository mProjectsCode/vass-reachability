use itertools::Itertools;
use petgraph::{graph::EdgeIndex, graph::NodeIndex};

use crate::automaton::utils::dyck_transitions_to_ltc_transition;

use super::{
    dfa::{DfaNodeData, DFA},
    ltc::LTC,
    nfa::NFA,
    vass::dimension_to_cfg_alphabet,
    AutBuild, Automaton,
};

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub edges: Vec<EdgeIndex<u32>>,
    pub start: NodeIndex<u32>,
    pub end: Option<NodeIndex<u32>>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            edges: Vec::new(),
            start: start_index,
            end: Some(start_index),
        }
    }

    /// Take an edge to a new node
    pub fn add_edge(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.edges.push(edge);
        self.end = Some(node);
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        let mut visited = vec![];

        for edge in &self.edges {
            if visited.contains(&edge) {
                return true;
            }

            visited.push(edge);
        }

        false
    }

    pub fn slice(&self, index: usize) -> Self {
        Path {
            edges: self.edges[..=index].to_vec(),
            start: self.start,
            end: None,
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

        for edge in &self.edges {
            let new = dfa.add_state(DfaNodeData::new(false, ()));
            dfa.add_transition(current, new, get_edge_weight(*edge));
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
        self.edges
            .iter()
            .map(|&edge| get_edge_weight(edge))
            .collect_vec()
    }

    pub fn to_ltc(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> (LTC, DFA<(), i32>) {
        let mut stack = vec![];
        let mut ltc_translation = vec![];

        for edge in &self.edges {
            let existing_pos = stack.iter().position(|x| x == edge);

            if let Some(pos) = existing_pos {
                let ltc_slice = stack.split_off(pos);
                // only push the transition if it's not empty
                if !stack.is_empty() {
                    ltc_translation.push(LTCTranslation::Path(stack));
                }
                // only push the loop if the last element is not the same
                // that just means we ran the last loop again
                let _loop = LTCTranslation::Loop(ltc_slice);
                if ltc_translation.last() != Some(&_loop) {
                    ltc_translation.push(_loop);
                }
                stack = vec![*edge];
            } else {
                stack.push(*edge);
            }
        }

        if !stack.is_empty() {
            ltc_translation.push(LTCTranslation::Path(stack));
        }

        // dbg!(&ltc_translation);

        let mut ltc = LTC::new(dimension);

        for translation in &ltc_translation {
            match translation {
                LTCTranslation::Path(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_transition(min_counters, counters);
                }
                LTCTranslation::Loop(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_loop(min_counters, counters);
                }
            }
        }

        let mut nfa = NFA::<(), i32>::new(dimension_to_cfg_alphabet(dimension));

        let start = nfa.add_state(DfaNodeData::new(false, ()));
        nfa.set_start(start);
        let mut current_end = start;

        for translation in &ltc_translation {
            match translation {
                LTCTranslation::Path(edges) => {
                    let mut current = nfa.add_state(DfaNodeData::new(false, ()));
                    nfa.add_transition(current_end, current, None);

                    for edge in edges {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current, new, Some(get_edge_weight(*edge)));
                        current = new;
                    }

                    current_end = current;
                }
                LTCTranslation::Loop(edges) => {
                    let loop_start = nfa.add_state(DfaNodeData::new(false, ()));
                    let mut current = loop_start;
                    nfa.add_transition(current_end, current, None);

                    for edge in edges.iter().take(edges.len() - 1) {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current, new, Some(get_edge_weight(*edge)));
                        current = new;
                    }

                    let last_edge = edges.last().unwrap();
                    nfa.add_transition(current, loop_start, Some(get_edge_weight(*last_edge)));

                    current = loop_start;

                    current_end = current;
                }
            }
        }

        nfa.graph[current_end].accepting = true;

        // dbg!(&nfa);

        let mut dfa = nfa.determinize_no_state_data();
        dfa.add_failure_state(());
        dfa = dfa.invert();

        // dbg!(&dfa);

        (ltc, dfa)
    }

    pub fn is_n_reaching(
        &self,
        initial_valuation: &[i32],
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> PathNReaching {
        let mut counters = initial_valuation.to_vec();

        for (i, edge) in self.edges.iter().enumerate() {
            let weight = get_edge_weight(*edge);

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
        self.edges
            .iter()
            .map(|x| format!("({:?}, {:?})", x.index(), get_edge_weight(*x)))
            .join(" -> ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LTCTranslation {
    Path(Vec<EdgeIndex<u32>>),
    Loop(Vec<EdgeIndex<u32>>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathNReaching {
    Negative(usize),
    False,
    True,
}
