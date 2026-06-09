use rand::{RngExt, SeedableRng, rngs::StdRng};
use vass_reach_lib::automaton::{
    ModifiableAutomaton,
    vass::{VASS, VASSEdge, counter::VASSCounterValuation, initialized::InitializedVASS},
};

use crate::{
    config::{InclusiveI32Range, InclusiveUsizeRange},
    random::RandomOptions,
};

pub fn generate_random_vass(
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
                let state = vass.add_node(());
                states.push(state);
            }

            for i in 0..transition_count {
                let from = r.random_range(0..state_count);
                let to = r.random_range(0..state_count);

                let mut input = vec![];

                for _ in 0..dimension {
                    input.push(
                        r.random_range(-max_tokens_per_transition..=max_tokens_per_transition),
                    );
                }

                vass.add_edge(&states[from], &states[to], VASSEdge::new(i, input.into()));
            }

            let initial_m: VASSCounterValuation = (0..dimension)
                .map(|_| r.random_range(0..=max_tokens_per_transition))
                .collect();

            let final_m: VASSCounterValuation = (0..dimension)
                .map(|_| r.random_range(0..=max_tokens_per_transition))
                .collect();

            vass.init(initial_m, final_m, states[0], states[state_count - 1])
        })
        .collect()
}

pub fn generate_random_vass_in_ranges(
    options: RandomOptions,
    counters: InclusiveUsizeRange,
    states: InclusiveUsizeRange,
    transitions: InclusiveUsizeRange,
    updates: InclusiveI32Range,
    valuations: InclusiveI32Range,
) -> anyhow::Result<Vec<InitializedVASS<usize, usize>>> {
    counters.validate("vass_counters")?;
    states.validate("vass_states")?;
    transitions.validate("vass_transitions")?;
    updates.validate("vass_updates")?;
    valuations.validate("vass_valuations")?;
    if counters.min == 0 || states.min == 0 {
        anyhow::bail!("VASS counter and state ranges must start at 1 or greater");
    }
    if valuations.min < 0 {
        anyhow::bail!("VASS initial/final valuations must be non-negative");
    }

    let mut r = StdRng::seed_from_u64(options.seed);
    let mut result = Vec::with_capacity(options.count);
    for _ in 0..options.count {
        let dimension = r.random_range(counters.min..=counters.max);
        let state_count = r.random_range(states.min..=states.max);
        let minimum_edges = state_count.saturating_sub(1);
        let transition_min = transitions.min.max(minimum_edges);
        if transition_min > transitions.max {
            anyhow::bail!(
                "transition range cannot connect the maximum selected state count: need at least {}",
                minimum_edges
            );
        }
        let transition_count = r.random_range(transition_min..=transitions.max);
        let alphabet = (0..transition_count).collect::<Vec<_>>();
        let mut vass = VASS::<usize, usize>::new(dimension, alphabet);
        let nodes = (0..state_count)
            .map(|index| vass.add_node(index))
            .collect::<Vec<_>>();

        // A forward backbone guarantees an initial-to-final control path.
        for edge in 0..minimum_edges {
            let update = (0..dimension)
                .map(|_| r.random_range(updates.min..=updates.max))
                .collect::<Vec<_>>();
            vass.add_edge(
                &nodes[edge],
                &nodes[edge + 1],
                VASSEdge::new(edge, update.into()),
            );
        }
        for edge in minimum_edges..transition_count {
            let source = r.random_range(0..state_count);
            let target = r.random_range(0..state_count);
            let update = (0..dimension)
                .map(|_| r.random_range(updates.min..=updates.max))
                .collect::<Vec<_>>();
            vass.add_edge(
                &nodes[source],
                &nodes[target],
                VASSEdge::new(edge, update.into()),
            );
        }

        let initial = (0..dimension)
            .map(|_| r.random_range(valuations.min..=valuations.max))
            .collect::<Vec<_>>();
        let final_valuation = (0..dimension)
            .map(|_| r.random_range(valuations.min..=valuations.max))
            .collect::<Vec<_>>();
        result.push(vass.init(
            initial.into(),
            final_valuation.into(),
            nodes[0],
            nodes[state_count - 1],
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use vass_reach_lib::automaton::AutomatonIterators;

    use super::generate_random_vass_in_ranges;
    use crate::{
        config::{InclusiveI32Range, InclusiveUsizeRange},
        random::RandomOptions,
    };

    #[test]
    fn ranged_generation_is_deterministic_and_connected() {
        let generate = || {
            generate_random_vass_in_ranges(
                RandomOptions::new(42, 5),
                InclusiveUsizeRange { min: 2, max: 4 },
                InclusiveUsizeRange { min: 2, max: 4 },
                InclusiveUsizeRange { min: 3, max: 7 },
                InclusiveI32Range { min: -2, max: 2 },
                InclusiveI32Range { min: 0, max: 3 },
            )
            .unwrap()
        };
        let first = generate();
        let second = generate();
        let first_json = first
            .iter()
            .map(|instance| instance.to_json().unwrap())
            .collect::<Vec<_>>();
        let second_json = second
            .iter()
            .map(|instance| instance.to_json().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(first_json, second_json);
        for instance in first {
            assert!((2..=4).contains(&instance.dimension()));
            assert!((2..=4).contains(&instance.state_count()));
            assert!((3..=7).contains(&instance.transition_count()));
            assert!(instance.iter_node_indices().count() >= 2);
        }
    }
}
