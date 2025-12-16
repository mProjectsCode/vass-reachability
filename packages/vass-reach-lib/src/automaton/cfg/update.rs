use std::{
    fmt::{Debug, Display},
    str::FromStr,
};

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

    /// Returns the increment or decrement value of the counter update.
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

impl FromStr for CFGCounterUpdate {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        let first = chars.next();
        let Some(first) = first else {
            anyhow::bail!("expected \"+\" or \"-\" at position 0, received eof")
        };
        let positive = if first == '+' {
            true
        } else if first == '-' {
            false
        } else {
            anyhow::bail!(
                "expected \"+\" or \"-\" at position 0, received \"{}\"",
                first
            )
        };
        let second = chars.next();
        let Some(second) = second else {
            anyhow::bail!("expected \"c\" at position 1, received eof")
        };
        if second != 'c' {
            anyhow::bail!("expected \"c\" at position 1, received \"{}\"", second)
        }

        let mut number = 0;
        let mut index = 2;
        while let Some(char) = chars.next() {
            if let Some(digit) = char.to_digit(10) {
                number = number * 10 + digit;
            } else {
                anyhow::bail!(
                    "expected digit at position {}, received \"{}\"",
                    index,
                    char
                )
            }

            index += 1;
        }

        Ok(CFGCounterUpdate::new(number, positive))
    }
}

impl CFGCounterUpdate {
    pub fn from_str_to_vec(s: &str) -> anyhow::Result<Vec<CFGCounterUpdate>> {
        s.split(" ").map(|p| p.parse()).collect()
    }
}

#[test]
fn test_cfg_counter_update_parser() {
    use itertools::Itertools;

    let counters = [
        CFGCounterUpdate::new(0, true),
        CFGCounterUpdate::new(0, false),
        CFGCounterUpdate::new(123, true),
        CFGCounterUpdate::new(123, false),
    ];

    for c in counters {
        assert_eq!(c, c.to_string().parse().unwrap())
    }

    let s = counters.iter().map(|c| c.to_string()).join(" ");

    assert_eq!(
        counters.as_slice(),
        &CFGCounterUpdate::from_str_to_vec(&s).unwrap()
    );
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
