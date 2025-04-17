use std::{
    fmt::{Debug, Display},
    num::NonZeroI32,
    ops::Neg,
};

use petgraph::{Direction, prelude::EdgeRef};

use super::node::DfaNode;
use crate::automaton::{
    AutBuild, AutomatonNode,
    dfa::DFA,
    path::{Path, path_like::PathLike},
    utils::VASSValuation,
};

/// A counter update in a CFG.
///
/// This is encoded as a non zero i32.
/// The counter index is the absolute value of the integer minus 1.
/// The increment or decrement value is the sign of the integer.
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CFGCounterUpdate(pub NonZeroI32);

impl CFGCounterUpdate {
    pub fn new(weight: i32) -> Option<Self> {
        NonZeroI32::new(weight).map(CFGCounterUpdate)
    }

    pub fn to_positive(&self) -> Self {
        CFGCounterUpdate(self.0.abs())
    }

    pub fn to_negative(&self) -> Self {
        CFGCounterUpdate(self.0.abs().neg())
    }

    /// Constructs an alphabet of counter updates for a CFG with `counter_count`
    /// counters. Meaning all counter updates from `1` to `counter_count`
    /// and `-1` to `-counter_count`.
    pub fn alphabet(counter_count: usize) -> Vec<CFGCounterUpdate> {
        let counter_count = counter_count as i32;
        (1..=counter_count)
            .chain((1..=counter_count).map(|x| -x))
            .map(|i| CFGCounterUpdate::new(i).unwrap())
            .collect()
    }

    /// Returns the counter index.
    pub fn counter(&self) -> usize {
        (self.0.get().abs() - 1) as usize
    }

    pub fn abs(&self) -> u32 {
        self.0.get().unsigned_abs()
    }

    /// Returns the increment or decrement value of the counter update.
    pub fn op(&self) -> i32 {
        self.0.get().signum()
    }

    pub fn op_i64(&self) -> i64 {
        self.0.get().signum() as i64
    }

    pub fn apply(&self, counters: &mut [i32]) {
        counters[self.counter()] += self.op();
    }

    pub fn apply_n(&self, counters: &mut [i32], times: i32) {
        counters[self.counter()] += self.op() * times;
    }

    pub fn apply_mod(&self, counters: &mut [i32], modulo: i32) {
        counters[self.counter()] = (counters[self.counter()] + self.op()).rem_euclid(modulo);
    }

    pub fn apply_rev(&self, counters: &mut [i32]) {
        counters[self.counter()] -= self.op();
    }
}

impl From<CFGCounterUpdate> for NonZeroI32 {
    fn from(x: CFGCounterUpdate) -> Self {
        x.0
    }
}

impl From<CFGCounterUpdate> for i32 {
    fn from(x: CFGCounterUpdate) -> Self {
        x.0.get()
    }
}

impl From<NonZeroI32> for CFGCounterUpdate {
    fn from(x: NonZeroI32) -> Self {
        CFGCounterUpdate(x)
    }
}

impl TryFrom<i32> for CFGCounterUpdate {
    type Error = ();

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        CFGCounterUpdate::new(value).ok_or(())
    }
}

impl Display for CFGCounterUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl Debug for CFGCounterUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub type VASSCFG<N> = DFA<N, CFGCounterUpdate>;

impl<N: AutomatonNode> VASSCFG<N> {
    /// Find a reaching paths though the CFG while only counting the counters
    /// modulo `mu`. If a path is found, it is the shortest possible
    /// reaching path with the given modulo.
    ///
    /// Since the number of possible counter valuations is finite, this function
    /// is guaranteed to terminate.
    pub fn modulo_reach(
        &self,
        mu: u32,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> Option<Path> {
        // For every node, we track which counter valuations we already visited.
        let mut visited = vec![std::collections::HashSet::new(); self.state_count()];
        let mut queue = std::collections::VecDeque::new();
        let mut mod_initial_valuation: Box<[i32]> = Box::from(initial_valuation);
        let mut mod_final_valuation: Box<[i32]> = Box::from(final_valuation);
        mod_initial_valuation.mod_euclid_mut(mu);
        mod_final_valuation.mod_euclid_mut(mu);

        let start = self.start.expect("CFG should have a start node");
        let initial_path = Path::new(start);
        if self.graph[start].accepting && mod_initial_valuation == mod_final_valuation {
            return Some(initial_path);
        }

        queue.push_back((initial_path, mod_initial_valuation));

        while let Some((path, valuation)) = queue.pop_front() {
            let last = path.end();

            for edge in self.graph.edges_directed(last, Direction::Outgoing) {
                let mut new_valuation: Box<[i32]> = valuation.clone();

                let update = edge.weight();
                update.apply_mod(&mut new_valuation, mu as i32);

                let target = edge.target();

                if visited[target.index()].insert(new_valuation.clone()) {
                    let mut new_path = path.clone();
                    new_path.add(edge.id(), target);

                    if self.graph[target].accepting && new_valuation == mod_final_valuation {
                        // paths.push(new_path);
                        // Optimization: we only search for the shortest path, so we can stop when
                        // we find one
                        return Some(new_path);
                    } else {
                        queue.push_back((new_path, new_valuation));
                    }
                }
            }
        }

        None
    }
}

pub fn build_bounded_counting_cfg(
    dimension: usize,
    counter: CFGCounterUpdate,
    limit: u32,
    start: usize,
) -> VASSCFG<()> {
    // if limit == 0 {
    //     panic!("Limit must be greater than 0");
    // }

    let counter_up = counter.to_positive();
    let counter_down = counter.to_negative();

    let mut cfg = VASSCFG::new(CFGCounterUpdate::alphabet(dimension));

    let negative = cfg.add_state(DfaNode::new(false, ()));
    let overflow = cfg.add_state(DfaNode::new(true, ()));

    // once negative always stays negative
    for c in CFGCounterUpdate::alphabet(dimension) {
        cfg.add_transition(negative, negative, c);
        cfg.add_transition(overflow, overflow, c);
    }

    let mut states = vec![negative];
    states.extend((0..=limit).map(|_| cfg.add_state(DfaNode::new(true, ()))));
    states.push(overflow);

    for i in 1..states.len() - 1 {
        let prev = states[i - 1];
        let current = states[i];
        let next = states[i + 1];
        cfg.add_transition(current, prev, counter_down);
        cfg.add_transition(current, next, counter_up);

        for c in CFGCounterUpdate::alphabet(dimension) {
            if c != counter_up && c != counter_down {
                cfg.add_transition(current, current, c);
            }
        }

        if i == start + 1 {
            cfg.set_start(current);
        }
    }

    // cfg.assert_complete();
    cfg.override_complete();

    // println!("{}", cfg.to_graphviz(None as Option<Path>));

    cfg
}

pub fn build_rev_limited_counting_cfg(
    dimension: usize,
    counter: CFGCounterUpdate,
    limit: u32,
    end: usize,
) -> VASSCFG<()> {
    let cfg = build_bounded_counting_cfg(dimension, counter, limit, end);

    cfg.reverse()
}
