use itertools::Itertools;
use petgraph::{graph::EdgeIndex, graph::NodeIndex};

use crate::automaton::utils::dyck_transitions_to_ltc_transition;

use super::ltc::LTC;

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub edges: Vec<EdgeIndex<u32>>,
    pub start: NodeIndex<u32>,
    pub end: NodeIndex<u32>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            edges: Vec::new(),
            start: start_index,
            end: start_index,
        }
    }

    /// Take an edge to a new node
    pub fn add_edge(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.edges.push(edge);
        self.end = node;
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

    pub fn to_ltc(&self, dimension: usize, get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32) -> LTC {
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

        // dbg!(&ltc_translation);s

        let mut ltc = LTC::new(dimension);

        for translation in ltc_translation {
            match translation {
                LTCTranslation::Path(edges) => {
                    let edge_weights = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_transition(min_counters, counters);
                }
                LTCTranslation::Loop(edges) => {
                    let edge_weights = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_loop(min_counters, counters);
                }
            }
        }

        ltc
    }

    pub fn is_zero_reaching(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> bool {
        let mut counters = vec![0; dimension];

        for edge in &self.edges {
            let weight = get_edge_weight(*edge);

            if weight > 0 {
                counters[(weight - 1) as usize] += 1;
            } else {
                counters[(-weight - 1) as usize] -= 1;
            }

            if counters.iter().any(|&x| x < 0) {
                return false;
            }
        }

        counters.iter().all(|&x| x == 0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LTCTranslation {
    Path(Vec<EdgeIndex<u32>>),
    Loop(Vec<EdgeIndex<u32>>),
}
