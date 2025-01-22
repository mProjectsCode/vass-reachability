use std::ops::Neg;

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

pub fn dyck_transitions_to_ltc_transition(
    transitions: &[i32],
    dimension: usize,
) -> (Box<[i32]>, Box<[i32]>) {
    let mut min_couners = vec![0; dimension].into_boxed_slice();
    let mut counters = vec![0; dimension].into_boxed_slice();

    for t in transitions {
        if *t > 0 {
            counters[(t - 1) as usize] += 1;
        } else {
            min_couners[(-t - 1) as usize] += 1;
        }
    }

    (min_couners, counters)
}
