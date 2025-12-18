use crate::automaton::{
    cfg::{
        CFG,
        update::{CFGCounterUpdatable, CFGCounterUpdate},
    },
    index_map::IndexMap,
    path::Path,
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

/// A path in the product of multiple graphs, storing the sequence of updates.
#[derive(Debug, Clone)]
pub struct MultiGraphPath {
    pub updates: Vec<CFGCounterUpdate>,
}

impl MultiGraphPath {
    pub fn new() -> Self {
        MultiGraphPath { updates: vec![] }
    }

    pub fn to_path<C: CFG>(&self, cfg: &C) -> Path<C::NIndex, CFGCounterUpdate> {
        let start = cfg.get_initial();
        let mut path = Path::new(start);

        for update in &self.updates {
            let target = cfg
                .successor(path.end(), update)
                .expect("path to be valid with in CFG");

            path.add(*update, target);
        }

        path
    }

    pub fn len(&self) -> usize {
        self.updates.len()
    }

    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    pub fn add(&mut self, letter: CFGCounterUpdate) {
        self.updates.push(letter);
    }

    pub fn iter(
        &self,
    ) -> impl DoubleEndedIterator + ExactSizeIterator + Iterator<Item = CFGCounterUpdate> + '_ {
        self.updates.iter().copied()
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

    /// Checks if the path visits a cfg note more than a certain number of
    /// times.
    pub fn visits_node_multiple_times(&self, cfg: &impl CFG, limit: u32) -> bool {
        let start = cfg.get_initial();
        let mut last_node = start;
        let mut visited = IndexMap::new(cfg.node_count());
        visited.insert(last_node, 1);

        for update in &self.updates {
            let edge_ref = cfg
                .outgoing_edge_indices(last_node)
                .find(|e| cfg.get_edge_unchecked(*e) == update)
                .expect("Path should be valid");

            last_node = cfg.edge_endpoints_unchecked(edge_ref).1;
            let value = visited.get_mut(last_node);
            *value += 1;
            if *value > limit {
                return true;
            }
        }

        false
    }

    pub fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            updates: self.updates[range].to_vec(),
        }
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
