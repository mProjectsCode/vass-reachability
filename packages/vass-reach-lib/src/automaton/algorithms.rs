use hashbrown::HashSet;
use itertools::Itertools;

pub use crate::automaton::scc::{SCC, SCCAlgorithms, SCCDag, SCCDagEdge};
use crate::automaton::{
    AutomatonIterators, CompactGIndex, ExplicitEdgeAutomaton, InitializedAutomaton,
    TransitionSystemType,
};

pub trait EdgeAutomatonAlgorithms<Type: TransitionSystemType<Self::NIndex>>:
    ExplicitEdgeAutomaton<Type> + InitializedAutomaton<Type> + AutomatonIterators<Type>
where
    Self::NIndex: CompactGIndex,
{
    fn to_graphviz(
        &self,
        highlight_nodes: Option<HashSet<Self::NIndex>>,
        highlight_edges: Option<HashSet<Self::EIndex>>,
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
            .filter(|node| self.is_accepting(node))
            .collect::<Vec<_>>();

        dot.push_str(&format!(
            "node [shape = doublecircle]; {};\n",
            accepting_states
                .iter()
                .map(|node| format!("{:?}", node.index()))
                .join(" ")
        ));
        dot.push_str("node [shape = circle];\n");

        let start = self.get_initial();
        dot.push_str(&format!("START -> {:?};\n", start.index()));

        for (node, _) in self.iter_nodes() {
            let mut attrs = vec![("label", format!("\"{:?}\"", node.index()))];

            if let Some(nodes) = &highlight_nodes
                && nodes.contains(&node)
            {
                attrs.push(("color", "red".to_string()));
            }

            dot.push_str(&format!(
                "{:?} [ {} ];\n",
                node.index(),
                attrs.iter().map(|(k, v)| format!("{}={}", k, v)).join(" ")
            ));
        }

        for (edge, data) in self.iter_edges() {
            let mut attrs = vec![("label", format!("\"{:?} ({:?})\"", data, edge.index()))];

            if let Some(edges) = &highlight_edges
                && edges.contains(&edge)
            {
                attrs.push(("color", "red".to_string()));
            }

            let source = self.edge_source_unchecked(&edge);
            let target = self.edge_target_unchecked(&edge);

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

impl<
    Type: TransitionSystemType<Self::NIndex>,
    T: ExplicitEdgeAutomaton<Type> + InitializedAutomaton<Type> + AutomatonIterators<Type>,
> EdgeAutomatonAlgorithms<Type> for T
where
    Self::NIndex: CompactGIndex,
{
}
