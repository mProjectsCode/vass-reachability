use std::collections::{BTreeSet, HashMap, HashSet};

use super::{
    seeds::{DerivedSCCMetadata, StepTraceSeed, TraceStepSccViewSeed},
    state::{state_dot_id, state_dot_suffix, state_key},
    transitions::collect_component_transitions,
};
use crate::automaton::implicit_cfg_product::state::MultiGraphState;

pub fn derive_scc_metadata(seed: &super::seeds::SCCDagSeed) -> DerivedSCCMetadata {
    let component_sizes = seed
        .components
        .iter()
        .map(|component| component.nodes.len())
        .collect();
    let accepting_sizes = seed
        .components
        .iter()
        .map(|component| component.accepting_nodes.len())
        .collect();
    let cyclic_components = seed
        .components
        .iter()
        .map(|component| component.cyclic)
        .collect();

    DerivedSCCMetadata {
        component_sizes,
        accepting_sizes,
        cyclic_components,
    }
}

pub fn derive_scc_component_view(
    seed: &StepTraceSeed,
    component_index: usize,
) -> anyhow::Result<TraceStepSccViewSeed> {
    let (entries, exits) = collect_component_boundaries(seed, component_index)?;
    let dot = build_component_dot(seed, component_index, &entries, &exits)?;

    Ok(TraceStepSccViewSeed {
        component_index,
        dot,
        entries,
        exits,
    })
}

fn build_component_dot(
    seed: &StepTraceSeed,
    component_index: usize,
    entries: &[MultiGraphState],
    exits: &[MultiGraphState],
) -> anyhow::Result<String> {
    let component = seed
        .scc_dag
        .components
        .get(component_index)
        .ok_or_else(|| anyhow::anyhow!("invalid component index"))?;

    let mut states = component.nodes.clone();
    states.sort();
    states.dedup();
    let state_set = states.iter().cloned().collect::<HashSet<_>>();

    let accepting = component
        .accepting_nodes
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let entry_keys = entries.iter().map(state_key).collect::<HashSet<_>>();
    let exit_keys = exits.iter().map(state_key).collect::<HashSet<_>>();
    let (entry_connections, exit_connections) = collect_boundary_connections(seed, component_index);

    let transitions = collect_component_transitions(seed, component_index, &state_set)?;

    let mut dot = String::new();
    dot.push_str("digraph scc_component {\n");
    dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
    dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
    dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
    dot.push_str("rankdir=LR;\n");

    for state in states {
        let key = state_key(&state);
        let id = state_dot_id(&state);
        let mut attrs = vec![("label", format!("\"{}\"", key))];

        if accepting.contains(&state) {
            attrs.push(("shape", "doublecircle".to_string()));
        } else {
            attrs.push(("shape", "circle".to_string()));
        }

        if entry_keys.contains(&key) && exit_keys.contains(&key) {
            attrs.push(("penwidth", "2".to_string()));
            attrs.push(("color", "\"#9c27b0\"".to_string()));
        } else if entry_keys.contains(&key) {
            attrs.push(("penwidth", "2".to_string()));
            attrs.push(("color", "\"#4caf50\"".to_string()));
        } else if exit_keys.contains(&key) {
            attrs.push(("penwidth", "2".to_string()));
            attrs.push(("color", "\"#d8902a\"".to_string()));
        }

        dot.push_str(&format!(
            "{} [{}];\n",
            id,
            attrs
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }

    for entry in entries {
        let entry_id = format!("ENTRY_{}", state_dot_suffix(entry));
        let target_id = state_dot_id(entry);
        let label = format_boundary_label("entry", entry_connections.get(&state_key(entry)));
        dot.push_str(&format!(
            "{} [shape=point,label=\"\",color=\"#4caf50\"];\n",
            entry_id
        ));
        dot.push_str(&format!(
            "{} -> {} [style=dashed,color=\"#4caf50\",label=\"{}\"];\n",
            entry_id, target_id, label
        ));
    }

    for exit in exits {
        let source_id = state_dot_id(exit);
        let exit_id = format!("EXIT_{}", state_dot_suffix(exit));
        let label = format_boundary_label("exit", exit_connections.get(&state_key(exit)));
        dot.push_str(&format!(
            "{} [shape=point,label=\"\",color=\"#ff9800\"];\n",
            exit_id
        ));
        dot.push_str(&format!(
            "{} -> {} [style=dashed,color=\"#ff9800\",label=\"{}\"];\n",
            source_id, exit_id, label
        ));
    }

    for transition in transitions {
        dot.push_str(&format!(
            "{} -> {} [label=\"{}\"];\n",
            state_dot_id(&transition.source),
            state_dot_id(&transition.target),
            transition.update
        ));
    }

    dot.push_str("}\n");
    Ok(dot)
}

fn collect_component_boundaries(
    seed: &StepTraceSeed,
    component_index: usize,
) -> anyhow::Result<(Vec<MultiGraphState>, Vec<MultiGraphState>)> {
    let component = seed
        .scc_dag
        .components
        .get(component_index)
        .ok_or_else(|| anyhow::anyhow!("invalid component index"))?;

    let state_set = component
        .nodes
        .iter()
        .cloned()
        .collect::<HashSet<MultiGraphState>>();

    let mut entries = Vec::new();
    let mut entry_seen = HashSet::new();
    let mut exits = Vec::new();
    let mut exit_seen = HashSet::new();

    if component_index == seed.scc_dag.root_component
        && let Some(root_state) = seed.found_path.path.states.first()
        && state_set.contains(root_state)
        && entry_seen.insert(state_key(root_state))
    {
        entries.push(root_state.clone());
    }

    for (source_component, outgoing) in seed.scc_dag.edges.iter().enumerate() {
        for edge in outgoing {
            if edge.target_component == component_index
                && let Some(entry_state) = edge.path.states.last()
                && state_set.contains(entry_state)
                && entry_seen.insert(state_key(entry_state))
            {
                entries.push(entry_state.clone());
            }

            if source_component == component_index
                && let Some(exit_state) = edge.path.states.first()
                && state_set.contains(exit_state)
                && exit_seen.insert(state_key(exit_state))
            {
                exits.push(exit_state.clone());
            }
        }
    }

    entries.sort();
    exits.sort();
    Ok((entries, exits))
}

fn collect_boundary_connections(
    seed: &StepTraceSeed,
    component_index: usize,
) -> (
    HashMap<String, BTreeSet<usize>>,
    HashMap<String, BTreeSet<usize>>,
) {
    let mut entry_connections: HashMap<String, BTreeSet<usize>> = HashMap::new();
    let mut exit_connections: HashMap<String, BTreeSet<usize>> = HashMap::new();

    for (source_component, outgoing) in seed.scc_dag.edges.iter().enumerate() {
        for edge in outgoing {
            if edge.target_component == component_index
                && let Some(entry_state) = edge.path.states.last()
            {
                entry_connections
                    .entry(state_key(entry_state))
                    .or_default()
                    .insert(source_component);
            }

            if source_component == component_index
                && let Some(exit_state) = edge.path.states.first()
            {
                exit_connections
                    .entry(state_key(exit_state))
                    .or_default()
                    .insert(edge.target_component);
            }
        }
    }

    (entry_connections, exit_connections)
}

fn format_boundary_label(prefix: &str, connected_components: Option<&BTreeSet<usize>>) -> String {
    let Some(connected_components) = connected_components else {
        return prefix.to_string();
    };

    if connected_components.is_empty() {
        return prefix.to_string();
    }

    let sccs = connected_components
        .iter()
        .map(|component| format!("SCC {}", component))
        .collect::<Vec<_>>()
        .join(", ");

    format!("{} {}", prefix, sccs)
}
