use std::{fmt::Display, vec::IntoIter};

use path_like::{EdgeListLike, PathLike};
use petgraph::graph::{EdgeIndex, NodeIndex};
use transition_sequence::TransitionSequence;

use super::{
    AutBuild, Automaton,
    dfa::{
        DFA,
        cfg::{VASSCFG, build_bounded_counting_cfg, build_rev_limited_counting_cfg},
    },
};
use crate::automaton::dfa::{cfg::CFGCounterUpdate, node::DfaNode};

pub mod parikh_image;
pub mod path_like;
pub mod transition_sequence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    transitions: TransitionSequence,
    start: NodeIndex<u32>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            transitions: TransitionSequence::new(),
            start: start_index,
        }
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        self.transitions.has_loop()
    }

    pub fn start(&self) -> NodeIndex<u32> {
        self.start
    }

    pub fn end(&self) -> NodeIndex<u32> {
        self.transitions.end().unwrap_or(self.start)
    }

    /// Whether the path contains a specific node.
    /// This does **not** check the start node.
    pub fn transitions_contain_node(&self, node: NodeIndex<u32>) -> bool {
        self.transitions.contains_node(node)
    }

    pub fn simple_to_dfa(
        &self,
        trap: bool,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> DFA<(), CFGCounterUpdate> {
        let mut dfa = DFA::<(), CFGCounterUpdate>::new(CFGCounterUpdate::alphabet(dimension));

        let mut current = dfa.add_state(DfaNode::new(false, ()));
        dfa.set_start(current);

        for (edge, _) in &self.transitions {
            let new = dfa.add_state(DfaNode::new(false, ()));
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
        dfa.invert_mut();

        dfa
    }

    /// Creates a bounded counting automaton from a path. This assumes the path
    /// has been cut after the first counter went negative.
    pub fn to_bounded_counting_cfg(
        &self,
        dimension: usize,
        initial_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> VASSCFG<()> {
        let negative_counter = self
            .last()
            .map(|(edge, _)| get_edge_weight(*edge))
            .expect("Path must have one edge");

        let counter_updates = self
            .transitions
            .iter()
            .map(|(edge, _)| get_edge_weight(*edge))
            .filter(|update| update.counter() == negative_counter.counter());

        let mut counter = 0;
        let mut max_counter = 0;
        for update in counter_updates {
            counter += update.op();
            max_counter = max_counter.max(counter);
        }

        let start = initial_valuation[negative_counter.counter()];

        // println!("start: {}, max: {}", start, start + max_counter);

        build_bounded_counting_cfg(
            dimension,
            negative_counter,
            (start + max_counter) as u32,
            start as usize,
        )
    }

    /// Creates a reverse bounded counting automaton from a path. This assumes
    /// the path has been cut before the last counter went negative.
    pub fn to_rev_bounded_counting_cfg(
        &self,
        dimension: usize,
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> VASSCFG<()> {
        let negative_counter = self
            .first()
            .map(|(edge, _)| get_edge_weight(*edge))
            .expect("Path must have one edge");

        let counter_updates = self
            .transitions
            .iter()
            .map(|(edge, _)| get_edge_weight(*edge))
            .filter(|update| update.counter() == negative_counter.counter());

        let mut counter = 0;
        let mut max_counter = 0;
        for update in counter_updates {
            counter += update.op();
            max_counter = max_counter.max(counter);
        }

        let end = final_valuation[negative_counter.counter()];

        // println!("start: {}, max: {}", start, start + max_counter);

        build_rev_limited_counting_cfg(
            dimension,
            negative_counter,
            (end + max_counter) as u32,
            end as usize,
        )
    }

    pub fn is_n_reaching(
        &self,
        initial_valuation: &[i32],
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> (PathNReaching, Vec<i32>) {
        let mut counters = initial_valuation.to_vec();
        let mut negative_index = None;

        for (i, edge) in self.iter().enumerate() {
            get_edge_weight(edge.0).apply(&mut counters);

            if negative_index.is_none() && counters.iter().any(|&x| x < 0) {
                negative_index = Some(i);
            }
        }

        if let Some(index) = negative_index {
            (PathNReaching::Negative(index), counters)
        } else {
            (
                PathNReaching::from_bool(counters == final_valuation),
                counters,
            )
        }
    }

    pub fn becomes_negative_reverse(
        &self,
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> Option<usize> {
        let mut counters = final_valuation.to_vec();

        for i in (self.len() - 1)..0 {
            let edge = self.transitions.get_edge(i);
            get_edge_weight(edge).apply_rev(&mut counters);

            if counters.iter().any(|&x| x < 0) {
                return Some(i);
            }
        }

        None
    }

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EdgeIndex) -> String) -> String {
        format!(
            "{:?} {}",
            self.start.index(),
            self.transitions.to_fancy_string(get_edge_string)
        )
    }
}

impl EdgeListLike for Path {
    fn iter_edges(&self) -> impl Iterator<Item = EdgeIndex<u32>> {
        self.transitions.iter_edges()
    }

    fn has_edge(&self, edge: EdgeIndex<u32>) -> bool {
        self.transitions.has_edge(edge)
    }

    fn get_edge_label(&self, edge: EdgeIndex<u32>) -> String {
        edge.index().to_string()
    }
}

impl PathLike for Path {
    fn iter_nodes(&self) -> impl Iterator<Item = NodeIndex<u32>> {
        self.transitions.iter_nodes()
    }

    fn has_node(&self, node: NodeIndex<u32>) -> bool {
        self.start == node || self.transitions.has_node(node)
    }

    fn get_node_label(&self, node: NodeIndex<u32>) -> String {
        node.index().to_string()
    }

    fn iter(&self) -> impl Iterator<Item = &(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.transitions.iter()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (EdgeIndex<u32>, NodeIndex<u32>)> {
        self.transitions.iter_mut()
    }

    fn first(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.transitions.first()
    }

    fn last(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.transitions.last()
    }

    fn split_off(&mut self, index: usize) -> Self {
        Path {
            transitions: self.transitions.split_off(index),
            start: self.start,
        }
    }

    fn slice(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice(index),
            start: self.start,
        }
    }

    fn slice_end(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice_end(index),
            start: self.start,
        }
    }

    fn add_pair(&mut self, edge: (EdgeIndex<u32>, NodeIndex<u32>)) {
        self.transitions.add_pair(edge);
    }

    fn len(&self) -> usize {
        self.transitions.len()
    }

    fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    fn contains_node(&self, node: NodeIndex<u32>) -> bool {
        self.start == node || self.transitions.contains_node(node)
    }

    fn get(&self, index: usize) -> (EdgeIndex<u32>, NodeIndex<u32>) {
        self.transitions.get(index)
    }

    fn get_node(&self, index: usize) -> NodeIndex<u32> {
        self.transitions.get_node(index)
    }

    fn get_edge(&self, index: usize) -> EdgeIndex<u32> {
        self.transitions.get_edge(index)
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}", self.start.index(), self.transitions)
    }
}

impl IntoIterator for Path {
    type Item = (EdgeIndex<u32>, NodeIndex<u32>);
    type IntoIter = IntoIter<(EdgeIndex<u32>, NodeIndex<u32>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.transitions.into_iter()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathNReaching {
    Negative(usize),
    False,
    True,
}

impl PathNReaching {
    pub fn is_true(&self) -> bool {
        matches!(self, PathNReaching::True)
    }

    pub fn is_false(&self) -> bool {
        matches!(self, PathNReaching::False)
    }

    pub fn is_negative(&self) -> bool {
        matches!(self, PathNReaching::Negative(_))
    }

    pub fn from_bool(b: bool) -> Self {
        match b {
            true => PathNReaching::True,
            false => PathNReaching::False,
        }
    }
}
