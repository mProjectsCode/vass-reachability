use std::cell::{Ref, RefCell};

use hashbrown::HashSet;
use itertools::Itertools;
use petgraph::graph::NodeIndex;

use crate::automaton::{
    Alphabet, Automaton, Deterministic, InitializedAutomaton, TransitionSystem,
    cfg::{
        update::CFGCounterUpdate,
        vasscfg::{
            VASSCFG, build_bounded_counting_cfg, build_modulo_counting_cfg,
            build_rev_bounded_counting_cfg,
        },
    },
    implicit_cfg_product::{path::MultiGraphPath, state::MultiGraphState},
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

pub mod path;
pub mod state;

#[derive(Debug, Clone)]
pub struct ImplicitCFGProduct {
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    pub mu: Box<[i32]>,
    pub forward_bound: Box<[u32]>,
    pub backward_bound: Box<[u32]>,
    pub cfgs: Vec<VASSCFG<()>>,
    explicit: RefCell<Option<VASSCFG<()>>>,
}

impl ImplicitCFGProduct {
    pub fn new(
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        cfg: VASSCFG<()>,
    ) -> Self {
        let mu = vec![2; dimension];
        let forward_bound = vec![2; dimension];
        let backward_bound = vec![2; dimension];

        let mut cfgs = Vec::new();
        cfgs.reserve(dimension * 3 + 1);
        cfgs.push(cfg);

        for i in 0..dimension {
            cfgs.push(build_modulo_counting_cfg(
                dimension,
                VASSCounterIndex::new(i as u32),
                mu[i],
                initial_valuation[i],
                final_valuation[i],
            ));
        }

        for i in 0..dimension {
            cfgs.push(build_counting_automaton(
                BoundedCFGDirection::Forward,
                forward_bound[i],
                VASSCounterIndex::new(i as u32),
                dimension,
                initial_valuation[i],
                final_valuation[i],
            ));
        }
        for i in 0..dimension {
            cfgs.push(build_counting_automaton(
                BoundedCFGDirection::Backward,
                backward_bound[i],
                VASSCounterIndex::new(i as u32),
                dimension,
                initial_valuation[i],
                final_valuation[i],
            ));
        }

        ImplicitCFGProduct {
            dimension,
            initial_valuation,
            final_valuation,
            mu: mu.into_boxed_slice(),
            forward_bound: forward_bound.into_boxed_slice(),
            backward_bound: backward_bound.into_boxed_slice(),
            cfgs,
            explicit: RefCell::new(None),
        }
    }

    pub fn main_cfg(&self) -> &VASSCFG<()> {
        &self.cfgs[0]
    }

    fn get_modulo_cfg_index(&self, counter: VASSCounterIndex) -> usize {
        1 + counter.to_usize()
    }

    fn get_forward_bound_cfg_index(&self, counter: VASSCounterIndex) -> usize {
        1 + self.dimension + counter.to_usize()
    }

    fn get_backward_bound_cfg_index(&self, counter: VASSCounterIndex) -> usize {
        1 + self.dimension * 2 + counter.to_usize()
    }

    pub fn set_mu(&mut self, counter: VASSCounterIndex, mu: i32) {
        assert!(mu > 0);

        self.reset_explicit();

        self.mu[counter.to_usize()] = mu;
        let index = self.get_modulo_cfg_index(counter);
        self.cfgs[index] = build_modulo_counting_cfg(
            self.dimension,
            counter,
            mu,
            self.initial_valuation[counter],
            self.final_valuation[counter],
        );
    }

    pub fn increment_mu(&mut self, counter: VASSCounterIndex) {
        let new_mu = self.get_mu(counter) + 1;
        self.set_mu(counter, new_mu);
    }

    pub fn get_mu(&self, counter: VASSCounterIndex) -> i32 {
        self.mu[counter.to_usize()]
    }

    pub fn set_forward_bound(&mut self, counter: VASSCounterIndex, bound: u32) {
        self.forward_bound[counter.to_usize()] = bound;

        self.reset_explicit();

        let index = self.get_forward_bound_cfg_index(counter);
        self.cfgs[index] = build_counting_automaton(
            BoundedCFGDirection::Forward,
            bound,
            counter,
            self.dimension,
            self.initial_valuation[counter],
            self.final_valuation[counter],
        );
    }

    pub fn set_backward_bound(&mut self, counter: VASSCounterIndex, bound: u32) {
        self.backward_bound[counter.to_usize()] = bound;

        self.reset_explicit();

        let index = self.get_backward_bound_cfg_index(counter);
        self.cfgs[index] = build_counting_automaton(
            BoundedCFGDirection::Backward,
            bound,
            counter,
            self.dimension,
            self.initial_valuation[counter],
            self.final_valuation[counter],
        );
    }

    pub fn get_forward_bound(&self, counter: VASSCounterIndex) -> u32 {
        self.forward_bound[counter.to_usize()]
    }

    pub fn get_forward_bounds(&self) -> Box<[u32]> {
        self.forward_bound.clone()
    }

    pub fn get_backward_bound(&self, counter: VASSCounterIndex) -> u32 {
        self.backward_bound[counter.to_usize()]
    }

    pub fn get_backward_bounds(&self) -> Box<[u32]> {
        self.backward_bound.clone()
    }

    pub fn add_cfg(&mut self, other: VASSCFG<()>) {
        assert!(
            other.alphabet() == self.cfgs[0].alphabet(),
            "CFGs must have the same alphabet"
        );
        assert!(other.is_complete(), "CFG must be complete");

        self.reset_explicit();

        self.cfgs.push(other);
    }

    pub fn reach(&self) -> Option<MultiGraphPath> {
        // For every node, we track which counter valuations we already visited.
        let mut visited = HashSet::<MultiGraphState>::new();
        let mut queue = std::collections::VecDeque::new();

        let start = self.get_start_multi_state();
        let initial_path = MultiGraphPath::new();
        if self.multi_state_accepting(&start) {
            return Some(initial_path);
        }

        queue.push_back(MultiGraphTraversalState::new(initial_path, start.clone()));
        visited.insert(start);

        while let Some(state) = queue.pop_front() {
            for letter in self.cfgs[0].alphabet() {
                let target = state.last_state.take_letter(&self.cfgs, letter);
                let Some(target) = target else {
                    continue;
                };
                // Optimization: if any of the graphs is in a trap state, we can stop this
                // branch of the search, because we cannot reach an accepting
                // state from a trap state.
                if self.multi_state_trap(&target) {
                    continue;
                }

                if !visited.contains(&target) {
                    visited.insert(target.clone());

                    let mut new_path = state.path.clone();
                    new_path.add(*letter);

                    if self.multi_state_accepting(&target) {
                        // paths.push(new_path);
                        // Optimization: we only search for the shortest path, so we can stop when
                        // we find one
                        return Some(new_path);
                    } else {
                        queue.push_back(MultiGraphTraversalState::new(new_path, target));
                    }
                }
            }
        }

        None
    }

    fn multi_state_accepting(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter().enumerate() {
            // we are accepting if all graphs are in an accepting state
            if !cfg.is_accepting(state[i]) {
                return false;
            }
        }

        true
    }

    fn multi_state_trap(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter().enumerate() {
            // we are in a trap if any graph is in a trap state
            if cfg.is_trap(state[i]) {
                return true;
            }
        }

        false
    }

    fn get_start_multi_state(&self) -> MultiGraphState {
        let start_states = self
            .iter()
            .map(|cfg| cfg.get_initial())
            .collect_vec()
            .into_boxed_slice();

        MultiGraphState {
            states: start_states,
        }
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a VASSCFG<()>> {
        self.cfgs.iter()
    }

    /// If not already done, constructs and returns a ref to the explicit
    /// product CFG.
    pub fn explicit(&self) -> Ref<'_, VASSCFG<()>> {
        if self.explicit.borrow().is_none() {
            let mut explicit_cfg = self.cfgs[0].clone();

            for cfg in self.cfgs.iter().skip(1) {
                explicit_cfg = explicit_cfg.intersect(cfg);
            }

            *self.explicit.borrow_mut() = Some(explicit_cfg);
        }

        Ref::map(self.explicit.borrow(), |opt| opt.as_ref().unwrap())
    }

    pub fn reset_explicit(&self) {
        *self.explicit.borrow_mut() = None;
    }
}

impl Alphabet for ImplicitCFGProduct {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[Self::Letter] {
        self.cfgs[0].alphabet()
    }
}

impl Automaton<Deterministic> for ImplicitCFGProduct {
    type NIndex = NodeIndex;

    type N = ();

    /// We don't actually know the exact number of nodes without constructing
    /// the full product. For now we return the upper bound, the product of
    /// all cfg node counts, but that could be huge.
    fn node_count(&self) -> usize {
        self.cfgs.iter().map(|cfg| cfg.node_count()).product()
    }

    fn get_node(&self, _index: Self::NIndex) -> Option<&()> {
        Some(&())
    }

    fn get_node_unchecked(&self, _index: Self::NIndex) -> &() {
        &()
    }
}

impl TransitionSystem<Deterministic> for ImplicitCFGProduct {
    fn successor(&self, _node: Self::NIndex, _letter: &Self::Letter) -> Option<Self::NIndex> {
        todo!()
    }

    fn successors(&self, _node: Self::NIndex) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        todo!()
    }

    fn predecessors(&self, _node: Self::NIndex) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        todo!()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum BoundedCFGDirection {
    Forward,
    Backward,
}

fn build_counting_automaton(
    direction: BoundedCFGDirection,
    bound: u32,
    counter: VASSCounterIndex,
    dimension: usize,
    initial_valuation: i32,
    final_valuation: i32,
) -> VASSCFG<()> {
    let min_bound = bound
        .max(initial_valuation.unsigned_abs())
        .max(final_valuation.unsigned_abs());

    match direction {
        BoundedCFGDirection::Forward => build_bounded_counting_cfg(
            dimension,
            counter,
            min_bound,
            initial_valuation,
            final_valuation,
        ),
        BoundedCFGDirection::Backward => build_rev_bounded_counting_cfg(
            dimension,
            counter,
            min_bound,
            initial_valuation,
            final_valuation,
        ),
    }
}

pub struct MultiGraphTraversalState {
    pub path: MultiGraphPath,
    pub last_state: MultiGraphState,
}

impl MultiGraphTraversalState {
    pub fn new(path: MultiGraphPath, last_state: MultiGraphState) -> Self {
        Self { path, last_state }
    }
}
