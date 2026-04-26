use hashbrown::HashSet;
use itertools::Itertools;

use super::SCCDag;
use crate::automaton::{GIndex, Letter};

impl<NIndex: GIndex, L: Letter> SCCDag<NIndex, L> {
    pub fn to_graphviz(
        &self,
        highlight_components: Option<HashSet<usize>>,
        highlight_edges: Option<HashSet<(usize, usize)>>,
        collapse_parallel_edges: bool,
    ) -> String {
        let mut dot = String::new();
        dot.push_str("digraph finite_state_machine {\n");
        dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
        dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("rankdir=LR;\n");
        dot.push_str("node [shape=point,label=\"\"]START\n");

        let accepting_components = self
            .components
            .iter()
            .enumerate()
            .filter(|(_, scc)| !scc.accepting_nodes.is_empty())
            .map(|(index, _)| format!("SCC_{}", index))
            .collect_vec();

        dot.push_str(&format!(
            "node [shape = doublecircle]; {};\n",
            accepting_components.join(" ")
        ));
        dot.push_str("node [shape = circle];\n");
        dot.push_str(&format!("START -> SCC_{};\n", self.root_component));

        for (component_index, scc) in self.components.iter().enumerate() {
            let mut attrs = vec![(
                "label",
                format!(
                    "\"SCC {}\\nsize: {}{}\"",
                    component_index,
                    scc.nodes.len(),
                    if scc.cyclic { "\\ncyclic" } else { "" }
                ),
            )];

            if let Some(components) = &highlight_components
                && components.contains(&component_index)
            {
                attrs.push(("color", "red".to_string()));
            }

            dot.push_str(&format!(
                "SCC_{} [ {} ];\n",
                component_index,
                attrs.iter().map(|(k, v)| format!("{}={}", k, v)).join(" ")
            ));
        }

        for (source_component, outgoing) in self.edges.iter().enumerate() {
            let mut emitted_targets = HashSet::new();

            for (edge_index, edge) in outgoing.iter().enumerate() {
                if collapse_parallel_edges && !emitted_targets.insert(edge.target_component) {
                    continue;
                }

                let highlighted = if collapse_parallel_edges {
                    highlight_edges.as_ref().is_some_and(|edges| {
                        outgoing
                            .iter()
                            .enumerate()
                            .any(|(candidate_index, candidate)| {
                                candidate.target_component == edge.target_component
                                    && edges.contains(&(source_component, candidate_index))
                            })
                    })
                } else {
                    highlight_edges
                        .as_ref()
                        .is_some_and(|edges| edges.contains(&(source_component, edge_index)))
                };

                let mut attrs = Vec::new();
                if !collapse_parallel_edges {
                    attrs.push((
                        "label",
                        format!("\"{:?} ({})\"", edge.path.transitions, edge_index),
                    ));
                }
                if highlighted {
                    attrs.push(("color", "red".to_string()));
                }

                let attr_text = if attrs.is_empty() {
                    String::new()
                } else {
                    format!(
                        " [ {} ]",
                        attrs.iter().map(|(k, v)| format!("{}={}", k, v)).join(" ")
                    )
                };

                dot.push_str(&format!(
                    "SCC_{} -> SCC_{}{};\n",
                    source_component, edge.target_component, attr_text
                ));
            }
        }

        dot.push_str("}\n");

        dot
    }
}
