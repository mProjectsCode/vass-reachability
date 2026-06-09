use vass_reach_lib::automaton::{
    ModifiableAutomaton,
    vass::{VASS, VASSEdge, initialized::InitializedVASS},
};

pub type PlaygroundVass = InitializedVASS<(), usize>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reduction {
    MergeStates,
    RemoveCounter {
        counter: usize,
    },
    RemoveTransition {
        transition: usize,
    },
    ReduceUpdate {
        transition: usize,
        counter: usize,
        from: i32,
        to: i32,
    },
    ReduceInitialValuation {
        counter: usize,
        from: i32,
        to: i32,
    },
    ReduceFinalValuation {
        counter: usize,
        from: i32,
        to: i32,
    },
}

#[derive(Debug, Clone)]
pub struct MinimizationResult {
    pub instance: PlaygroundVass,
    pub reductions: Vec<Reduction>,
}

/// Repeatedly applies the first accepted simplification until reaching a fixed
/// point.
///
/// The predicate defines the property that every accepted candidate must
/// preserve.
pub fn minimize(
    mut instance: PlaygroundVass,
    mut preserves_property: impl FnMut(&PlaygroundVass) -> bool,
) -> MinimizationResult {
    let mut reductions = Vec::new();

    loop {
        if instance.state_count() > 1 {
            let candidate = merge_states(&instance);
            if preserves_property(&candidate) {
                instance = candidate;
                reductions.push(Reduction::MergeStates);
                continue;
            }
        }

        if instance.dimension() > 1 {
            let mut accepted = None;
            for counter in 0..instance.dimension() {
                let candidate = remove_counter(&instance, counter);
                if preserves_property(&candidate) {
                    accepted = Some((counter, candidate));
                    break;
                }
            }
            if let Some((counter, candidate)) = accepted {
                instance = candidate;
                reductions.push(Reduction::RemoveCounter { counter });
                continue;
            }
        }

        let mut accepted = None;
        for transition in instance.vass.graph.edge_indices().collect::<Vec<_>>() {
            let mut candidate = instance.clone();
            candidate.vass.graph.remove_edge(transition);
            if preserves_property(&candidate) {
                accepted = Some((transition.index(), candidate));
                break;
            }
        }
        if let Some((transition, candidate)) = accepted {
            instance = candidate;
            reductions.push(Reduction::RemoveTransition { transition });
            continue;
        }

        let mut accepted = None;
        'transitions: for transition in instance.vass.graph.edge_indices().collect::<Vec<_>>() {
            for counter in 0..instance.dimension() {
                let from = instance.vass.graph[transition].update[counter];
                if from == 0 {
                    continue;
                }
                let to = from - from.signum();
                let mut candidate = instance.clone();
                candidate.vass.graph[transition].update[counter] = to;
                if preserves_property(&candidate) {
                    accepted = Some((transition.index(), counter, from, to, candidate));
                    break 'transitions;
                }
            }
        }
        if let Some((transition, counter, from, to, candidate)) = accepted {
            instance = candidate;
            reductions.push(Reduction::ReduceUpdate {
                transition,
                counter,
                from,
                to,
            });
            continue;
        }

        let mut accepted = None;
        for counter in 0..instance.dimension() {
            let from = instance.initial_valuation[counter];
            if from <= 0 {
                continue;
            }
            let mut candidate = instance.clone();
            candidate.initial_valuation[counter] -= 1;
            if preserves_property(&candidate) {
                accepted = Some((counter, from, candidate));
                break;
            }
        }
        if let Some((counter, from, candidate)) = accepted {
            instance = candidate;
            reductions.push(Reduction::ReduceInitialValuation {
                counter,
                from,
                to: from - 1,
            });
            continue;
        }

        let mut accepted = None;
        for counter in 0..instance.dimension() {
            let from = instance.final_valuation[counter];
            if from <= 0 {
                continue;
            }
            let mut candidate = instance.clone();
            candidate.final_valuation[counter] -= 1;
            if preserves_property(&candidate) {
                accepted = Some((counter, from, candidate));
                break;
            }
        }
        if let Some((counter, from, candidate)) = accepted {
            instance = candidate;
            reductions.push(Reduction::ReduceFinalValuation {
                counter,
                from,
                to: from - 1,
            });
            continue;
        }

        return MinimizationResult {
            instance,
            reductions,
        };
    }
}

/// Compares VASS instances modulo state names, counter order, transition order,
/// and transition letters.
pub fn equivalent(left: &PlaygroundVass, right: &PlaygroundVass) -> bool {
    canonical_form(left) == canonical_form(right)
}

fn merge_states(instance: &PlaygroundVass) -> PlaygroundVass {
    let mut vass = VASS::new(instance.dimension(), instance.vass.alphabet.clone());
    let state = vass.add_node(());
    for edge in instance.vass.graph.edge_weights() {
        vass.add_edge(&state, &state, edge.clone());
    }
    vass.init(
        instance.initial_valuation.clone(),
        instance.final_valuation.clone(),
        state,
        state,
    )
}

fn remove_counter(instance: &PlaygroundVass, removed: usize) -> PlaygroundVass {
    let mut vass = VASS::new(instance.dimension() - 1, instance.vass.alphabet.clone());
    let states = (0..instance.state_count())
        .map(|_| vass.add_node(()))
        .collect::<Vec<_>>();

    for transition in instance.vass.graph.edge_indices() {
        let (source, target) = instance.vass.graph.edge_endpoints(transition).unwrap();
        let edge = &instance.vass.graph[transition];
        let update = edge
            .update
            .iter()
            .enumerate()
            .filter_map(|(counter, value)| (counter != removed).then_some(*value))
            .collect::<Vec<_>>();
        vass.add_edge(
            &states[source.index()],
            &states[target.index()],
            VASSEdge::new(edge.data, update.into()),
        );
    }

    let initial = remove_coordinate(instance.initial_valuation.iter(), removed);
    let final_valuation = remove_coordinate(instance.final_valuation.iter(), removed);
    vass.init(
        initial.into(),
        final_valuation.into(),
        states[instance.initial_node.index()],
        states[instance.final_node.index()],
    )
}

fn remove_coordinate<'a>(values: impl Iterator<Item = &'a i32>, removed: usize) -> Vec<i32> {
    values
        .enumerate()
        .filter_map(|(counter, value)| (counter != removed).then_some(*value))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CanonicalForm {
    dimension: usize,
    state_count: usize,
    initial_state: usize,
    final_state: usize,
    initial_valuation: Vec<i32>,
    final_valuation: Vec<i32>,
    transitions: Vec<(usize, usize, Vec<i32>)>,
}

fn canonical_form(instance: &PlaygroundVass) -> CanonicalForm {
    let state_permutations = permutations(instance.state_count());
    let counter_permutations = permutations(instance.dimension());
    let mut best = None;

    for state_order in &state_permutations {
        let mut state_map = vec![0; state_order.len()];
        for (new, old) in state_order.iter().copied().enumerate() {
            state_map[old] = new;
        }
        for counter_order in &counter_permutations {
            let mut transitions = instance
                .vass
                .graph
                .edge_indices()
                .map(|transition| {
                    let (source, target) = instance.vass.graph.edge_endpoints(transition).unwrap();
                    let update = counter_order
                        .iter()
                        .map(|counter| instance.vass.graph[transition].update[*counter])
                        .collect();
                    (state_map[source.index()], state_map[target.index()], update)
                })
                .collect::<Vec<_>>();
            transitions.sort();

            let form = CanonicalForm {
                dimension: instance.dimension(),
                state_count: instance.state_count(),
                initial_state: state_map[instance.initial_node.index()],
                final_state: state_map[instance.final_node.index()],
                initial_valuation: counter_order
                    .iter()
                    .map(|counter| instance.initial_valuation[*counter])
                    .collect(),
                final_valuation: counter_order
                    .iter()
                    .map(|counter| instance.final_valuation[*counter])
                    .collect(),
                transitions,
            };
            if best.as_ref().is_none_or(|current| form < *current) {
                best = Some(form);
            }
        }
    }

    best.unwrap()
}

fn permutations(size: usize) -> Vec<Vec<usize>> {
    fn generate(index: usize, values: &mut Vec<usize>, output: &mut Vec<Vec<usize>>) {
        if index == values.len() {
            output.push(values.clone());
            return;
        }
        for swap_with in index..values.len() {
            values.swap(index, swap_with);
            generate(index + 1, values, output);
            values.swap(index, swap_with);
        }
    }

    let mut values = (0..size).collect();
    let mut output = Vec::new();
    generate(0, &mut values, &mut output);
    output
}

#[cfg(test)]
mod tests {
    use vass_reach_lib::automaton::{
        ModifiableAutomaton,
        vass::{VASS, VASSEdge},
    };

    use super::{Reduction, equivalent, minimize};

    #[test]
    fn equivalence_ignores_state_counter_and_transition_order() {
        let mut left = VASS::new(2, vec![0, 1]);
        let l0 = left.add_node(());
        let l1 = left.add_node(());
        left.add_edge(&l0, &l1, VASSEdge::new(0, vec![1, -1].into()));
        left.add_edge(&l1, &l0, VASSEdge::new(1, vec![0, 1].into()));
        let left = left.init(vec![1, 0].into(), vec![0, 1].into(), l0, l1);

        let mut right = VASS::new(2, vec![4, 8]);
        let r1 = right.add_node(());
        let r0 = right.add_node(());
        right.add_edge(&r0, &r1, VASSEdge::new(8, vec![-1, 1].into()));
        right.add_edge(&r1, &r0, VASSEdge::new(4, vec![1, 0].into()));
        let right = right.init(vec![0, 1].into(), vec![1, 0].into(), r0, r1);

        assert!(equivalent(&left, &right));
    }

    #[test]
    fn minimization_reaches_a_fixed_point() {
        let mut vass = VASS::new(2, vec![0, 1]);
        let q0 = vass.add_node(());
        let q1 = vass.add_node(());
        vass.add_edge(&q0, &q1, VASSEdge::new(0, vec![2, -1].into()));
        vass.add_edge(&q1, &q0, VASSEdge::new(1, vec![1, 0].into()));
        let instance = vass.init(vec![2, 1].into(), vec![1, 1].into(), q0, q1);

        let result = minimize(instance, |candidate| {
            candidate.dimension() >= 1 && candidate.transition_count() >= 1
        });

        assert_eq!(result.instance.dimension(), 1);
        assert_eq!(result.instance.state_count(), 1);
        assert_eq!(result.instance.transition_count(), 1);
        assert!(
            result
                .instance
                .vass
                .graph
                .edge_weights()
                .all(|edge| edge.update.iter().all(|value| *value == 0))
        );
        assert!(
            result
                .instance
                .initial_valuation
                .iter()
                .all(|value| *value == 0)
        );
        assert!(
            result
                .instance
                .final_valuation
                .iter()
                .all(|value| *value == 0)
        );
        assert!(
            result
                .reductions
                .iter()
                .any(|reduction| matches!(reduction, Reduction::MergeStates))
        );
    }
}
