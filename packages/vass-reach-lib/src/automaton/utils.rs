use crate::automaton::{
    cfg::update::{CFGCounterUpdatable, CFGCounterUpdate},
    vass::counter::{VASSCounterUpdate, VASSCounterValuation},
};

/// Converts a sequence of CFG counter updates to a pair of valuations.
///
/// The first valuation is the minimum valuation that is reached by the updates.
/// It needs to be subtracted from the counters first.
/// The second valuation is the valuation that needs to be added after
/// subtracting the first valuation to reach the final valuation.
pub fn cfg_updates_to_counter_updates(
    updates: impl Iterator<Item = CFGCounterUpdate>,
    dimension: usize,
) -> (VASSCounterUpdate, VASSCounterUpdate) {
    let mut min_counters = VASSCounterValuation::from(vec![0; dimension]);
    let mut counters = VASSCounterValuation::from(vec![0; dimension]);

    for update in updates {
        counters.apply_cfg_update(update);

        for (counter, min_counter) in counters.iter().zip(min_counters.iter_mut()) {
            *min_counter = (*min_counter).min(*counter);
        }
    }

    for min_counter in min_counters.iter_mut() {
        *min_counter = min_counter.abs();
    }

    for (counter, min_counter) in counters.iter_mut().zip(min_counters.iter()) {
        *counter += min_counter;
    }

    (min_counters.to_update(), counters.to_update())
}

pub fn cfg_updates_to_counter_update(
    updates: impl Iterator<Item = CFGCounterUpdate>,
    dimension: usize,
) -> VASSCounterUpdate {
    let mut counters = VASSCounterUpdate::zero(dimension);

    for update in updates {
        counters[update.counter()] += update.op();
    }

    counters
}

pub fn vass_update_to_cfg_updates(update: &VASSCounterUpdate) -> Vec<CFGCounterUpdate> {
    let mut vec = vec![];

    for (i, m) in update.iter().enumerate() {
        let label = CFGCounterUpdate::new(i as u32, *m > 0);

        for _ in 0..m.abs() {
            vec.push(label);
        }
    }

    vec
}
