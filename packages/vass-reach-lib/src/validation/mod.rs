use crate::automaton::{
    AutomatonNode,
    dfa::cfg::{CFGCounterUpdatable, VASSCFG},
    path::parikh_image::ParikhImage,
    vass::counter::VASSCounterValuation,
};

pub mod same_language;

/// Tests that a given Parikh image is arrives at the final valuation in a VASS
/// CFG.
pub fn test_parikh_image<N: AutomatonNode>(
    parikh_image: &ParikhImage,
    cfg: &VASSCFG<N>,
    initial_valuation: &VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
) {
    let mut valuation: VASSCounterValuation = initial_valuation.clone();

    for (edge, count) in parikh_image.image.iter() {
        let update = cfg.graph.edge_weight(*edge).unwrap();

        valuation.apply_cfg_update_times(*update, *count as i32);
    }

    assert_eq!(&valuation, final_valuation);
}
