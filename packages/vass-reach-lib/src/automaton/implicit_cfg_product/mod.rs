use hashbrown::HashSet;
use itertools::Itertools;

use crate::automaton::{
    Alphabet, Automaton, AutomatonEdge, Deterministic, ExplicitEdgeAutomaton, InitializedAutomaton,
    TransitionSystem,
    cfg::{
        update::CFGCounterUpdate,
        vasscfg::{
            VASSCFG, build_bounded_counting_cfg, build_modulo_counting_cfg,
            build_rev_bounded_counting_cfg,
        },
    },
    implicit_cfg_product::state::MultiGraphState,
    path::Path,
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

pub mod state;

type MultiGraphPath = Path<MultiGraphState, CFGCounterUpdate>;

/// An implicit representation of the product of multiple CFGs, where we only
/// store the individual CFGs and compute the product on the fly when needed.
///
/// - The main cfg is stored in the first position of the `cfgs` vector.
/// - Then come `dimension` many modulo counting CFGs, one for each counter.
///   (index `1..dimension+1`)
/// - Then come `dimension` many forward bounded counting CFGs (index
///   `dimension+1...dimension*2+1`)
/// - Then come `dimension` many backward bounded counting CFGs (index
///   `dimension*2+1...dimension*3+1`)
#[derive(Debug)]
pub struct ImplicitCFGProduct {
    pub dimension: usize,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    pub mu: Box<[i32]>,
    pub forward_bound: Box<[u32]>,
    pub backward_bound: Box<[u32]>,
    pub cfgs: Vec<VASSCFG<()>>,
    explicit: Option<VASSCFG<()>>,
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

        let mut cfgs = Vec::with_capacity(dimension * 3 + 1);
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
            explicit: None,
        }
    }

    /// Use with care, only intended for testing. Constructs the product without
    /// the counting CFGs, which means that methods may randomly panic if they
    /// try to access the counting CFGs.
    pub fn new_without_counting_cfgs(
        dimension: usize,
        initial_valuation: VASSCounterValuation,
        final_valuation: VASSCounterValuation,
        cfg: VASSCFG<()>,
    ) -> Self {
        let mu = vec![2; dimension];
        let forward_bound = vec![2; dimension];
        let backward_bound = vec![2; dimension];

        let cfgs = vec![cfg];

        ImplicitCFGProduct {
            dimension,
            initial_valuation,
            final_valuation,
            mu: mu.into_boxed_slice(),
            forward_bound: forward_bound.into_boxed_slice(),
            backward_bound: backward_bound.into_boxed_slice(),
            cfgs,
            explicit: None,
        }
    }

    pub fn main_cfg(&self) -> &VASSCFG<()> {
        &self.cfgs[0]
    }

    pub fn main_cfg_index(&self) -> usize {
        0
    }

    pub fn get_modulo_cfg_index(&self, counter: VASSCounterIndex) -> usize {
        1 + counter.to_usize()
    }

    pub fn get_forward_bound_cfg_index(&self, counter: VASSCounterIndex) -> usize {
        1 + self.dimension + counter.to_usize()
    }

    pub fn get_backward_bound_cfg_index(&self, counter: VASSCounterIndex) -> usize {
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

        tracing::debug!(
            "Adding new CFG with {} nodes and {} edges to the product",
            other.node_count(),
            other.edge_count()
        );

        self.reset_explicit();

        self.cfgs.push(other);
    }

    pub fn reach(&self) -> Option<MultiGraphPath> {
        // For every node, we track which counter valuations we already visited.
        let mut visited = HashSet::<MultiGraphState>::new();
        let mut queue = std::collections::VecDeque::new();

        let start = self.initial();
        if self.is_accepting(&start) {
            return Some(MultiGraphPath::new(start));
        }

        queue.push_back(MultiGraphTraversalState::new(vec![], start.clone()));
        visited.insert(start);

        while let Some(state) = queue.pop_front() {
            for letter in self.alphabet() {
                let target = state.last_state.take_letter(&self.cfgs, letter);
                let Some(target) = target else {
                    continue;
                };
                // Optimization: if any of the graphs is in a trap state, we can stop this
                // branch of the search, because we cannot reach an accepting
                // state from a trap state.
                if self.is_trap(&target) {
                    continue;
                }

                if !visited.contains(&target) {
                    visited.insert(target.clone());

                    let mut word = state.word.clone();
                    word.push(*letter);

                    let new_state = MultiGraphTraversalState::new(word, target);

                    if self.is_accepting(&new_state.last_state) {
                        // paths.push(new_path);
                        // Optimization: we only search for the shortest path, so we can stop when
                        // we find one

                        return Some(new_state.to_path(self));
                    } else {
                        queue.push_back(new_state);
                    }
                }
            }
        }

        None
    }

    pub fn find_scc_surrounding(&self, node: MultiGraphState) -> HashSet<MultiGraphState> {
        let mut stack = vec![];
        let mut current_path = vec![];
        let mut scc = HashSet::new();
        let mut visited = HashSet::new();

        stack.push(node.clone());
        current_path.push(node.clone());
        scc.insert(node);

        while let Some(current) = stack.last().cloned() {
            if !visited.contains(&current) {
                visited.insert(current.clone());
            }

            let mut found_unvisited = false;
            for letter in self.alphabet() {
                let successor = current.take_letter(&self.cfgs, letter);
                let Some(successor) = successor else {
                    continue;
                };

                if !visited.contains(&successor) {
                    stack.push(successor.clone());
                    current_path.push(successor);
                    found_unvisited = true;
                    break;
                } else if scc.contains(&successor) {
                    for node in &current_path {
                        scc.insert(node.clone());
                    }
                }
            }

            if !found_unvisited {
                stack.pop();
                if !current_path.is_empty() && current_path.last() == Some(&current) {
                    current_path.pop();
                }
            }
        }

        scc
    }

    fn is_accepting(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter().enumerate() {
            // we are accepting if all graphs are in an accepting state
            if !cfg.is_accepting(&state[i]) {
                return false;
            }
        }

        true
    }

    fn is_trap(&self, state: &MultiGraphState) -> bool {
        for (i, cfg) in self.iter().enumerate() {
            // we are in a trap if any graph is in a trap state
            if cfg.is_trap(state[i]) {
                return true;
            }
        }

        false
    }

    pub fn initial(&self) -> MultiGraphState {
        let start_states = self
            .iter()
            .map(|cfg| cfg.get_initial())
            .collect_vec()
            .into_boxed_slice();

        MultiGraphState {
            states: start_states,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &VASSCFG<()>> {
        self.cfgs.iter()
    }

    /// If not already done, constructs and returns a ref to the explicit
    /// product CFG.
    pub fn explicit(&mut self) -> &VASSCFG<()> {
        if self.explicit.is_none() {
            // TODO: construct the product in one step
            let mut explicit_cfg = self.cfgs[0].clone();

            for cfg in self.cfgs.iter().skip(1) {
                explicit_cfg = explicit_cfg.intersect(cfg);
            }

            self.explicit = Some(explicit_cfg);
        }

        self.explicit.as_ref().unwrap()
    }

    pub fn reset_explicit(&mut self) {
        self.explicit = None;
    }
}

impl Alphabet for ImplicitCFGProduct {
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[Self::Letter] {
        self.cfgs[0].alphabet()
    }
}

impl Automaton<Deterministic> for ImplicitCFGProduct {
    type NIndex = MultiGraphState;

    type N = ();

    /// We don't actually know the exact number of nodes without constructing
    /// the full product. For now we return the upper bound, the product of
    /// all cfg node counts, but that could be huge.
    fn node_count(&self) -> usize {
        self.cfgs.iter().map(|cfg| cfg.node_count()).product()
    }

    fn get_node(&self, _index: &Self::NIndex) -> Option<&()> {
        Some(&())
    }

    fn get_node_unchecked(&self, _index: &Self::NIndex) -> &() {
        &()
    }
}

impl TransitionSystem<Deterministic> for ImplicitCFGProduct {
    fn successor(&self, node: &Self::NIndex, letter: &Self::Letter) -> Option<Self::NIndex> {
        node.take_letter(&self.cfgs, letter)
    }

    fn successors<'a>(
        &'a self,
        node: &'a Self::NIndex,
    ) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        Box::new(
            self.alphabet()
                .iter()
                .filter_map(move |letter| self.successor(node, letter)),
        )
    }

    fn predecessors(&self, node: &Self::NIndex) -> Box<dyn Iterator<Item = Self::NIndex> + '_> {
        // For each letter in the shared alphabet:
        //  - collect predecessors (sources of incoming edges matching that letter) for
        //    each component
        //  - if every component has ≥1 predecessor, emit the Cartesian product of those
        //    per-component sets
        //
        // This avoids constructing the explicit product; it only touches the relevant
        // incoming edges.
        let mut result = Vec::new();

        for letter in self.alphabet() {
            // gather predecessors for each component under `letter`
            let mut per_comp_preds: Vec<Vec<_>> = Vec::with_capacity(self.cfgs.len());
            let mut empty = false;

            for (i, cfg) in self.cfgs.iter().enumerate() {
                let target = node[i];
                let preds: Vec<_> = cfg
                    .incoming_edge_indices(&target)
                    .filter(|e| cfg.get_edge_unchecked(e).matches(letter))
                    .map(|e| cfg.edge_source_unchecked(&e))
                    .collect();

                if preds.is_empty() {
                    empty = true;
                    break;
                }
                per_comp_preds.push(preds);
            }

            if empty {
                continue; // this letter cannot produce a predecessor product-state
            }

            // produce Cartesian product of per-component predecessor lists
            for combo in per_comp_preds.into_iter().multi_cartesian_product() {
                result.push(MultiGraphState::from(combo.into_boxed_slice()));
            }
        }

        Box::new(result.into_iter())
    }
}

impl InitializedAutomaton<Deterministic> for ImplicitCFGProduct {
    fn get_initial(&self) -> Self::NIndex {
        self.initial()
    }

    fn is_accepting(&self, node: &Self::NIndex) -> bool {
        self.is_accepting(node)
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
    pub word: Vec<CFGCounterUpdate>,
    pub last_state: MultiGraphState,
}

impl MultiGraphTraversalState {
    pub fn new(word: Vec<CFGCounterUpdate>, last_state: MultiGraphState) -> Self {
        Self { word, last_state }
    }

    pub fn to_path(&self, product: &ImplicitCFGProduct) -> MultiGraphPath {
        let mut path = MultiGraphPath::new(product.initial());

        for update in &self.word {
            let next_state = path.end().take_letter(&product.cfgs, update).unwrap();
            path.add(*update, next_state);
        }

        debug_assert_eq!(path.end(), &self.last_state);

        path
    }
}
