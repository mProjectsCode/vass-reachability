use std::num::NonZeroI32;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CFGCounterUpdate(pub NonZeroI32);

impl CFGCounterUpdate {
    pub fn new(weight: i32) -> Option<Self> {
        NonZeroI32::new(weight).map(CFGCounterUpdate)
    }

    pub fn alphabet(counter_count: usize) -> Vec<CFGCounterUpdate> {
        let counter_count = counter_count as i32;
        (1..=counter_count)
            .chain((1..=counter_count).map(|x| -x))
            .map(|i| CFGCounterUpdate::new(i).unwrap())
            .collect()
    }

    /// Returns the counter index.
    pub fn counter(&self) -> usize {
        (self.0.get().abs() - 1) as usize
    }

    /// Returns the increment or decrement value of the counter update.
    pub fn op(&self) -> i32 {
        self.0.get().signum()
    }

    pub fn op_i64(&self) -> i64 {
        self.0.get().signum() as i64
    }

    pub fn apply(&self, counters: &mut [i32]) {
        counters[self.counter()] += self.op();
    }

    pub fn apply_n(&self, counters: &mut [i32], times: i32) {
        counters[self.counter()] += self.op() * times;
    }

    pub fn apply_mod(&self, counters: &mut [i32], modulo: i32) {
        counters[self.counter()] = (counters[self.counter()] + self.op()).rem_euclid(modulo);
    }
}

impl From<CFGCounterUpdate> for NonZeroI32 {
    fn from(x: CFGCounterUpdate) -> Self {
        x.0
    }
}

impl From<CFGCounterUpdate> for i32 {
    fn from(x: CFGCounterUpdate) -> Self {
        x.0.get()
    }
}

impl From<NonZeroI32> for CFGCounterUpdate {
    fn from(x: NonZeroI32) -> Self {
        CFGCounterUpdate(x)
    }
}
