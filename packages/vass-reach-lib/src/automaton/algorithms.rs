use std::cmp::Ordering;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;

use crate::automaton::{
    AutomatonIterators, CompactGIndex, Deterministic, ExplicitEdgeAutomaton, GIndex,
    InitializedAutomaton, Letter, TransitionSystemType, path::Path,
};

pub trait AutomatonAlgorithms<Type: TransitionSystemType<Self::NIndex>>:
    InitializedAutomaton<Type>
{
    /// Find the SCC surrounding a given node. Returns a vector of all the nodes
    /// that are part of the SCC.
    fn find_scc_surrounding(&self, node: Self::NIndex) -> Vec<Self::NIndex> {
        let forward = reachable_from(&node, |current| self.successors(current).collect_vec());
        let backward = reachable_from(&node, |current| self.predecessors(current).collect_vec());

        let mut scc = forward
            .into_iter()
            .filter(|candidate| backward.contains(candidate))
            .collect_vec();
        scc.sort();
        scc
    }

    /// Builds the rooted SCC tree of the accepting part of the graph.
    ///
    /// Only nodes that are reachable from the initial node and can still reach
    /// an accepting node are kept. SCCs are duplicated along different
    /// root-to-leaf branches so the result is a tree, not a DAG.
    fn find_scc_tree(&self) -> SCCTree<Self::NIndex, Self::Letter>
    where
        Type: TransitionSystemType<Self::NIndex, SuccessorType = Option<Self::NIndex>>,
        Self::Letter: Letter,
        Self: Sized,
    {
        let initial = self.get_initial();
        let reachable = reachable_from(&initial, |current| self.successors(current).collect_vec());

        self.find_scc_tree_in_subgraph(initial, &reachable, |node| self.is_accepting(node))
    }

    fn find_scc_tree_in_subgraph<F>(
        &self,
        initial: Self::NIndex,
        allowed: &HashSet<Self::NIndex>,
        is_accepting: F,
    ) -> SCCTree<Self::NIndex, Self::Letter>
    where
        Type: TransitionSystemType<Self::NIndex, SuccessorType = Option<Self::NIndex>>,
        Self::Letter: Letter,
        Self: Sized,
        F: Fn(&Self::NIndex) -> bool,
    {
        let relevant = collect_relevant_scc_nodes_in_subgraph::<Self, Type, F>(
            self,
            &initial,
            allowed,
            &is_accepting,
        );
        let (components, component_of_node) =
            compute_sccs::<Self, Type, F>(self, &relevant, &is_accepting);
        let component_edges =
            collect_component_edges::<Self, Type>(self, &relevant, &components, &component_of_node);

        let root_component = component_of_node[&initial];
        build_scc_tree(root_component, &components, &component_edges)
    }
}

impl<T: InitializedAutomaton<Deterministic>> AutomatonAlgorithms<Deterministic> for T {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCC<NIndex: GIndex> {
    pub nodes: Vec<NIndex>,
    pub accepting_nodes: Vec<NIndex>,
    pub cyclic: bool,
}

impl<NIndex: GIndex> SCC<NIndex> {
    pub fn is_trivial(&self) -> bool {
        self.nodes.len() == 1 && !self.cyclic
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCCTree<NIndex: GIndex, L: Letter> {
    pub scc: SCC<NIndex>,
    pub children: Vec<SCCTreeEdge<NIndex, L>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCCTreeEdge<NIndex: GIndex, L: Letter> {
    pub path: Path<NIndex, L>,
    pub child: Box<SCCTree<NIndex, L>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SCCTreeEdgeSpec<NIndex: GIndex, L: Letter> {
    path: Path<NIndex, L>,
    target_component: usize,
}

type SCCComponents<NIndex> = (Vec<SCC<NIndex>>, HashMap<NIndex, usize>);

fn collect_relevant_scc_nodes_in_subgraph<A, Type, F>(
    automaton: &A,
    initial: &A::NIndex,
    allowed: &HashSet<A::NIndex>,
    is_accepting: &F,
) -> HashSet<A::NIndex>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let reachable = reachable_from(initial, |current| {
        sorted_neighbors(automaton.successors(current), allowed)
    });
    let accepting = reachable
        .iter()
        .filter(|node| is_accepting(node))
        .cloned()
        .collect_vec();

    assert!(
        !accepting.is_empty(),
        "Cannot build SCC tree for a graph without reachable accepting nodes"
    );

    let mut relevant = HashSet::new();
    for node in &accepting {
        relevant.extend(reachable_from(node, |current| {
            sorted_neighbors(automaton.predecessors(current), &reachable)
        }));
    }

    relevant
}

fn compute_sccs<A, Type, F>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
    is_accepting: &F,
) -> SCCComponents<A::NIndex>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let finish_order = compute_finish_order::<A, Type>(automaton, relevant);
    collect_components_from_finish_order::<A, Type, F>(
        automaton,
        relevant,
        finish_order,
        is_accepting,
    )
}

fn compute_finish_order<A, Type>(automaton: &A, relevant: &HashSet<A::NIndex>) -> Vec<A::NIndex>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
{
    let mut relevant_nodes = relevant.iter().cloned().collect_vec();
    relevant_nodes.sort();

    let mut visited = HashSet::new();
    let mut finish_order = Vec::with_capacity(relevant_nodes.len());

    for node in relevant_nodes {
        if visited.contains(&node) {
            continue;
        }

        let mut stack = vec![(node, false)];
        while let Some((current, expanded)) = stack.pop() {
            if expanded {
                finish_order.push(current);
                continue;
            }

            if !visited.insert(current.clone()) {
                continue;
            }

            stack.push((current.clone(), true));

            for successor in sorted_neighbors(automaton.successors(&current), relevant)
                .into_iter()
                .rev()
            {
                if !visited.contains(&successor) {
                    stack.push((successor, false));
                }
            }
        }
    }

    finish_order
}

fn collect_components_from_finish_order<A, Type, F>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
    mut finish_order: Vec<A::NIndex>,
    is_accepting: &F,
) -> SCCComponents<A::NIndex>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let mut assigned = HashSet::new();
    let mut components = Vec::new();
    let mut component_of_node = HashMap::new();

    while let Some(node) = finish_order.pop() {
        if assigned.contains(&node) {
            continue;
        }

        let mut component_nodes = Vec::new();
        let mut stack = vec![node.clone()];
        assigned.insert(node.clone());

        while let Some(current) = stack.pop() {
            component_nodes.push(current.clone());

            for predecessor in sorted_neighbors(automaton.predecessors(&current), relevant)
                .into_iter()
                .rev()
            {
                if assigned.insert(predecessor.clone()) {
                    stack.push(predecessor);
                }
            }
        }

        component_nodes.sort();

        let component_index = components.len();
        for component_node in &component_nodes {
            component_of_node.insert(component_node.clone(), component_index);
        }

        components.push(build_scc::<A, Type, F>(
            automaton,
            component_nodes,
            is_accepting,
        ));
    }

    (components, component_of_node)
}

fn build_scc<A, Type, F>(automaton: &A, nodes: Vec<A::NIndex>, is_accepting: &F) -> SCC<A::NIndex>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let cyclic = nodes.len() > 1
        || automaton
            .alphabet()
            .iter()
            .any(|letter| automaton.successor(&nodes[0], letter) == Some(nodes[0].clone()));

    let accepting_nodes = nodes
        .iter()
        .filter(|node| is_accepting(node))
        .cloned()
        .collect_vec();

    SCC {
        nodes,
        accepting_nodes,
        cyclic,
    }
}

fn collect_component_edges<A, Type>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
    components: &[SCC<A::NIndex>],
    component_of_node: &HashMap<A::NIndex, usize>,
) -> Vec<Vec<SCCTreeEdgeSpec<A::NIndex, A::Letter>>>
where
    Type: TransitionSystemType<A::NIndex, SuccessorType = Option<A::NIndex>>,
    A: InitializedAutomaton<Type> + ?Sized,
    A::Letter: Letter,
{
    let mut component_edges = vec![Vec::new(); components.len()];

    for (component_index, component) in components.iter().enumerate() {
        for node in &component.nodes {
            for letter in automaton.alphabet() {
                let Some(successor) = automaton.successor(node, letter) else {
                    continue;
                };

                if !relevant.contains(&successor) {
                    continue;
                }

                let target_component = component_of_node[&successor];
                if target_component == component_index {
                    continue;
                }

                let mut path = Path::new(node.clone());
                path.add(letter.clone(), successor);

                component_edges[component_index].push(SCCTreeEdgeSpec {
                    path,
                    target_component,
                });
            }
        }

        sort_and_dedup_component_edges(&mut component_edges[component_index]);
    }

    component_edges
}

fn sort_and_dedup_component_edges<NIndex: GIndex, L: Letter>(
    edges: &mut Vec<SCCTreeEdgeSpec<NIndex, L>>,
) {
    edges.sort_by(|left, right| {
        compare_paths(&left.path, &right.path)
            .then(left.target_component.cmp(&right.target_component))
    });
    edges.dedup_by(|left, right| {
        left.target_component == right.target_component && left.path == right.path
    });
}

fn reachable_from<NIndex, I>(start: &NIndex, mut next: impl FnMut(&NIndex) -> I) -> HashSet<NIndex>
where
    NIndex: Clone + Eq + std::hash::Hash + Ord,
    I: IntoIterator<Item = NIndex>,
{
    let mut visited = HashSet::new();
    let mut stack = vec![start.clone()];

    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        let mut successors = next(&current).into_iter().collect_vec();
        successors.sort();
        successors.dedup();

        for successor in successors.into_iter().rev() {
            if !visited.contains(&successor) {
                stack.push(successor);
            }
        }
    }

    visited
}

fn sorted_neighbors<NIndex: GIndex>(
    neighbors: impl IntoIterator<Item = NIndex>,
    relevant: &HashSet<NIndex>,
) -> Vec<NIndex> {
    let mut neighbors = neighbors
        .into_iter()
        .filter(|neighbor| relevant.contains(neighbor))
        .collect_vec();
    neighbors.sort();
    neighbors.dedup();
    neighbors
}

fn compare_paths<NIndex: GIndex, L: Letter>(
    left: &Path<NIndex, L>,
    right: &Path<NIndex, L>,
) -> Ordering {
    left.states
        .cmp(&right.states)
        .then(left.transitions.cmp(&right.transitions))
}

fn build_scc_tree<NIndex, L>(
    component_index: usize,
    components: &[SCC<NIndex>],
    component_edges: &[Vec<SCCTreeEdgeSpec<NIndex, L>>],
) -> SCCTree<NIndex, L>
where
    NIndex: GIndex,
    L: Letter,
{
    let children = component_edges[component_index]
        .iter()
        .map(|edge| SCCTreeEdge {
            path: edge.path.clone(),
            child: Box::new(build_scc_tree(
                edge.target_component,
                components,
                component_edges,
            )),
        })
        .collect();

    SCCTree {
        scc: components[component_index].clone(),
        children,
    }
}

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
