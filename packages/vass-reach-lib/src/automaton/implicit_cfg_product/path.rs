use hashbrown::HashMap;
use petgraph::graph::NodeIndex;

use crate::automaton::{
    TransitionSystem,
    cfg::{
        CFG,
        update::{CFGCounterUpdatable, CFGCounterUpdate},
    },
    implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
    path::Path,
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

/// A path in the product of multiple graphs, storing the sequence of updates
/// and states.
///
/// The states are stored as `MultiGraphState`, which contains the individual
/// states in each graph of the product.
#[derive(Debug, Clone)]
pub struct MultiGraphPath {
    pub updates: Vec<CFGCounterUpdate>,
    /// INVARIANT: There are always updates.len() + 1 states in the path.
    pub states: Vec<MultiGraphState>,
}

impl MultiGraphPath {
    pub fn new(start: MultiGraphState) -> Self {
        MultiGraphPath {
            updates: vec![],
            states: vec![start],
        }
    }

    pub fn from_word(
        start: MultiGraphState,
        word: impl IntoIterator<Item = CFGCounterUpdate>,
        product: &ImplicitCFGProduct,
    ) -> Self {
        let mut path = MultiGraphPath::new(start);

        for update in word {
            let target = product.successor(path.end(), &update).unwrap_or_else(|| {
                panic!("Invalid update {:?} from state {:?}", update, path.end())
            });
            path.add(update, target);
        }

        path
    }

    pub fn start(&self) -> &MultiGraphState {
        &self.states[0]
    }

    pub fn end(&self) -> &MultiGraphState {
        self.states.last().unwrap()
    }

    /// Turns this MultiGraphPath into a Path in the specified cfg.
    /// The cfg_index specifies which cfg in the product to extract the path
    /// for.
    pub fn to_path_in_cfg<C: CFG<NIndex = NodeIndex>>(
        &self,
        cfg: &C,
        cfg_index: usize,
    ) -> Path<C::NIndex, CFGCounterUpdate> {
        // TODO: cfg not needed, we can read initial state from this path
        let mut path = Path::new(self.start().cfg_state(cfg_index));

        for (update, state) in self.iter_updates_and_state() {
            path.add(*update, state.cfg_state(cfg_index));
        }

        path
    }

    pub fn len(&self) -> usize {
        debug_assert!(self.states.len() == self.updates.len() + 1);

        self.updates.len()
    }

    /// The number of states in the path, which is always one more than the
    /// number of updates.
    pub fn state_len(&self) -> usize {
        debug_assert!(self.states.len() == self.updates.len() + 1);

        self.states.len()
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    pub fn add(&mut self, letter: CFGCounterUpdate, state: MultiGraphState) {
        self.updates.push(letter);
        self.states.push(state);
    }

    pub fn concat(&mut self, mut other: Self) {
        debug_assert!(self.end() == other.start());
        debug_assert!(self.states.len() == self.updates.len() + 1);
        debug_assert!(other.states.len() == other.updates.len() + 1);

        self.updates.append(&mut other.updates);
        self.states.append(&mut other.states[1..].to_vec());

        debug_assert!(self.states.len() == self.updates.len() + 1);
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator<Item = CFGCounterUpdate> + ExactSizeIterator + '_ {
        self.updates.iter().copied()
    }

    pub fn iter_updates_and_state(
        &self,
    ) -> impl DoubleEndedIterator<Item = (&CFGCounterUpdate, &MultiGraphState)> + ExactSizeIterator
    {
        self.updates.iter().zip(self.states.iter().skip(1))
    }

    pub fn iter_states(
        &self,
    ) -> impl DoubleEndedIterator<Item = &MultiGraphState> + ExactSizeIterator {
        self.states.iter()
    }

    pub fn contains_state(&self, node: &MultiGraphState) -> bool {
        self.states.contains(node)
    }

    /// Checks if a path is N-reaching.
    pub fn is_n_reaching(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> bool {
        let mut counters = initial_valuation.clone();

        for edge in self.iter() {
            counters.apply_cfg_update(edge);

            if counters.has_negative_counter() {
                return false;
            }
        }

        &counters == final_valuation
    }

    pub fn get_path_final_valuation(
        &self,
        initial_valuation: &VASSCounterValuation,
    ) -> VASSCounterValuation {
        let mut counters = initial_valuation.clone();
        for edge in self.iter() {
            counters.apply_cfg_update(edge);
        }
        counters
    }

    /// Finds the first counter that turns negative along the path. If no
    /// counter turns negative `None` is returned. If a counter is found,
    /// the counter-index and the position in the path is returned.
    pub fn find_negative_counter_forward(
        &self,
        initial_valuation: &VASSCounterValuation,
    ) -> Option<(VASSCounterIndex, usize)> {
        let mut counters = initial_valuation.clone();

        for (i, edge) in self.iter().enumerate() {
            counters.apply_cfg_update(edge);

            if let Some(counter) = counters.find_negative_counter() {
                return Some((counter, i));
            }
        }

        None
    }

    /// Finds the first counter that turns negative along the reversed path. If
    /// no counter turns negative `None` is returned. If a counter is found,
    /// the counter-index and the position in the path is returned.
    pub fn find_negative_counter_backward(
        &self,
        final_valuation: &VASSCounterValuation,
    ) -> Option<(VASSCounterIndex, usize)> {
        let mut counters = final_valuation.clone();

        for (i, edge) in self.iter().enumerate().rev() {
            counters.apply_cfg_update(edge.reverse());

            if let Some(counter) = counters.find_negative_counter() {
                return Some((counter, i));
            }
        }

        None
    }

    /// Checks if the path visits a product note more than a certain number of
    /// times.
    pub fn visits_node_multiple_times(&self, limit: u32) -> bool {
        let mut visited = HashMap::new();
        visited.insert(self.start(), 1);

        for (_, state) in self.iter_updates_and_state() {
            let value = visited.entry(state).or_insert(0);
            *value += 1;
            if *value > limit {
                return true;
            }
        }

        false
    }

    /// Checks if the path visits a product node more than a certain number of
    /// times while also having increasing counter valuations.
    pub fn is_counter_forwards_pumped(
        &self,
        dimension: usize,
        counter: VASSCounterIndex,
        limit: u32,
    ) -> bool {
        let mut visited = HashMap::new();
        let mut counters = VASSCounterValuation::zero(dimension);
        visited.insert(self.start(), (1, counters.clone()));

        // TODO: Currently we look at the entire product here, but this is a problem.
        // Since the product contains the bounded counting separators, we don't actually
        // see pumping in the product since we always change state in some
        // bounded counting separator.
        //
        // Our current metric for pumping is that we see the same product state multiple
        // times with increasing counter valuations, but exactly that does not
        // happen due to the bounded counting separators.

        // 1. go back to looking only at the main cfg
        // 2. look at the product, but not at the indices corresponding to the modulo
        //    and bounded counting separators
        // 3. use some other metric for pumping

        for (update, state) in self.iter_updates_and_state() {
            counters.apply_cfg_update(*update);

            let entry = visited
                .entry(state)
                .or_insert((0, VASSCounterValuation::zero(dimension)));

            // check that we have pumped and that we pumped the counter we care about
            if counters >= entry.1 && counters[counter] > entry.1[counter] {
                entry.0 += 1;
                entry.1 = counters.clone();
                if entry.0 > limit {
                    return true;
                }
            }
        }

        false
    }

    /// Checks if the path visits a product node more than a certain number of
    /// times while also having decreasing counter valuations.
    pub fn is_counter_backwards_pumped(
        &self,
        dimension: usize,
        counter: VASSCounterIndex,
        limit: u32,
    ) -> bool {
        let mut visited = HashMap::new();
        let mut counters = VASSCounterValuation::zero(dimension);
        visited.insert(self.start(), (1, counters.clone()));

        // iterate in reverse order
        for (update, state) in self.iter_updates_and_state().rev() {
            // apply the reverse update since we are going backwards
            counters.apply_cfg_update(update.reverse());

            let entry = visited
                .entry(state)
                .or_insert((0, VASSCounterValuation::zero(dimension)));

            // check that we have pumped and that we pumped the counter we care about
            if counters >= entry.1 && counters[counter] > entry.1[counter] {
                entry.0 += 1;
                entry.1 = counters.clone();
                if entry.0 > limit {
                    return true;
                }
            }
        }

        false
    }

    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            updates: self.updates[range.clone()].to_vec(),
            states: self.states[range.start..=range.end].to_vec(),
        }
    }

    pub fn split_at(self, f: impl Fn(&MultiGraphState) -> bool) -> Vec<Self> {
        let mut parts = vec![];
        let mut current_part = MultiGraphPath::new(self.start().clone());

        for (update, state) in self.iter_updates_and_state() {
            current_part.add(*update, state.clone());

            if f(state) {
                parts.push(current_part);
                current_part = MultiGraphPath::new(state.clone());
            }
        }

        parts.push(current_part);

        parts
    }

    pub fn max_counter_value(
        &self,
        initial_valuation: &VASSCounterValuation,
        counter: VASSCounterIndex,
    ) -> i32 {
        let counter_updates = self.iter().filter(|update| update.counter() == counter);

        let mut value = initial_valuation[counter];
        let mut max_value = initial_valuation[counter];
        for update in counter_updates {
            value += update.op();
            max_value = max_value.max(value);
        }

        max_value
    }

    pub fn max_counter_value_from_back(
        &self,
        final_valuation: &VASSCounterValuation,
        counter: VASSCounterIndex,
    ) -> i32 {
        let counter_updates = self
            .iter()
            .rev()
            .filter(|update| update.counter() == counter);

        let mut value = final_valuation[counter];
        let mut max_value = final_valuation[counter];
        for update in counter_updates {
            value -= update.op();
            max_value = max_value.max(value);
        }

        max_value
    }

    pub fn to_fancy_string(&self) -> String {
        self.updates
            .iter()
            .map(|u| format!("{}", u))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
