use std::collections::HashSet;

use super::seeds::StepTraceSeed;
use crate::automaton::{
    cfg::update::CFGCounterUpdate, implicit_cfg_product::state::MultiGraphState,
};

#[derive(Debug, Clone)]
pub(super) struct SccTransitionSeed {
    pub source: MultiGraphState,
    pub target: MultiGraphState,
    pub update: CFGCounterUpdate,
}

pub(super) fn collect_component_transitions(
    seed: &StepTraceSeed,
    component_index: usize,
    state_set: &HashSet<MultiGraphState>,
) -> anyhow::Result<Vec<SccTransitionSeed>> {
    if let Some(component) = seed.scc_dag.components.get(component_index)
        && !component.internal_edges.is_empty()
    {
        return component
            .internal_edges
            .iter()
            .map(|edge| {
                Ok(SccTransitionSeed {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    update: edge.transition,
                })
            })
            .collect::<anyhow::Result<Vec<_>>>();
    }

    let fallback = collect_internal_component_transitions(seed, state_set)?;
    Ok(fallback)
}

pub(super) fn collect_internal_component_transitions(
    seed: &StepTraceSeed,
    state_set: &HashSet<MultiGraphState>,
) -> anyhow::Result<Vec<SccTransitionSeed>> {
    let mut transitions = Vec::new();

    collect_internal_transitions_from_path(&seed.found_path.path, state_set, &mut transitions)?;
    for outgoing in &seed.scc_dag.edges {
        for edge in outgoing {
            collect_internal_transitions_from_path(&edge.path, state_set, &mut transitions)?;
        }
    }

    Ok(transitions)
}

fn collect_internal_transitions_from_path(
    path: &super::seeds::PathSeed,
    state_set: &HashSet<MultiGraphState>,
    output: &mut Vec<SccTransitionSeed>,
) -> anyhow::Result<()> {
    let edge_count = path
        .transitions
        .len()
        .min(path.states.len().saturating_sub(1));
    for i in 0..edge_count {
        let source = path.states[i].clone();
        let target = path.states[i + 1].clone();
        if !state_set.contains(&source) || !state_set.contains(&target) {
            continue;
        }

        let update = path.transitions[i];
        output.push(SccTransitionSeed {
            source,
            target,
            update,
        });
    }

    Ok(())
}
