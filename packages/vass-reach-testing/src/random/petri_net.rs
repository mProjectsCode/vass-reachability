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
    no_guards: bool,
) -> Vec<InitializedPetriNet> {
    let mut r = StdRng::seed_from_u64(options.seed);

    (0..options.count)
        .map(|_| {
            let mut petri_net = PetriNet::new(place_count);

            for _ in 0..transition_count {
                if no_guards {
                    generate_guard_free_transition(
                        &mut r,
                        &mut petri_net,
                        place_count,
                        max_tokens_per_transition,
                    );
                } else {
                    generate_transition(
                        &mut r,
                        &mut petri_net,
                        place_count,
                        max_tokens_per_transition,
                    );
                }
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

fn generate_transition(
    r: &mut StdRng,
    net: &mut PetriNet,
    place_count: usize,
    max_tokens_per_transition: usize,
) {
    let mut input = vec![];
    let mut output = vec![];

    for p in 1..=place_count {
        input.push((r.gen_range(0..max_tokens_per_transition), p));
        output.push((r.gen_range(0..max_tokens_per_transition), p));
    }

    net.add_transition(input, output);
}

fn generate_guard_free_transition(
    r: &mut StdRng,
    net: &mut PetriNet,
    place_count: usize,
    max_tokens_per_transition: usize,
) {
    let mut input = vec![];
    let mut output = vec![];

    assert!(max_tokens_per_transition > 0);

    for p in 1..=place_count {
        let change = r.gen_range(1..2 * max_tokens_per_transition);
        let change = (change as i64) - max_tokens_per_transition as i64;
        if change > 0 {
            input.push((change as usize, p));
        }
        if change < 0 {
            output.push((change.unsigned_abs() as usize, p));
        }
    }

    net.add_transition(input, output);
}
