use std::ops::Neg;

use crate::automaton::dfa::cfg::CFGCounterUpdate;

/// Utility methods for working with valuations of VASS.
pub trait VASSValuation {
    fn neg(&self) -> Self;
    fn neg_mut(&mut self);
    fn add(&self, other: &Self) -> Self;
    fn add_mut(&mut self, other: &Self);
    fn mod_euclid(&self, modulus: u32) -> Self;
    fn mod_euclid_mut(&mut self, modulus: u32);
}

impl VASSValuation for Box<[i32]> {
    fn neg(&self) -> Self {
        self.iter().map(|x| x.neg()).collect()
    }

    fn neg_mut(&mut self) {
        for x in self.iter_mut() {
            *x = x.neg();
        }
    }

    fn add(&self, other: &Self) -> Self {
        self.iter().zip(other.iter()).map(|(x, y)| x + y).collect()
    }

    fn add_mut(&mut self, other: &Self) {
        for (x, y) in self.iter_mut().zip(other.iter()) {
            *x += y;
        }
    }

    fn mod_euclid(&self, modulus: u32) -> Self {
        self.iter().map(|x| x.rem_euclid(modulus as i32)).collect()
    }

    fn mod_euclid_mut(&mut self, modulus: u32) {
        for x in self.iter_mut() {
            *x = x.rem_euclid(modulus as i32);
        }
    }
}

/// Converts a sequence of CFG counter updates to a pair of valuations.
///
/// The first valuation is the minimum valuation that is reached by the updates.
/// It needs to be subtracted from the counters first.
/// The second valuation is the valuation that needs to be added after
/// subtracting the first valuation to reach the final valuation.
pub fn cfg_updates_to_ltc_transition(
    updates: impl Iterator<Item = CFGCounterUpdate>,
    dimension: usize,
) -> (Box<[i32]>, Box<[i32]>) {
    let mut min_counters = vec![0; dimension].into_boxed_slice();
    let mut counters = vec![0; dimension].into_boxed_slice();

    for update in updates {
        update.apply(&mut counters);
        for (i, counter) in counters.iter().enumerate() {
            min_counters[i] = min_counters[i].min(*counter);
        }
    }

    for min_counter in min_counters.iter_mut() {
        *min_counter = min_counter.abs();
    }

    for (i, counter) in counters.iter_mut().enumerate() {
        *counter += min_counters[i];
    }

    (min_counters, counters)
}

pub fn vass_update_to_cfg_updates(marking: &[i32]) -> Vec<CFGCounterUpdate> {
    let mut vec = vec![];

    for (i, m) in marking.iter().enumerate() {
        let index = (i + 1) as i32;

        let label = if *m > 0 {
            CFGCounterUpdate::new(index).unwrap()
        } else {
            CFGCounterUpdate::new(-index).unwrap()
        };

        for _ in 0..m.abs() {
            vec.push(label);
        }
    }

    vec
}
