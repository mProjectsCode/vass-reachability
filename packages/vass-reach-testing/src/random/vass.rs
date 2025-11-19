use rand::{Rng, SeedableRng, rngs::StdRng};
use vass_reach_lib::automaton::{
    AutBuild,
    vass::{VASS, counter::VASSCounterValuation, initialized::InitializedVASS},
};

use crate::random::RandomOptions;

pub fn generate_radom_vass(
    options: RandomOptions,
    state_count: usize,
    dimension: usize,
    transition_count: usize,
    max_tokens_per_transition: i32,
) -> Vec<InitializedVASS<(), usize>> {
    let mut r = StdRng::seed_from_u64(options.seed);
    let alphabet = (0..transition_count).collect::<Vec<_>>();

    (0..options.count)
        .map(|_| {
            let mut vass = VASS::<(), usize>::new(dimension, alphabet.clone());

            let mut states = vec![];
            for _i in 0..state_count {
                let state = vass.add_state(());
                states.push(state);
            }

            for i in 0..transition_count {
                let from = r.gen_range(0..state_count);
                let to = r.gen_range(0..state_count);

                let mut input = vec![];

                for _ in 0..dimension {
                    input.push(r.gen_range(-max_tokens_per_transition..=max_tokens_per_transition));
                }

                vass.add_transition(states[from], states[to], (i, input.into()));
            }

            let initial_m: VASSCounterValuation = (0..dimension)
                .into_iter()
                .map(|_| r.gen_range(0..=max_tokens_per_transition))
                .collect();

            let final_m: VASSCounterValuation = (0..dimension)
                .into_iter()
                .map(|_| r.gen_range(0..=max_tokens_per_transition))
                .collect();

            vass.init(initial_m, final_m, states[0], states[state_count - 1])
        })
        .collect()
}
