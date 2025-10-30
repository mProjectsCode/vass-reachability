use std::fmt::{Debug, Display};

use crate::automaton::vass::counter::{VASSCounterIndex, VASSCounterValuation};

/// Macro to create a cfg increment update
#[macro_export]
macro_rules! cfg_inc {
    ($x:expr) => {
        CFGCounterUpdate::new($x as u32, true)
    };
}

/// Macro to create a cfg decrement update
#[macro_export]
macro_rules! cfg_dec {
    ($x:expr) => {
        CFGCounterUpdate::new($x as u32, false)
    };
}

/// A counter update in a CFG.
#[derive(Copy, Clone, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct CFGCounterUpdate {
    counter: VASSCounterIndex,
    positive: bool,
}

impl CFGCounterUpdate {
    pub fn new(index: u32, positive: bool) -> Self {
        CFGCounterUpdate {
            counter: VASSCounterIndex::new(index),
            positive,
        }
    }

    pub fn positive(counter: VASSCounterIndex) -> Self {
        CFGCounterUpdate {
            counter,
            positive: true,
        }
    }

    pub fn negative(counter: VASSCounterIndex) -> Self {
        CFGCounterUpdate {
            counter,
            positive: false,
        }
    }

    pub fn to_positive(&self) -> Self {
        CFGCounterUpdate {
            counter: self.counter,
            positive: true,
        }
    }

    pub fn to_negative(&self) -> Self {
        CFGCounterUpdate {
            counter: self.counter,
            positive: false,
        }
    }

    pub fn reverse(&self) -> Self {
        CFGCounterUpdate {
            counter: self.counter,
            positive: !self.positive,
        }
    }

    /// Constructs an alphabet of counter updates for a CFG with `counter_count`
    /// counters. Meaning all counter updates from `1` to `counter_count`
    /// and `-1` to `-counter_count`.
    pub fn alphabet(counter_count: usize) -> Vec<CFGCounterUpdate> {
        (0..counter_count)
            .map(|c| CFGCounterUpdate::new(c as u32, true))
            .chain((0..counter_count).map(|c| CFGCounterUpdate::new(c as u32, false)))
            .collect()
    }

    /// Returns the counter index.
    pub fn counter(&self) -> VASSCounterIndex {
        self.counter
    }

    /// Returns the increment or decrement value of the counter update.
    pub fn op(&self) -> i32 {
        if self.positive { 1 } else { -1 }
    }

    pub fn op_i64(&self) -> i64 {
        if self.positive { 1 } else { -1 }
    }
}

impl Display for CFGCounterUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            if self.positive { '+' } else { '-' },
            self.counter
        )
    }
}

impl Debug for CFGCounterUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self)
    }
}

pub trait CFGCounterUpdatable {
    fn apply_cfg_update(&mut self, update: CFGCounterUpdate);
    fn apply_cfg_update_times(&mut self, update: CFGCounterUpdate, times: i32);
    fn apply_cfg_update_mod(&mut self, update: CFGCounterUpdate, modulo: i32);
    fn apply_cfg_update_mod_slice(&mut self, update: CFGCounterUpdate, modulo: &[i32]);
    fn can_apply_cfg_update(&self, update: &CFGCounterUpdate) -> bool;
}

impl CFGCounterUpdatable for VASSCounterValuation {
    fn apply_cfg_update(&mut self, update: CFGCounterUpdate) {
        self[update.counter()] += update.op();
    }

    fn apply_cfg_update_times(&mut self, update: CFGCounterUpdate, times: i32) {
        self[update.counter()] += update.op() * times;
    }

    fn apply_cfg_update_mod(&mut self, update: CFGCounterUpdate, modulo: i32) {
        let counter = update.counter();
        self[counter] = (self[counter] + update.op()).rem_euclid(modulo);
    }

    fn apply_cfg_update_mod_slice(&mut self, update: CFGCounterUpdate, modulo: &[i32]) {
        let counter = update.counter();
        self[counter] = (self[counter] + update.op()).rem_euclid(modulo[counter.to_usize()]);
    }

    fn can_apply_cfg_update(&self, update: &CFGCounterUpdate) -> bool {
        if update.positive {
            true
        } else {
            self[update.counter()] > 0
        }
    }
}
