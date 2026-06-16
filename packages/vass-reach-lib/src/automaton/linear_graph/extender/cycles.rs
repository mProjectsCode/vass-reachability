use std::collections::VecDeque;

use hashbrown::HashSet;

use super::MultiGraphPath;
use crate::automaton::{
    Alphabet, TransitionSystem,
    cfg::update::CFGCounterUpdate,
    implicit_cfg_product::{state::MultiGraphState, view::ImplicitCFGProductView},
};

pub(super) fn preferred_rooted_cycle(
    product: &ImplicitCFGProductView<'_>,
    root: &MultiGraphState,
    allowed: &HashSet<MultiGraphState>,
    preferred: Option<&CFGCounterUpdate>,
) -> Option<MultiGraphPath> {
    let mut first_letters = product.alphabet().iter().collect::<Vec<_>>();
    first_letters.sort_by_key(|letter| preferred != Some(*letter));
    let mut fallback = None;

    for first in first_letters {
        let Some(target) = product.successor(root, first) else {
            continue;
        };
        if !allowed.contains(&target) {
            continue;
        }

        let mut first_path = MultiGraphPath::new(root.clone());
        first_path.add(*first, target.clone());
        let cycle = if &target == root {
            Some(first_path)
        } else {
            shortest_path_to_root(product, first_path, root, allowed)
        };

        let Some(cycle) = cycle else {
            continue;
        };
        let nonzero_effect = cycle
            .transitions
            .iter()
            .fold(vec![0i32; product.dimension()], |mut effect, update| {
                effect[update.counter().to_usize()] += update.op();
                effect
            })
            .into_iter()
            .any(|effect| effect != 0);

        if nonzero_effect {
            return Some(cycle);
        }
        fallback.get_or_insert(cycle);
    }

    fallback
}

fn shortest_path_to_root(
    product: &ImplicitCFGProductView<'_>,
    initial_path: MultiGraphPath,
    root: &MultiGraphState,
    allowed: &HashSet<MultiGraphState>,
) -> Option<MultiGraphPath> {
    let mut queue = VecDeque::from([initial_path.clone()]);
    let mut visited = HashSet::new();
    visited.insert(initial_path.end().clone());

    while let Some(path) = queue.pop_front() {
        for letter in product.alphabet() {
            let Some(target) = product.successor(path.end(), letter) else {
                continue;
            };
            if !allowed.contains(&target) {
                continue;
            }

            let mut next = path.clone();
            next.add(*letter, target.clone());
            if &target == root {
                return Some(next);
            }

            if visited.insert(target) {
                queue.push_back(next);
            }
        }
    }

    None
}
