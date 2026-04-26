use std::collections::VecDeque;

use petgraph::graph::NodeIndex;

use crate::automaton::{
    cfg::{ExplicitEdgeCFG, update::CFGCounterUpdate},
    vass::{counter::VASSCounterValuation, omega::OmegaCounterValuation},
};

/// A node in a Karp-Miller coverability tree over a CFG control graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KarpMillerTreeNode {
    /// Control location in the CFG.
    pub control: NodeIndex,
    /// Coverability valuation at this node.
    pub valuation: OmegaCounterValuation,
    /// Parent node index in the tree, if present.
    pub parent: Option<usize>,
    /// Edge label used from parent to this node.
    pub incoming: Option<CFGCounterUpdate>,
    /// Child indices in the tree.
    pub children: Vec<usize>,
    /// Closed nodes are not expanded further.
    pub closed: bool,
}

/// Full Karp-Miller coverability tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KarpMillerCoverabilityTree {
    root: usize,
    nodes: Vec<KarpMillerTreeNode>,
}

impl KarpMillerCoverabilityTree {
    pub fn root(&self) -> usize {
        self.root
    }

    pub fn nodes(&self) -> &[KarpMillerTreeNode] {
        &self.nodes
    }

    pub fn node(&self, index: usize) -> &KarpMillerTreeNode {
        &self.nodes[index]
    }

    pub fn to_graphviz(&self) -> String {
        let mut dot = String::new();
        dot.push_str("digraph karp_miller_tree {\n");
        dot.push_str("fontname=\"Helvetica,Arial,sans-serif\"\n");
        dot.push_str("node [fontname=\"Helvetica,Arial,sans-serif\", shape=box]\n");
        dot.push_str("edge [fontname=\"Helvetica,Arial,sans-serif\"]\n");

        for (index, node) in self.nodes.iter().enumerate() {
            let status = if node.closed { "closed" } else { "open" };
            let mut attrs = vec![(
                "label",
                format!(
                    "\"#{}\\nq={}\\n{}\\n{}\"",
                    index,
                    node.control.index(),
                    node.valuation,
                    status
                ),
            )];

            if index == self.root {
                attrs.push(("shape", "doubleoctagon".to_string()));
            }

            dot.push_str(&format!(
                "n{} [{}];\n",
                index,
                attrs
                    .into_iter()
                    .map(|(k, v)| format!("{}={}", k, v))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }

        for (parent_index, parent) in self.nodes.iter().enumerate() {
            for child in parent.children.iter().copied() {
                let edge_label = self.nodes[child]
                    .incoming
                    .map(|u| u.to_string())
                    .unwrap_or_default();
                dot.push_str(&format!(
                    "n{} -> n{} [label=\"{}\"];\n",
                    parent_index, child, edge_label
                ));
            }
        }

        dot.push_str("}\n");
        dot
    }
}

/// Build the Karp-Miller coverability tree for a CFG-based transition system.
///
/// Configurations are `(control, valuation)` where `control` is a CFG node and
/// `valuation` is in `N^d`. The resulting tree labels valuations in
/// `N^d U {omega}^d`.
pub fn build_karp_miller_coverability_tree<C: ExplicitEdgeCFG>(
    cfg: &C,
    initial_valuation: &VASSCounterValuation,
) -> KarpMillerCoverabilityTree {
    assert_eq!(
        initial_valuation.dimension(),
        cfg.alphabet().len() / 2,
        "Initial valuation dimension must match CFG counter dimension",
    );

    let root = KarpMillerTreeNode {
        control: cfg.get_initial(),
        valuation: OmegaCounterValuation::from_finite(initial_valuation),
        parent: None,
        incoming: None,
        children: Vec::new(),
        closed: false,
    };

    let mut tree = KarpMillerCoverabilityTree {
        root: 0,
        nodes: vec![root],
    };

    let mut queue = VecDeque::new();
    queue.push_back(0usize);

    while let Some(current_index) = queue.pop_front() {
        if tree.nodes[current_index].closed {
            continue;
        }

        let control = tree.nodes[current_index].control;
        let valuation = tree.nodes[current_index].valuation.clone();

        let outgoing = cfg.outgoing_edge_indices(&control).collect::<Vec<_>>();

        for edge in outgoing {
            let update = *cfg.get_edge_unchecked(&edge);
            if !valuation.can_apply_cfg_update(update) {
                continue;
            }

            let target = cfg.edge_target_unchecked(&edge);
            let mut next_valuation = valuation.clone();
            next_valuation.apply_cfg_update(update);

            for ancestor in iter_ancestors(current_index, &tree.nodes) {
                let ancestor_node = &tree.nodes[ancestor];
                if ancestor_node.control == target && ancestor_node.valuation.leq(&next_valuation) {
                    next_valuation.accelerate_with(&ancestor_node.valuation);
                }
            }

            let closed = iter_ancestors(current_index, &tree.nodes).any(|ancestor| {
                let ancestor_node = &tree.nodes[ancestor];
                ancestor_node.control == target && ancestor_node.valuation.leq(&next_valuation)
            });

            let child_index = tree.nodes.len();
            tree.nodes.push(KarpMillerTreeNode {
                control: target,
                valuation: next_valuation,
                parent: Some(current_index),
                incoming: Some(update),
                children: Vec::new(),
                closed,
            });
            tree.nodes[current_index].children.push(child_index);

            if !closed {
                queue.push_back(child_index);
            }
        }
    }

    tree
}

fn iter_ancestors(
    node_index: usize,
    nodes: &[KarpMillerTreeNode],
) -> impl Iterator<Item = usize> + '_ {
    let mut ancestors = Vec::new();
    let mut current = Some(node_index);
    while let Some(index) = current {
        ancestors.push(index);
        current = nodes[index].parent;
    }
    ancestors.into_iter()
}
