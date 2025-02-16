use crate::automaton::{dfa::DFA, parikh_image::ParikhImage, AutomatonNode};

pub mod same_language;

pub fn test_parikh_image<N: AutomatonNode>(
    parikh_image: &ParikhImage,
    cfg: &DFA<N, i32>,
    initial_valuation: &Box<[i32]>,
    final_valuation: &Box<[i32]>,
) {
    let mut valuation = initial_valuation.clone();

    for (edge, count) in parikh_image.image.iter() {
        let edge_data = cfg.graph.edge_weight(*edge).unwrap();

        let counter = (edge_data.abs() - 1) as usize;
        let sign = edge_data.signum();

        valuation[counter] += sign * (*count) as i32;
    }

    assert_eq!(valuation, *final_valuation);
}
