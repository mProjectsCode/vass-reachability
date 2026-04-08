use std::cmp::Ordering;

use hashbrown::{HashMap, HashSet};
use itertools::Itertools;

use crate::automaton::{Deterministic, GIndex, InitializedAutomaton, Letter, path::Path};

pub trait SCCAlgorithms: InitializedAutomaton<Deterministic> {
    /// Find the SCC surrounding a given node. Returns a vector of all the nodes
    /// that are part of the SCC.
    fn find_scc_surrounding(&self, node: Self::NIndex) -> Vec<Self::NIndex> {
        // Restrict to nodes reachable from `node` using only forward edges,
        // then compute SCCs in that induced subgraph and pick the component
        // containing `node`.
        let forward_reachable = reachable_from(&node, |current| self.successors(current));
        let (components, component_of_node) = compute_sccs(self, &forward_reachable, &|_| false);

        let component = component_of_node
            .get(&node)
            .expect("Start node must be part of its own forward-reachable SCC set");
        let mut scc = components[*component].nodes.clone();
        scc.sort();
        scc
    }

    /// Builds the rooted SCC DAG of the accepting part of the graph.
    ///
    /// Only nodes that are reachable from the initial node and can still reach
    /// an accepting node are kept. The result is the condensation DAG of the
    /// relevant subgraph, rooted at the component containing the initial node.
    fn find_scc_dag(&self) -> SCCDag<Self::NIndex, Self::Letter>
    where
        Self::Letter: Letter,
        Self: Sized,
    {
        let initial = self.get_initial();
        let reachable = reachable_from(&initial, |current| self.successors(current));

        self.find_scc_dag_in_subgraph(initial, &reachable, |node| self.is_accepting(node))
    }

    fn find_scc_dag_in_subgraph<F>(
        &self,
        initial: Self::NIndex,
        allowed: &HashSet<Self::NIndex>,
        is_accepting: F,
    ) -> SCCDag<Self::NIndex, Self::Letter>
    where
        Self::Letter: Letter,
        Self: Sized,
        F: Fn(&Self::NIndex) -> bool,
    {
        let relevant =
            collect_relevant_scc_nodes_in_subgraph(self, &initial, allowed, &is_accepting);
        let (components, component_of_node) = compute_sccs(self, &relevant, &is_accepting);
        let component_edges =
            collect_component_edges(self, &relevant, &components, &component_of_node);

        SCCDag {
            root_component: component_of_node[&initial],
            components,
            edges: component_edges,
            trivial_paths_rolled: false,
        }
    }
}

impl<T: InitializedAutomaton<Deterministic>> SCCAlgorithms for T {}

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
pub struct SCCDag<NIndex: GIndex, L: Letter> {
    pub root_component: usize,
    pub components: Vec<SCC<NIndex>>,
    // For each component, the list of edges to other components.
    pub edges: Vec<Vec<SCCDagEdge<NIndex, L>>>,
    /// True iff non-accepting trivial SCCs have been bypassed so cross-
    /// component edges may carry longer connector paths.
    pub trivial_paths_rolled: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SCCDagEdge<NIndex: GIndex, L: Letter> {
    // A path from the source component to the target component, represented as a sequence of
    // states and transitions. The first state is part of the source component (and marks the
    // exit point), the last state is part of the target component (and marks the entry point), and
    // all intermediate states are outside of both components.
    pub path: Path<NIndex, L>,
    pub target_component: usize,
}

impl<NIndex: GIndex, L: Letter> SCCDag<NIndex, L> {
    pub fn root(&self) -> &SCC<NIndex> {
        &self.components[self.root_component]
    }

    pub fn outgoing_edges(&self, component: usize) -> &[SCCDagEdge<NIndex, L>] {
        &self.edges[component]
    }

    /// Returns a copy where non-accepting trivial SCCs are bypassed by
    /// concatenating incoming and outgoing edge paths.
    ///
    /// Accepting trivial SCCs are intentionally kept, because they represent
    /// valid terminal stopping points.
    pub fn with_rolled_trivial_paths(&self) -> Self {
        if self.trivial_paths_rolled {
            return self.clone();
        }

        let mut simplified = self.clone();

        loop {
            let mut changed = false;

            for component in 0..simplified.components.len() {
                if component == simplified.root_component {
                    continue;
                }

                let scc = &simplified.components[component];
                if !scc.is_trivial() || !scc.accepting_nodes.is_empty() {
                    continue;
                }

                let outgoing = simplified.edges[component].clone();
                let mut has_incoming = false;

                for source in 0..simplified.edges.len() {
                    if source == component {
                        continue;
                    }

                    let mut next_edges = Vec::new();
                    let mut rewrote_from_source = false;

                    for edge in simplified.edges[source].iter().cloned() {
                        if edge.target_component != component {
                            next_edges.push(edge);
                            continue;
                        }

                        has_incoming = true;
                        rewrote_from_source = true;

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
                        simplified.edges[source] = next_edges;
                        changed = true;
                    }
                }

                if has_incoming {
                    simplified.edges[component].clear();
                }
            }

            if !changed {
                break;
            }
        }

        simplified.trivial_paths_rolled = true;
        simplified
    }
}

type SCCComponents<NIndex> = (Vec<SCC<NIndex>>, HashMap<NIndex, usize>);

fn collect_relevant_scc_nodes_in_subgraph<A, F>(
    automaton: &A,
    initial: &A::NIndex,
    allowed: &HashSet<A::NIndex>,
    is_accepting: &F,
) -> HashSet<A::NIndex>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let reachable = reachable_from(initial, |current| {
        Box::new(
            automaton
                .successors(current)
                .filter(|neighbor| allowed.contains(neighbor)),
        )
    });
    let reverse_reachable = build_reverse_adjacency(automaton, &reachable);
    let accepting = reachable
        .iter()
        .filter(|node| is_accepting(node))
        .cloned()
        .collect_vec();

    assert!(
        !accepting.is_empty(),
        "Cannot build SCC DAG for a graph without reachable accepting nodes"
    );

    reachable_from_many(accepting, |current| {
        Box::new(
            reverse_reachable
                .get(current)
                .cloned()
                .unwrap_or_default()
                .into_iter(),
        )
    })
}

fn compute_sccs<A, F>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
    is_accepting: &F,
) -> SCCComponents<A::NIndex>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
    F: Fn(&A::NIndex) -> bool,
{
    let finish_order = compute_finish_order::<A>(automaton, relevant);
    let reverse_relevant = build_reverse_adjacency(automaton, relevant);
    collect_components_from_finish_order::<A, F>(
        automaton,
        relevant,
        finish_order,
        &reverse_relevant,
        is_accepting,
    )
}

fn compute_finish_order<A>(automaton: &A, relevant: &HashSet<A::NIndex>) -> Vec<A::NIndex>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
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

            for successor in automaton
                .successors(&current)
                .filter(|neighbor| relevant.contains(neighbor))
            {
                if !visited.contains(&successor) {
                    stack.push((successor, false));
                }
            }
        }
    }

    finish_order
}

fn collect_components_from_finish_order<A, F>(
    automaton: &A,
    _relevant: &HashSet<A::NIndex>,
    mut finish_order: Vec<A::NIndex>,
    reverse_relevant: &HashMap<A::NIndex, Vec<A::NIndex>>,
    is_accepting: &F,
) -> SCCComponents<A::NIndex>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
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

            for predecessor in reverse_relevant
                .get(&current)
                .into_iter()
                .flat_map(|neighbors| neighbors.iter())
            {
                if assigned.insert(predecessor.clone()) {
                    stack.push(predecessor.clone());
                }
            }
        }

        component_nodes.sort();

        let component_index = components.len();
        for component_node in &component_nodes {
            component_of_node.insert(component_node.clone(), component_index);
        }

        components.push(build_scc::<A, F>(automaton, component_nodes, is_accepting));
    }

    (components, component_of_node)
}

fn build_scc<A, F>(automaton: &A, nodes: Vec<A::NIndex>, is_accepting: &F) -> SCC<A::NIndex>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
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

fn collect_component_edges<A>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
    components: &[SCC<A::NIndex>],
    component_of_node: &HashMap<A::NIndex, usize>,
) -> Vec<Vec<SCCDagEdge<A::NIndex, A::Letter>>>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
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

                component_edges[component_index].push(SCCDagEdge {
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
    edges: &mut Vec<SCCDagEdge<NIndex, L>>,
) {
    edges.sort_by(|left, right| {
        compare_paths(&left.path, &right.path)
            .then(left.target_component.cmp(&right.target_component))
    });
    edges.dedup_by(|left, right| {
        left.target_component == right.target_component && left.path == right.path
    });
}

fn reachable_from<'a, NIndex>(
    start: &NIndex,
    mut next: impl FnMut(&NIndex) -> Box<dyn Iterator<Item = NIndex> + 'a>,
) -> HashSet<NIndex>
where
    NIndex: GIndex,
{
    let mut visited = HashSet::new();
    let mut stack = vec![start.clone()];

    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        for successor in next(&current) {
            if !visited.contains(&successor) {
                stack.push(successor);
            }
        }
    }

    visited
}

fn reachable_from_many<'a, NIndex>(
    starts: impl IntoIterator<Item = NIndex>,
    mut next: impl FnMut(&NIndex) -> Box<dyn Iterator<Item = NIndex> + 'a>,
) -> HashSet<NIndex>
where
    NIndex: GIndex,
{
    let mut visited = HashSet::new();
    let mut stack = starts.into_iter().collect_vec();

    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            continue;
        }

        for successor in next(&current) {
            if !visited.contains(&successor) {
                stack.push(successor);
            }
        }
    }

    visited
}

fn build_reverse_adjacency<A>(
    automaton: &A,
    relevant: &HashSet<A::NIndex>,
) -> HashMap<A::NIndex, Vec<A::NIndex>>
where
    A: InitializedAutomaton<Deterministic> + ?Sized,
{
    let mut reverse = HashMap::<A::NIndex, Vec<A::NIndex>>::new();

    for node in relevant {
        for successor in automaton.successors(node) {
            if !relevant.contains(&successor) {
                continue;
            }

            reverse.entry(successor).or_default().push(node.clone());
        }
    }

    reverse
}

fn compare_paths<NIndex: GIndex, L: Letter>(
    left: &Path<NIndex, L>,
    right: &Path<NIndex, L>,
) -> Ordering {
    left.states
        .cmp(&right.states)
        .then(left.transitions.cmp(&right.transitions))
}
