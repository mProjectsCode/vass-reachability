use crate::automaton::{
    cfg::CFG, path::parikh_image::ParikhImage, vass::counter::VASSCounterValuation,
};

pub mod same_language;

/// Tests that a given Parikh image is arrives at the final valuation in a VASS
/// CFG.
pub fn test_parikh_image<C: CFG>(
    parikh_image: &ParikhImage<C::EIndex>,
    cfg: &C,
    initial_valuation: &VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
) {
    let effect = parikh_image.get_total_counter_effect(cfg, initial_valuation.dimension());
    let mut valuation = initial_valuation.clone();
    valuation.apply_update(&effect);

    assert_eq!(&valuation, final_valuation);
}
