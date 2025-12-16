use hashbrown::{HashMap, HashSet};
use itertools::Itertools;

use crate::automaton::{
    InitializedAutomaton, Language,
    cfg::{
        update::CFGCounterUpdatable,
        vasscfg::{VASSCFG, build_bounded_counting_cfg, build_rev_bounded_counting_cfg},
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
    pub cfg: VASSCFG<()>,
    /// mu for modulo counting, one per counter
    pub mu: Box<[i32]>,
    pub forward_bound: Box<[BoundedCFGCache]>,
    pub backward_bound: Box<[BoundedCFGCache]>,
    pub other_cfg: Vec<VASSCFG<()>>,
}

impl ImplicitCFGProduct {
    pub fn new(
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        cfg: VASSCFG<()>,
    ) -> Self {
        let mu = vec![2; dimension].into_boxed_slice();
        let forward_bound = BoundedCFGCache::build_initial(
            BoundedCFGDirection::Forward,
            dimension,
            &initial_valuation,
            &final_valuation,
        );
        let backward_bound = BoundedCFGCache::build_initial(
            BoundedCFGDirection::Backward,
            dimension,
            &initial_valuation,
            &final_valuation,
        );
        let other_cfg = vec![];

        ImplicitCFGProduct {
            dimension,
            initial_valuation,
            final_valuation,
            cfg,
            mu,
            forward_bound,
            backward_bound,
            other_cfg,
        }
    }

    pub fn set_mu(&mut self, counter: VASSCounterIndex, mu: i32) {
        assert!(mu > 0);
        self.mu[counter.to_usize()] = mu;
    }

    pub fn increment_mu(&mut self, counter: VASSCounterIndex) {
        self.mu[counter.to_usize()] += 1;
    }

    pub fn get_mu(&self, counter: VASSCounterIndex) -> i32 {
        self.mu[counter.to_usize()]
    }

    pub fn set_forward_bound(&mut self, counter: VASSCounterIndex, bound: u32) {
        self.forward_bound[counter.to_usize()].rebuild(
            bound,
            counter,
            self.dimension,
            self.initial_valuation[counter],
            self.final_valuation[counter],
        )
    }

    pub fn set_backward_bound(&mut self, counter: VASSCounterIndex, bound: u32) {
        self.backward_bound[counter.to_usize()].rebuild(
            bound,
            counter,
            self.dimension,
            self.initial_valuation[counter],
            self.final_valuation[counter],
        )
    }

    pub fn get_forward_bound(&self, counter: VASSCounterIndex) -> u32 {
        self.forward_bound[counter.to_usize()].bound
    }

    pub fn get_forward_bounds(&self) -> Box<[u32]> {
        self.forward_bound.iter().map(|cache| cache.bound).collect()
    }

    pub fn get_backward_bound(&self, counter: VASSCounterIndex) -> u32 {
        self.backward_bound[counter.to_usize()].bound
    }

    pub fn get_backward_bounds(&self) -> Box<[u32]> {
        self.backward_bound
            .iter()
            .map(|cache| cache.bound)
            .collect()
    }

    pub fn add_cfg(&mut self, other: VASSCFG<()>) {
        assert!(
            other.alphabet() == self.cfg.alphabet(),
            "CFGs must have the same alphabet"
        );
        assert!(other.is_complete(), "CFG must be complete");

        self.other_cfg.push(other);
    }

    pub fn reach(&self) -> Option<MultiGraphPath> {
        let graphs = self.iter_all_graphs().collect_vec();

        // For every node, we track which counter valuations we already visited.
        let mut visited = HashMap::<MultiGraphState, HashSet<VASSCounterValuation>>::new();
        let mut queue = std::collections::VecDeque::new();
        let mut mod_initial_valuation: VASSCounterValuation = self.initial_valuation.clone();
        let mut mod_final_valuation: VASSCounterValuation = self.final_valuation.clone();
        mod_initial_valuation.mod_euclid_slice_mut(&self.mu);
        mod_final_valuation.mod_euclid_slice_mut(&self.mu);

        let start = self.get_start_multi_state();
        let initial_path = MultiGraphPath::new();
        if self.multi_state_accepting(&start) && mod_initial_valuation == mod_final_valuation {
            return Some(initial_path);
        }

        queue.push_back(MultiGraphTraversalState::new(
            initial_path,
            start.clone(),
            mod_initial_valuation.clone(),
        ));
        visited
            .entry(start)
            .or_default()
            .insert(mod_initial_valuation);

        while let Some(state) = queue.pop_front() {
            for letter in self.cfg.alphabet() {
                let target = state.last_state.take_letter(&graphs, letter);
                let Some(target) = target else {
                    println!("No target for letter {:?}", letter);
                    continue;
                };
                // Optimization: if any of the graphs is in a trap state, we can stop this
                // branch of the search, because we cannot reach an accepting
                // state from a trap state.
                if self.multi_state_trap(&target) {
                    continue;
                }

                let mut new_valuation = state.mod_valuation.clone();
                new_valuation.apply_cfg_update_mod_slice(*letter, &self.mu);

                let entry = visited.entry(target.clone()).or_default();

                if !entry.contains(&new_valuation) {
                    entry.insert(new_valuation.clone());

                    let mut new_path = state.path.clone();
                    new_path.add(*letter);

                    if self.multi_state_accepting(&target) && new_valuation == mod_final_valuation {
                        // paths.push(new_path);
                        // Optimization: we only search for the shortest path, so we can stop when
                        // we find one
                        return Some(new_path);
                    } else {
                        queue.push_back(MultiGraphTraversalState::new(
                            new_path,
                            target,
                            new_valuation,
                        ));
                    }
                }
            }
        }

        None
    }

    fn multi_state_accepting(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter_all_graphs().enumerate() {
            // we are accepting if all graphs are in an accepting state
            if !cfg.graph[state.states[i]].accepting {
                return false;
            }
        }

        true
    }

    fn multi_state_trap(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter_all_graphs().enumerate() {
            // we are in a trap if any graph is in a trap state
            if cfg.graph[state.states[i]].trap {
                return true;
            }
        }

        false
    }

    fn get_start_multi_state(&self) -> MultiGraphState {
        let start_states = self
            .iter_all_graphs()
            .map(|cfg| cfg.get_initial())
            .collect_vec()
            .into_boxed_slice();

        MultiGraphState {
            states: start_states,
        }
    }

    pub fn iter_all_graphs(&self) -> impl Iterator<Item = &VASSCFG<()>> {
        std::iter::once(&self.cfg)
            .chain(self.forward_bound.iter().map(|cache| &cache.automaton))
            .chain(self.backward_bound.iter().map(|cache| &cache.automaton))
            .chain(self.other_cfg.iter())
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

#[derive(Debug, Clone)]
pub struct BoundedCFGCache {
    pub direction: BoundedCFGDirection,
    pub bound: u32,
    pub automaton: VASSCFG<()>,
}

impl BoundedCFGCache {
    pub fn new(
        direction: BoundedCFGDirection,
        bound: u32,
        counter: VASSCounterIndex,
        dimension: usize,
        initial_valuation: i32,
        final_valuation: i32,
    ) -> Self {
        BoundedCFGCache {
            direction,
            bound,
            automaton: build_counting_automaton(
                direction,
                bound,
                counter,
                dimension,
                initial_valuation,
                final_valuation,
            ),
        }
    }

    pub fn rebuild(
        &mut self,
        bound: u32,
        counter: VASSCounterIndex,
        dimension: usize,
        initial_valuation: i32,
        final_valuation: i32,
    ) {
        self.bound = bound;
        self.automaton = build_counting_automaton(
            self.direction,
            bound,
            counter,
            dimension,
            initial_valuation,
            final_valuation,
        );
    }

    pub fn build_initial(
        direction: BoundedCFGDirection,
        dimension: usize,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> Box<[BoundedCFGCache]> {
        VASSCounterIndex::iter_counters(dimension)
            .map(|i| {
                BoundedCFGCache::new(
                    direction,
                    2,
                    i,
                    dimension,
                    initial_valuation[i],
                    final_valuation[i],
                )
            })
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }
}

pub struct MultiGraphTraversalState {
    pub path: MultiGraphPath,
    pub last_state: MultiGraphState,
    pub mod_valuation: VASSCounterValuation,
}

impl MultiGraphTraversalState {
    pub fn new(
        path: MultiGraphPath,
        last_state: MultiGraphState,
        mod_valuation: VASSCounterValuation,
    ) -> Self {
        Self {
            path,
            last_state,
            mod_valuation,
        }
    }
}
