use std::collections::VecDeque;

use hashbrown::HashSet;

use crate::{
    automaton::{
        AutomatonEdge, AutomatonNode, ExplicitEdgeAutomaton, FromLetter,
        vass::initialized::InitializedVASS,
    },
    config::ShortWitnessConfig,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct ShortWitness {
    pub depth: usize,
    pub explored_configurations: usize,
}

/// Searches a finite prefix of the concrete VASS configuration graph.
///
/// Finding the target is an exact reachability proof. Exhausting either limit
/// is inconclusive and leaves the complete solver to continue normally.
pub(super) fn find_short_witness<N, E>(
    instance: &InitializedVASS<N, E>,
    config: &ShortWitnessConfig,
) -> Option<ShortWitness>
where
    N: AutomatonNode,
    E: AutomatonEdge + FromLetter,
{
    if !*config.get_enabled() || *config.get_max_configurations() == 0 {
        return None;
    }

    let initial = (instance.initial_node, instance.initial_valuation.clone());
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    visited.insert(initial.clone());
    queue.push_back((initial, 0));

    while let Some(((node, valuation), depth)) = queue.pop_front() {
        if node == instance.final_node && valuation == instance.final_valuation {
            return Some(ShortWitness {
                depth,
                explored_configurations: visited.len(),
            });
        }

        if depth >= *config.get_max_depth() {
            continue;
        }

        for edge in instance.outgoing_edge_indices(&node) {
            let target = instance.edge_target_unchecked(&edge);
            let update = &instance.get_edge_unchecked(&edge).update;
            if !valuation.can_apply_update(update) {
                continue;
            }

            let mut next_valuation = valuation.clone();
            next_valuation.apply_update(update);
            let next = (target, next_valuation);
            if !visited.insert(next.clone()) {
                continue;
            }

            if visited.len() >= *config.get_max_configurations() {
                return None;
            }
            queue.push_back((next, depth + 1));
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::find_short_witness;
    use crate::{
        automaton::{
            ModifiableAutomaton,
            vass::{VASS, VASSEdge},
        },
        config::ShortWitnessConfig,
    };

    fn instance() -> super::InitializedVASS<(), usize> {
        let mut vass = VASS::new(1, (0..2).collect());
        let q0 = vass.add_node(());
        let q1 = vass.add_node(());
        vass.add_edge(&q0, &q0, VASSEdge::new(0, vec![1].into()));
        vass.add_edge(&q0, &q1, VASSEdge::new(1, vec![-2].into()));
        vass.init(vec![0].into(), vec![0].into(), q0, q1)
    }

    #[test]
    fn finds_short_exact_witness() {
        let witness = find_short_witness(&instance(), &ShortWitnessConfig::default()).unwrap();
        assert_eq!(witness.depth, 3);
    }

    #[test]
    fn depth_limit_is_inconclusive() {
        let config = ShortWitnessConfig::default().with_max_depth(2);
        assert_eq!(find_short_witness(&instance(), &config), None);
    }

    #[test]
    fn configuration_limit_is_inconclusive() {
        let config = ShortWitnessConfig::default().with_max_configurations(2);
        assert_eq!(find_short_witness(&instance(), &config), None);
    }
}
