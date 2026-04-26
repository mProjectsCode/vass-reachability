use std::fmt::{Display, Formatter};

use crate::automaton::{cfg::update::CFGCounterUpdate, vass::counter::VASSCounterValuation};

/// A single component in a coverability valuation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OmegaCounter {
    Finite(i32),
    Omega,
}

impl OmegaCounter {
    fn greater_than(self, other: Self) -> bool {
        match (self, other) {
            (OmegaCounter::Finite(a), OmegaCounter::Finite(b)) => a > b,
            (OmegaCounter::Omega, OmegaCounter::Finite(_)) => true,
            _ => false,
        }
    }

    fn leq(self, other: Self) -> bool {
        match (self, other) {
            (OmegaCounter::Finite(a), OmegaCounter::Finite(b)) => a <= b,
            (OmegaCounter::Finite(_), OmegaCounter::Omega) => true,
            (OmegaCounter::Omega, OmegaCounter::Omega) => true,
            (OmegaCounter::Omega, OmegaCounter::Finite(_)) => false,
        }
    }

    fn can_apply(self, update: CFGCounterUpdate) -> bool {
        if update.op() >= 0 {
            return true;
        }

        match self {
            OmegaCounter::Omega => true,
            OmegaCounter::Finite(v) => v > 0,
        }
    }

    fn apply(self, update: CFGCounterUpdate) -> Self {
        match self {
            OmegaCounter::Omega => OmegaCounter::Omega,
            OmegaCounter::Finite(v) => OmegaCounter::Finite(v + update.op()),
        }
    }
}

impl Display for OmegaCounter {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            OmegaCounter::Finite(v) => write!(f, "{}", v),
            OmegaCounter::Omega => write!(f, "w"),
        }
    }
}

/// A counter valuation over N U {omega}.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct OmegaCounterValuation {
    values: Box<[OmegaCounter]>,
}

impl OmegaCounterValuation {
    pub fn new(values: Box<[OmegaCounter]>) -> Self {
        Self { values }
    }

    pub fn from_finite(valuation: &VASSCounterValuation) -> Self {
        Self {
            values: valuation
                .iter()
                .copied()
                .map(OmegaCounter::Finite)
                .collect(),
        }
    }

    pub fn dimension(&self) -> usize {
        self.values.len()
    }

    pub fn values(&self) -> &[OmegaCounter] {
        &self.values
    }

    pub fn can_apply_cfg_update(&self, update: CFGCounterUpdate) -> bool {
        self.values[update.counter().to_usize()].can_apply(update)
    }

    pub fn apply_cfg_update(&mut self, update: CFGCounterUpdate) {
        let index = update.counter().to_usize();
        self.values[index] = self.values[index].apply(update);
    }

    pub fn leq(&self, other: &Self) -> bool {
        if self.dimension() != other.dimension() {
            return false;
        }

        self.values
            .iter()
            .copied()
            .zip(other.values.iter().copied())
            .all(|(a, b)| a.leq(b))
    }

    pub fn accelerate_with(&mut self, ancestor: &Self) {
        debug_assert_eq!(self.dimension(), ancestor.dimension());

        for (value, ancestor_value) in self.values.iter_mut().zip(ancestor.values.iter().copied()) {
            if value.greater_than(ancestor_value) {
                *value = OmegaCounter::Omega;
            }
        }
    }
}

impl Display for OmegaCounterValuation {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let values = self
            .values
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "[{}]", values)
    }
}
