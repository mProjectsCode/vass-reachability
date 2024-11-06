use std::ops::Neg;

pub fn neg_vec(vec: &[i32]) -> Vec<i32> {
    vec.iter().map(|x| x.neg()).collect()
}

pub fn add_vec(a: &[i32], b: &[i32]) -> Vec<i32> {
    a.iter().zip(b.iter()).map(|(x, y)| x + y).collect()
}

pub fn mut_add_vec(a: &mut [i32], b: &[i32]) {
    for i in 0..a.len() {
        a[i] += b[i];
    }
}

pub fn dyck_transitions_to_ltc_transition(
    transitions: &[i32],
    dimension: usize,
) -> (Vec<i32>, Vec<i32>) {
    let mut min_couners = vec![0; dimension];
    let mut counters = vec![0; dimension];

    for t in transitions {
        if *t > 0 {
            counters[(t - 1) as usize] += 1;
        } else {
            min_couners[(-t - 1) as usize] += 1;
        }
    }

    (min_couners, counters)
}
