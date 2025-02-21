use crate::automaton::{dfa::VASSCFG, parikh_image::ParikhImage, AutomatonNode};

pub mod same_language;

pub fn test_parikh_image<N: AutomatonNode>(
    parikh_image: &ParikhImage,
    cfg: &VASSCFG<N>,
    initial_valuation: &[i32],
    final_valuation: &[i32],
) {
    let mut valuation: Box<[i32]> = initial_valuation.into();

    for (edge, count) in parikh_image.image.iter() {
        let update = cfg.graph.edge_weight(*edge).unwrap();

        update.apply_n(&mut valuation, *count as i32);
    }

    assert_eq!(valuation.as_ref(), final_valuation);
}
