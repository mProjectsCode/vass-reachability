use std::fmt::Display;

use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};
use transition_sequence::TransitionSequence;

use super::{
    AutBuild, Automaton,
    dfa::{DFA, cfg::VASSCFG},
};
use crate::automaton::dfa::{cfg::CFGCounterUpdate, node::DfaNode};

pub mod parikh_image;
pub mod transition_sequence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path {
    pub transitions: TransitionSequence,
    pub start: NodeIndex<u32>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            transitions: TransitionSequence::new(),
            start: start_index,
        }
    }

    /// Take an edge to a new node
    pub fn add_edge(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.transitions.add(edge, node);
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        self.transitions.has_loop()
    }

    pub fn last(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.transitions.last()
    }

    pub fn end(&self) -> NodeIndex<u32> {
        self.transitions.end().unwrap_or(self.start)
    }

    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    pub fn slice(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice(index),
            start: self.start,
        }
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

    /// Creates a DFA from the path, which disallows a counter to go negative.
    /// This assumes that the path has been cut at the first negative counter.
    pub fn to_negative_cut_dfa(
        &self,
        dimension: usize,
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
            .filter(|update| update.counter() == negative_counter.counter())
            .collect_vec();

        let alphabet = CFGCounterUpdate::alphabet(dimension);

        let mut dfa = VASSCFG::<()>::new(alphabet.clone());

        let mut current = dfa.add_state(DfaNode::new(false, ()));
        dfa.set_start(current);

        for update in counter_updates {
            let new = dfa.add_state(DfaNode::new(false, ()));
            dfa.add_transition(current, new, update);

            for letter in &alphabet {
                if letter.counter() != negative_counter.counter() {
                    dfa.add_transition(current, current, *letter);
                }
            }

            current = new;
        }

        for letter in &alphabet {
            dfa.add_transition(current, current, *letter);
        }

        dfa.graph[current].accepting = true;
        dfa.add_failure_state(());
        dfa.invert_mut();

        dfa
    }

    pub fn to_word<T>(&self, get_edge_weight: impl Fn(EdgeIndex<u32>) -> T) -> Vec<T> {
        self.transitions.to_word(get_edge_weight)
    }

    pub fn is_n_reaching(
        &self,
        initial_valuation: &[i32],
        final_valuation: &[i32],
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> (PathNReaching, Vec<i32>) {
        let mut counters = initial_valuation.to_vec();
        let mut negative_index = None;

        for (i, edge) in self.transitions.iter().enumerate() {
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

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EdgeIndex) -> String) -> String {
        format!(
            "{:?} {}",
            self.start.index(),
            self.transitions.to_fancy_string(get_edge_string)
        )
    }
}

impl Display for Path {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}", self.start.index(), self.transitions)
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
