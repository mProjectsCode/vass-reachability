use std::collections::HashMap;

use super::{
    seeds::{
        SccCounterEffectRepresentativeSeed, SccCycleCounterEffectSeed, StepTraceSeed,
        TraceStepSccCounterEffectSetSeed,
    },
    transitions::collect_component_transitions,
};
use crate::automaton::{
    cfg::update::CFGCounterUpdate, implicit_cfg_product::state::MultiGraphState,
};

const MAX_ROOTED_BASIC_CYCLES: usize = 512;
const MAX_ROOTED_CYCLE_EXPANSIONS: usize = 50_000;

pub fn derive_scc_counter_effect_set(
    seed: &StepTraceSeed,
    component_index: usize,
    entry_state: &MultiGraphState,
    start_value: i64,
) -> anyhow::Result<TraceStepSccCounterEffectSetSeed> {
    let component = seed
        .scc_dag
        .components
        .get(component_index)
        .ok_or_else(|| anyhow::anyhow!("invalid component index"))?;

    let mut states = component.nodes.clone();
    states.sort();
    states.dedup();
    let state_set = states
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();

    if !state_set.contains(entry_state) {
        anyhow::bail!("entry state does not belong to selected component");
    }

    let transitions = collect_component_transitions(seed, component_index, &state_set)?;
    let dimension = infer_counter_dimension(seed, &transitions);

    let mut state_to_index = HashMap::new();
    for (idx, state) in states.iter().enumerate() {
        state_to_index.insert(state.clone(), idx);
    }

    let start_index = *state_to_index
        .get(entry_state)
        .ok_or_else(|| anyhow::anyhow!("entry state does not belong to selected component"))?;

    let mut adjacency = vec![Vec::<SccTransitionArc>::new(); states.len()];
    for transition in transitions {
        let Some(&source_idx) = state_to_index.get(&transition.source) else {
            continue;
        };
        let Some(&target_idx) = state_to_index.get(&transition.target) else {
            continue;
        };

        adjacency[source_idx].push(SccTransitionArc {
            target: target_idx,
            update: transition.update,
        });
    }

    let rooted_cycles = enumerate_rooted_basic_cycles(
        &adjacency,
        start_index,
        MAX_ROOTED_BASIC_CYCLES,
        MAX_ROOTED_CYCLE_EXPANSIONS,
    );

    let mut basic_cycles = Vec::with_capacity(rooted_cycles.cycles.len());
    for cycle in rooted_cycles.cycles {
        let mut cycle_states = Vec::with_capacity(cycle.len() + 1);
        cycle_states.push(states[start_index].clone());

        let mut cycle_transitions = Vec::with_capacity(cycle.len());
        let mut counters = vec![start_value; dimension];
        let mut positive_valid = true;

        for edge in cycle {
            cycle_transitions.push(edge.update);
            let counter = edge.update.counter().to_usize();
            if counter < counters.len() {
                let delta = edge.update.op_i64();
                counters[counter] += delta;
                if counters[counter] < 0 {
                    positive_valid = false;
                    break;
                }
            }

            cycle_states.push(states[edge.target].clone());
        }

        if !positive_valid {
            continue;
        }

        // Keep the reported value as a counter effect (delta), not as absolute
        // counters.
        let effect = counters
            .iter()
            .map(|counter| counter - start_value)
            .collect::<Vec<_>>();

        basic_cycles.push(SccCycleCounterEffectSeed {
            states: cycle_states,
            transitions: cycle_transitions,
            effect,
        });
    }

    let mut effect_set = Vec::<SccCounterEffectRepresentativeSeed>::new();
    for cycle in &basic_cycles {
        let candidate = SccCounterEffectRepresentativeSeed {
            effect: cycle.effect.clone(),
            sample_cycle: cycle.clone(),
        };

        let mut discard_candidate = false;
        for existing in &effect_set {
            if let Some(scale) =
                positive_integer_multiple_scale(&candidate.effect, &existing.effect)
                && scale >= 1
            {
                discard_candidate = true;
                break;
            }
        }

        if discard_candidate {
            continue;
        }

        effect_set.retain(|existing| {
            match positive_integer_multiple_scale(&existing.effect, &candidate.effect) {
                Some(scale) => scale <= 1,
                None => true,
            }
        });

        effect_set.push(candidate);
    }
    effect_set.sort_by(|left, right| left.effect.cmp(&right.effect));

    Ok(TraceStepSccCounterEffectSetSeed {
        component_index,
        entry: entry_state.clone(),
        start_value,
        dimension,
        total_cycles: basic_cycles.len(),
        capped: rooted_cycles.capped,
        basic_cycles,
        effect_set,
    })
}

#[derive(Debug, Clone)]
struct SccTransitionArc {
    target: usize,
    update: CFGCounterUpdate,
}

#[derive(Debug)]
struct RootedCycleEnumeration {
    cycles: Vec<Vec<SccTransitionArc>>,
    capped: bool,
}

fn positive_integer_multiple_scale(candidate: &[i64], base: &[i64]) -> Option<i64> {
    if candidate.len() != base.len() {
        return None;
    }

    let candidate_all_zero = candidate.iter().all(|value| *value == 0);
    let base_all_zero = base.iter().all(|value| *value == 0);
    if candidate_all_zero && base_all_zero {
        return Some(1);
    }
    if candidate_all_zero || base_all_zero {
        return None;
    }

    let mut scale: Option<i64> = None;
    for (&cand, &base_value) in candidate.iter().zip(base.iter()) {
        if base_value == 0 {
            if cand != 0 {
                return None;
            }
            continue;
        }

        if cand % base_value != 0 {
            return None;
        }

        let ratio = cand / base_value;
        if ratio <= 0 {
            return None;
        }

        if let Some(existing) = scale {
            if existing != ratio {
                return None;
            }
        } else {
            scale = Some(ratio);
        }
    }

    scale
}

fn enumerate_rooted_basic_cycles(
    adjacency: &[Vec<SccTransitionArc>],
    root: usize,
    max_cycles: usize,
    max_expansions: usize,
) -> RootedCycleEnumeration {
    struct DfsState<'a> {
        adjacency: &'a [Vec<SccTransitionArc>],
        visited: &'a mut [bool],
        path: &'a mut Vec<SccTransitionArc>,
        cycles: &'a mut Vec<Vec<SccTransitionArc>>,
        expansions: &'a mut usize,
        max_cycles: usize,
        max_expansions: usize,
        capped: &'a mut bool,
    }

    impl<'a> DfsState<'a> {
        fn dfs(&mut self, current: usize, root: usize) {
            if *self.expansions >= self.max_expansions || self.cycles.len() >= self.max_cycles {
                *self.capped = true;
                return;
            }

            *self.expansions += 1;

            for edge in &self.adjacency[current] {
                if *self.expansions >= self.max_expansions || self.cycles.len() >= self.max_cycles {
                    *self.capped = true;
                    return;
                }

                if edge.target == root {
                    self.path.push(edge.clone());
                    self.cycles.push(self.path.clone());
                    self.path.pop();
                    continue;
                }

                if self.visited[edge.target] {
                    continue;
                }

                self.visited[edge.target] = true;
                self.path.push(edge.clone());
                self.dfs(edge.target, root);
                self.path.pop();
                self.visited[edge.target] = false;

                if *self.capped {
                    return;
                }
            }
        }
    }

    let mut visited = vec![false; adjacency.len()];
    visited[root] = true;

    let mut path = Vec::<SccTransitionArc>::new();
    let mut cycles = Vec::<Vec<SccTransitionArc>>::new();
    let mut expansions = 0_usize;
    let mut capped = false;

    if root < adjacency.len() {
        let mut dfs_state = DfsState {
            adjacency,
            visited: &mut visited,
            path: &mut path,
            cycles: &mut cycles,
            expansions: &mut expansions,
            max_cycles,
            max_expansions,
            capped: &mut capped,
        };
        dfs_state.dfs(root, root);
    }

    RootedCycleEnumeration { cycles, capped }
}

fn infer_counter_dimension(
    seed: &StepTraceSeed,
    transitions: &[super::transitions::SccTransitionSeed],
) -> usize {
    let from_initial = seed.initial_valuation.as_ref().map_or(0, |v| v.dimension());
    let from_transitions = transitions
        .iter()
        .map(|transition| transition.update.counter().to_usize() + 1)
        .max()
        .unwrap_or(0);

    from_initial.max(from_transitions)
}
