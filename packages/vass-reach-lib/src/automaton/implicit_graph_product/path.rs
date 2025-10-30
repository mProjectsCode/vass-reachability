use petgraph::visit::EdgeRef;

use crate::automaton::{
    cfg::{
        update::{CFGCounterUpdatable, CFGCounterUpdate},
        vasscfg::VASSCFG,
    },
    implicit_graph_product::state::MultiGraphState,
    path::{Path, PathNReaching, path_like::PathLike},
    vass::counter::{VASSCounterIndex, VASSCounterValuation},
};

/// A path in the product of multiple graphs, storing the sequence of updates
/// and the end state.
#[derive(Debug, Clone)]
pub struct MultiGraphPath {
    pub updates: Vec<CFGCounterUpdate>,
    pub end_state: MultiGraphState,
}

impl MultiGraphPath {
    pub fn new(start_state: MultiGraphState) -> Self {
        MultiGraphPath {
            updates: vec![],
            end_state: start_state,
        }
    }

    pub fn to_path(&self, cfg: &VASSCFG<()>) -> Path {
        let start = cfg.get_start().expect("CFG should have a start node");
        let mut last_node = start;
        let mut path = Path::new(start);

        for update in &self.updates {
            let edge_ref = cfg
                .graph
                .edges_directed(last_node, petgraph::Direction::Outgoing)
                .find(|e| e.weight() == update)
                .expect("Path should be valid");

            path.add(edge_ref.id(), edge_ref.target());
            last_node = edge_ref.target();
        }

        path
    }

    pub fn add_clone(&self, letter: CFGCounterUpdate, target: MultiGraphState) -> Self {
        let mut new_updates = self.updates.clone();
        new_updates.push(letter);

        MultiGraphPath {
            updates: new_updates,
            end_state: target,
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = CFGCounterUpdate> + '_ {
        self.updates.iter().copied()
    }

    pub fn is_n_reaching(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> (PathNReaching, VASSCounterValuation) {
        let mut counters = initial_valuation.clone();
        let mut negative_index = None;

        for (i, edge) in self.iter().enumerate() {
            counters.apply_cfg_update(edge);

            let negative_counter = counters.find_negative_counter();
            if negative_index.is_none()
                && let Some(counter) = negative_counter
            {
                negative_index = Some((i, counter));
            }
        }

        if let Some(index) = negative_index {
            (PathNReaching::Negative(index), counters)
        } else {
            (
                PathNReaching::from_bool(&counters == final_valuation),
                counters,
            )
        }
    }

    pub fn max_counter_value(
        &self,
        initial_valuation: &VASSCounterValuation,
        counter: VASSCounterIndex,
    ) -> u32 {
        let counter_updates = self.iter().filter(|update| update.counter() == counter);

        let mut value = initial_valuation[counter];
        let mut max_value = 1;
        for update in counter_updates {
            value += update.op();
            max_value = max_value.max(value);
        }

        max_value as u32
    }

    pub fn to_fancy_string(&self) -> String {
        self.updates
            .iter()
            .map(|u| format!("{}", u))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
