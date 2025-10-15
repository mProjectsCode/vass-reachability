use rand::{Rng, SeedableRng, rngs::StdRng};
use vass_reach_lib::automaton::{
    petri_net::{PetriNet, initialized::InitializedPetriNet},
    vass::counter::VASSCounterValuation,
};

use crate::random::RandomOptions;

pub fn generate_random_petri_net(
    options: RandomOptions,
    place_count: usize,
    transition_count: usize,
    max_tokens_per_transition: usize,
) -> Vec<InitializedPetriNet> {
    let mut r = StdRng::seed_from_u64(options.seed);

    (0..options.count)
        .map(|_| {
            let mut petri_net = PetriNet::new(place_count);

            for _ in 0..transition_count {
                let mut input = vec![];
                let mut output = vec![];

                for p in 1..=place_count {
                    input.push((r.gen_range(0..max_tokens_per_transition), p));
                    output.push((r.gen_range(0..max_tokens_per_transition), p));
                }

                petri_net.add_transition(input, output);
            }

            let initial_m: VASSCounterValuation = (0..place_count)
                .into_iter()
                .map(|_| r.gen_range(0..max_tokens_per_transition) as i32)
                .collect();
            let final_m: VASSCounterValuation = (0..place_count)
                .into_iter()
                .map(|_| r.gen_range(0..max_tokens_per_transition) as i32)
                .collect();

            petri_net.init(initial_m, final_m)
        })
        .collect()
}
