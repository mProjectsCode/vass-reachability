use itertools::Itertools;

use crate::automaton::{GIndex, InitializedAutomaton, index_map::IndexSet};

pub trait AutomatonAlgorithms: InitializedAutomaton {
    /// Find the SCC surrounding a given node. Returns a vector of all the nodes
    /// that are part of the SCC.
    fn find_scc_surrounding(&self, node: Self::NIndex) -> Vec<Self::NIndex> {
        let mut stack = vec![];
        let mut current_path = vec![];
        let mut scc = IndexSet::new(self.node_count());
        let mut visited = IndexSet::new(self.node_count());

        stack.push(node);
        current_path.push(node);
        scc.insert(node);

        while let Some(&current) = stack.last() {
            if !visited.contains(current) {
                visited.insert(current);
            }

            let mut found_unvisited = false;
            for edge in self.outgoing_edge_indices(current) {
                let successor = self.edge_target_unchecked(edge);

                if !visited.contains(successor) {
                    stack.push(successor);
                    current_path.push(successor);
                    found_unvisited = true;
                    break;
                } else if scc.contains(successor) {
                    for node in &current_path {
                        scc.insert(*node);
                    }
                }
            }

            if !found_unvisited {
                stack.pop();
                if !current_path.is_empty() && current_path.last() == Some(&current) {
                    current_path.pop();
                }
            }
        }

        scc.to_vec()
    }

    fn to_graphviz(
        &self,
        nodes: Option<Vec<Self::NIndex>>,
        edges: Option<Vec<Self::EIndex>>,
    ) -> String {
        let mut dot = String::new();
        dot.push_str("digraph finite_state_machine {\n");
        dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
        dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");
        dot.push_str("rankdir=LR;\n");
        dot.push_str("node [shape=point,label=\"\"]START\n");

        let accepting_states = self
            .iter_node_indices()
            .filter(|node| self.is_accepting(*node))
            .collect::<Vec<_>>();

        dot.push_str(&format!(
            "node [shape = doublecircle]; {};\n",
            accepting_states
                .iter()
                .map(|node| format!("{:?}", node.index()))
                .join(" ")
        ));
        dot.push_str("node [shape = circle];\n");

        if let Some(nodes) = nodes {
            for node in nodes {
                dot.push_str(&format!("{:?} [color = red]\n", node.index()));
            }
        }

        let start = self.get_initial();
        dot.push_str(&format!("START -> {:?};\n", start.index()));

        for (edge, data) in self.iter_edges() {
            let mut attrs = vec![("label", format!("\"{:?} ({:?})\"", data, edge.index()))];

            if let Some(edges) = &edges
                && edges.contains(&edge)
            {
                attrs.push(("color", "red".to_string()));
            }

            let source = self.edge_source_unchecked(edge);
            let target = self.edge_target_unchecked(edge);

            dot.push_str(&format!(
                "{:?} -> {:?} [ {} ];\n",
                source.index(),
                target.index(),
                attrs.iter().map(|(k, v)| format!("{}={}", k, v)).join(" ")
            ));
        }

        dot.push_str("}\n");

        dot
    }
}

impl<T: InitializedAutomaton> AutomatonAlgorithms for T {}
