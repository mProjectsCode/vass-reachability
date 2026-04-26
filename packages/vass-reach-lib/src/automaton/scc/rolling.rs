use super::{SCCDag, SCCDagEdge, sort_and_dedup_component_edges};
use crate::automaton::{GIndex, Letter};

pub(super) fn is_non_root_non_accepting_trivial_component<NIndex: GIndex, L: Letter>(
    dag: &SCCDag<NIndex, L>,
    component: usize,
) -> bool {
    let scc = &dag.components[component];
    component != dag.root_component && scc.is_trivial() && scc.accepting_nodes.is_empty()
}

fn bypass_trivial_component_once<NIndex: GIndex, L: Letter>(
    dag: &mut SCCDag<NIndex, L>,
    component: usize,
) -> bool {
    let outgoing = dag.edges[component].clone();
    let mut has_incoming = false;
    let mut changed = false;

    for source in 0..dag.edges.len() {
        if source == component {
            continue;
        }

        let mut next_edges = Vec::new();
        let mut rewrote_from_source = false;

        for edge in dag.edges[source].iter().cloned() {
            if edge.target_component != component {
                next_edges.push(edge);
                continue;
            }

            has_incoming = true;
            rewrote_from_source = true;

            // Bridge every incoming edge through each outgoing edge.
            // This preserves reachability while eliminating the trivial
            // intermediate SCC from explicit traversal.
            for out in &outgoing {
                let mut path = edge.path.clone();
                path.concat(out.path.clone());
                next_edges.push(SCCDagEdge {
                    path,
                    target_component: out.target_component,
                });
            }
        }

        if rewrote_from_source {
            sort_and_dedup_component_edges(&mut next_edges);
            dag.edges[source] = next_edges;
            changed = true;
        }
    }

    if has_incoming {
        dag.edges[component].clear();
    }

    changed
}

pub(super) fn roll_trivial_paths_in_place<NIndex: GIndex, L: Letter>(dag: &mut SCCDag<NIndex, L>) {
    loop {
        let mut changed = false;

        for component in 0..dag.components.len() {
            if !is_non_root_non_accepting_trivial_component(dag, component) {
                continue;
            }

            if bypass_trivial_component_once(dag, component) {
                changed = true;
            }
        }

        if !changed {
            break;
        }
    }
}

pub(super) fn compact_removed_trivial_components_in_place<NIndex: GIndex, L: Letter>(
    dag: &mut SCCDag<NIndex, L>,
) {
    // Remove non-root, non-accepting trivial SCCs entirely and remap
    // component indices to keep the DAG compact.
    let removed_components = dag
        .components
        .iter()
        .enumerate()
        .map(|(component, _)| is_non_root_non_accepting_trivial_component(dag, component))
        .collect::<Vec<_>>();

    if !removed_components.iter().any(|removed| *removed) {
        return;
    }

    let mut remap = vec![None; dag.components.len()];
    let mut compacted_components = Vec::new();

    for (old_component, scc) in dag.components.iter().enumerate() {
        if removed_components[old_component] {
            continue;
        }

        remap[old_component] = Some(compacted_components.len());
        compacted_components.push(scc.clone());
    }

    let mut compacted_edges = vec![Vec::new(); compacted_components.len()];

    for old_source in 0..dag.edges.len() {
        let Some(new_source) = remap[old_source] else {
            continue;
        };

        for edge in &dag.edges[old_source] {
            let Some(new_target) = remap[edge.target_component] else {
                continue;
            };

            compacted_edges[new_source].push(SCCDagEdge {
                path: edge.path.clone(),
                target_component: new_target,
            });
        }

        sort_and_dedup_component_edges(&mut compacted_edges[new_source]);
    }

    dag.root_component = remap[dag.root_component]
        .expect("Root SCC must never be removed during trivial SCC compaction");
    dag.components = compacted_components;
    dag.edges = compacted_edges;
}
