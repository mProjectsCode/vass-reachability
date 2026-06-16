use std::{fmt, sync::Arc};

use hashbrown::{HashMap, HashSet};
use petgraph::{
    algo::kosaraju_scc,
    graph::{DiGraph, EdgeIndex, NodeIndex},
    visit::EdgeRef,
};

use super::{
    LinearGraph, LinearGraphAutomaton,
    part::{LinearGraphPart, LinearGraphPathSegment, LinearGraphRegion},
};
use crate::automaton::{
    Alphabet, GIndex, Language,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
    nfa::NFA,
    path::Path,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RootedLinearGraphError {
    EmptyLinearGraph,
    MissingGraph { part: usize, graph: usize },
    MissingPath { part: usize, path: usize },
    MissingRepeatPath { part: usize, path: usize },
    InvalidGraphReferenceCount { graph: usize, references: usize },
    InvalidPathReferenceCount { path: usize, references: usize },
    EmptyPath { path: usize },
    InvalidPath { path: usize },
    InvalidRepeatPath { path: usize },
    InvalidGraphBoundary { graph: usize },
    DisconnectedParts { left_part: usize, right_part: usize },
    RegionIsEmpty { graph: usize },
    RegionIsNotStronglyConnected { graph: usize },
    RegionIsNotRooted { graph: usize },
    MissingFinalRootedRegion,
}

impl fmt::Display for RootedLinearGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyLinearGraph => {
                write!(formatter, "linear graph must contain at least one part")
            }
            Self::MissingGraph { part, graph } => {
                write!(formatter, "part {part} references missing graph {graph}")
            }
            Self::MissingPath { part, path } => {
                write!(formatter, "part {part} references missing path {path}")
            }
            Self::MissingRepeatPath { part, path } => {
                write!(
                    formatter,
                    "part {part} references missing repeated path {path}"
                )
            }
            Self::InvalidGraphReferenceCount { graph, references } => write!(
                formatter,
                "graph {graph} must be referenced exactly once, found {references} references"
            ),
            Self::InvalidPathReferenceCount { path, references } => write!(
                formatter,
                "path {path} must be referenced exactly once, found {references} references"
            ),
            Self::EmptyPath { path } => write!(formatter, "path {path} contains no states"),
            Self::InvalidPath { path } => {
                write!(
                    formatter,
                    "path {path} has inconsistent states and transitions"
                )
            }
            Self::InvalidRepeatPath { path } => {
                write!(formatter, "repeated path {path} is empty or not closed")
            }
            Self::InvalidGraphBoundary { graph } => {
                write!(formatter, "graph {graph} has a missing start or end node")
            }
            Self::DisconnectedParts {
                left_part,
                right_part,
            } => write!(
                formatter,
                "parts {left_part} and {right_part} do not share a boundary"
            ),
            Self::RegionIsEmpty { graph } => write!(formatter, "graph {graph} is empty"),
            Self::RegionIsNotStronglyConnected { graph } => {
                write!(formatter, "graph {graph} is not strongly connected")
            }
            Self::RegionIsNotRooted { graph } => {
                write!(
                    formatter,
                    "graph {graph} has different entry and exit nodes"
                )
            }
            Self::MissingFinalRootedRegion => {
                write!(formatter, "rooted linear graph must end in a graph region")
            }
        }
    }
}

impl std::error::Error for RootedLinearGraphError {}

/// A linear graph whose graph regions are SCCs entered and exited at the same
/// product state. The last region's root is the graph's sole final state.
#[derive(Debug, Clone)]
pub struct RootedLinearGraph<'a, NIndex: GIndex = MultiGraphState, A = ImplicitCFGProduct>
where
    A: LinearGraphAutomaton<NIndex>,
{
    inner: LinearGraph<'a, NIndex, A>,
}

impl<'a, NIndex: GIndex, A> RootedLinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    pub fn try_from_linear_graph(
        inner: LinearGraph<'a, NIndex, A>,
    ) -> Result<Self, RootedLinearGraphError> {
        validate_structure(&inner)?;
        validate_rooted(&inner)?;
        Ok(Self { inner })
    }

    pub fn as_linear_graph(&self) -> &LinearGraph<'a, NIndex, A> {
        &self.inner
    }

    pub fn into_linear_graph(self) -> LinearGraph<'a, NIndex, A> {
        self.inner
    }

    pub fn root(&self) -> &NIndex {
        let last = self
            .inner
            .sequence
            .last()
            .expect("validated rooted linear graph must have a final region");
        last.unwrap_graph(&self.inner).product_start()
    }

    pub fn to_nfa(&self) -> NFA<(), CFGCounterUpdate> {
        self.inner.to_nfa()
    }

    pub fn to_cfg(&self) -> VASSCFG<()> {
        self.inner.to_cfg()
    }

    pub fn to_graphviz(&self) -> String {
        let mut dot = String::from(
            "digraph rooted_linear_graph {\n\
             fontname=\"Helvetica,Arial,sans-serif\"\n\
             node [fontname=\"Helvetica,Arial,sans-serif\"]\n\
             edge [fontname=\"Helvetica,Arial,sans-serif\"]\n\
             rankdir=LR;\n\
             START [shape=point,label=\"\"];\n",
        );
        let mut previous_end = None;

        for (part_index, part) in self.inner.sequence.iter().enumerate() {
            let (part_start, part_end) = match *part {
                LinearGraphPart::Graph(graph_index) => {
                    let graph = self.inner.graph(graph_index);
                    dot.push_str(&format!(
                        "subgraph cluster_{part_index} {{\nlabel=\"SCC {part_index}\";\n"
                    ));

                    for node in graph.graph.node_indices() {
                        let id = format!("part_{part_index}_node_{}", node.index());
                        let is_root = node == graph.start;
                        let is_final = part_index + 1 == self.inner.sequence.len() && is_root;
                        let shape = if is_final { "doublecircle" } else { "circle" };
                        let color = if is_root { "blue" } else { "black" };
                        let root_label = if is_root { "\\nroot" } else { "" };
                        dot.push_str(&format!(
                            "{id} [shape={shape},color={color},label=\"{}{}\"];\n",
                            escape_dot_label(&format!("{:?}", graph.graph[node])),
                            root_label
                        ));
                    }

                    for edge in graph.graph.edge_references() {
                        dot.push_str(&format!(
                            "part_{part_index}_node_{} -> part_{part_index}_node_{} [label=\"{}\"];\n",
                            edge.source().index(),
                            edge.target().index(),
                            escape_dot_label(&format!("{:?}", edge.weight()))
                        ));
                    }
                    dot.push_str("}\n");

                    (
                        format!("part_{part_index}_node_{}", graph.start.index()),
                        format!("part_{part_index}_node_{}", graph.end.index()),
                    )
                }
                LinearGraphPart::Path(path_index) => {
                    let path = &self.inner.path(path_index).path;
                    dot.push_str(&format!(
                        "subgraph cluster_{part_index} {{\nlabel=\"Path {part_index}\";\n"
                    ));

                    for (state_index, state) in path.states.iter().enumerate() {
                        dot.push_str(&format!(
                            "part_{part_index}_node_{state_index} [shape=circle,label=\"{}\"];\n",
                            escape_dot_label(&format!("{state:?}"))
                        ));
                    }
                    for (edge_index, update) in path.transitions.iter().enumerate() {
                        dot.push_str(&format!(
                            "part_{part_index}_node_{edge_index} -> part_{part_index}_node_{} [label=\"{}\"];\n",
                            edge_index + 1,
                            escape_dot_label(&format!("{update:?}"))
                        ));
                    }
                    dot.push_str("}\n");

                    (
                        format!("part_{part_index}_node_0"),
                        format!("part_{part_index}_node_{}", path.states.len() - 1),
                    )
                }
                LinearGraphPart::RepeatPath(path_index) => {
                    let path = &self.inner.repeat_path(path_index).path;
                    dot.push_str(&format!(
                        "subgraph cluster_{part_index} {{\nlabel=\"RepeatPath {part_index}\";\n"
                    ));

                    for (state_index, state) in path.states.iter().enumerate() {
                        dot.push_str(&format!(
                            "part_{part_index}_node_{state_index} [shape=circle,label=\"{}\"];\n",
                            escape_dot_label(&format!("{state:?}"))
                        ));
                    }
                    for (edge_index, update) in path.transitions.iter().enumerate() {
                        dot.push_str(&format!(
                            "part_{part_index}_node_{edge_index} -> part_{part_index}_node_{} [label=\"{}\"];\n",
                            edge_index + 1,
                            escape_dot_label(&format!("{update:?}"))
                        ));
                    }
                    dot.push_str(&format!(
                        "part_{part_index}_node_{} -> part_{part_index}_node_0 [style=dotted,label=\"repeat\"];\n",
                        path.states.len() - 1
                    ));
                    dot.push_str("}\n");

                    (
                        format!("part_{part_index}_node_0"),
                        format!("part_{part_index}_node_0"),
                    )
                }
            };

            if let Some(previous_end) = previous_end {
                dot.push_str(&format!(
                    "{previous_end} -> {part_start} [style=dashed,label=\"epsilon\"];\n"
                ));
            } else {
                dot.push_str(&format!("START -> {part_start};\n"));
            }
            previous_end = Some(part_end);
        }

        dot.push_str("}\n");
        dot
    }

    pub fn iter_parts(&self) -> impl Iterator<Item = &LinearGraphPart> {
        self.inner.iter_parts()
    }

    pub fn iter_path_parts(&self) -> impl Iterator<Item = &LinearGraphPathSegment<NIndex>> {
        self.inner.iter_path_parts()
    }

    pub fn iter_graph_parts(&self) -> impl Iterator<Item = &LinearGraphRegion<NIndex>> {
        self.inner.iter_graph_parts()
    }
}

fn escape_dot_label(label: &str) -> String {
    label
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

impl<NIndex: GIndex, A> Language for RootedLinearGraph<'_, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    fn accepts<'b>(&self, input: impl IntoIterator<Item = &'b CFGCounterUpdate>) -> bool
    where
        CFGCounterUpdate: 'b,
    {
        self.inner.accepts(input)
    }
}

impl<NIndex: GIndex, A> Alphabet for RootedLinearGraph<'_, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    type Letter = CFGCounterUpdate;

    fn alphabet(&self) -> &[CFGCounterUpdate] {
        self.inner.alphabet()
    }
}

impl<'a, NIndex: GIndex, A> LinearGraph<'a, NIndex, A>
where
    A: LinearGraphAutomaton<NIndex>,
{
    /// Produces an exact finite cover by rooted linear graphs.
    ///
    /// Every returned language is contained in this linear graph's language,
    /// and the union of all returned languages equals this graph's language.
    /// The returned languages may overlap.
    ///
    /// The result can be exponential in the number of simple paths through
    /// the graph regions. Output order is deterministic.
    pub fn refine_to_rooted(
        &self,
    ) -> Result<Vec<RootedLinearGraph<'a, NIndex, A>>, RootedLinearGraphError> {
        validate_structure(self)?;

        let mut candidates = vec![LinearGraph::empty(self.automaton, self.dimension)];

        for part in &self.sequence {
            match *part {
                LinearGraphPart::Path(path_index) => {
                    let path = self.paths[path_index].clone();
                    for candidate in &mut candidates {
                        candidate.add_path(path.clone());
                    }
                }
                LinearGraphPart::RepeatPath(path_index) => {
                    let path = self.repeat_paths[path_index].clone();
                    for candidate in &mut candidates {
                        candidate.add_repeat_path(path.clone());
                    }
                }
                LinearGraphPart::Graph(graph_index) => {
                    let fragments = refine_region(&self.graphs[graph_index]);
                    let mut expanded =
                        Vec::with_capacity(candidates.len().saturating_mul(fragments.len()));

                    for candidate in candidates {
                        for fragment in &fragments {
                            let mut next = candidate.clone();
                            append_fragment(&mut next, fragment);
                            expanded.push(next);
                        }
                    }

                    candidates = expanded;
                }
            }
        }

        for candidate in &mut candidates {
            if !candidate
                .sequence
                .last()
                .is_some_and(LinearGraphPart::is_graph)
            {
                let final_state = candidate
                    .sequence
                    .last()
                    .expect("candidate is non-empty")
                    .end(candidate)
                    .clone();
                candidate.add_graph(singleton_region(
                    final_state,
                    candidate.automaton.alphabet().to_vec(),
                ));
            }
        }

        candidates
            .into_iter()
            .map(RootedLinearGraph::try_from_linear_graph)
            .collect()
    }
}

#[derive(Clone)]
enum FragmentPart<NIndex: GIndex> {
    Graph(Arc<LinearGraphRegion<NIndex>>),
    Path(LinearGraphPathSegment<NIndex>),
}

fn append_fragment<'a, NIndex: GIndex, A>(
    target: &mut LinearGraph<'a, NIndex, A>,
    fragment: &[FragmentPart<NIndex>],
) where
    A: LinearGraphAutomaton<NIndex>,
{
    for part in fragment {
        match part {
            FragmentPart::Graph(graph) => target.add_graph(Arc::clone(graph)),
            FragmentPart::Path(path) => target.add_path(path.clone()),
        }
    }
}

fn refine_region<NIndex: GIndex>(
    region: &LinearGraphRegion<NIndex>,
) -> Vec<Vec<FragmentPart<NIndex>>> {
    // Cycle erasure decomposes every entry-to-exit walk into a simple path
    // plus closed walks rooted at its vertices. Each closed walk stays inside
    // the root's SCC, which is exactly what these rooted regions represent.
    let components = sorted_components(region);
    let component_of = components
        .iter()
        .enumerate()
        .flat_map(|(component, nodes)| nodes.iter().copied().map(move |node| (node, component)))
        .collect::<HashMap<_, _>>();
    let mut rooted_regions = HashMap::new();

    for node in region.graph.node_indices() {
        let component = component_of[&node];
        rooted_regions.insert(
            node,
            Arc::new(induced_rooted_region(region, &components[component], node)),
        );
    }

    simple_edge_paths(region)
        .into_iter()
        .map(|edges| {
            let mut fragment = Vec::with_capacity(edges.len().saturating_mul(2) + 1);
            let mut current = region.start;
            fragment.push(FragmentPart::Graph(Arc::clone(&rooted_regions[&current])));

            for edge_index in edges {
                let (source, target) = region
                    .graph
                    .edge_endpoints(edge_index)
                    .expect("enumerated edge must remain live");
                debug_assert_eq!(source, current);

                let mut path = Path::new(region.graph[source].clone());
                path.add(
                    *region
                        .graph
                        .edge_weight(edge_index)
                        .expect("enumerated edge must remain live"),
                    region.graph[target].clone(),
                );
                fragment.push(FragmentPart::Path(path.into()));
                fragment.push(FragmentPart::Graph(Arc::clone(&rooted_regions[&target])));
                current = target;
            }

            fragment
        })
        .collect()
}

fn sorted_components<NIndex: GIndex>(region: &LinearGraphRegion<NIndex>) -> Vec<Vec<NodeIndex>> {
    let mut components = kosaraju_scc(&region.graph);
    for component in &mut components {
        component.sort_unstable_by_key(|node| node.index());
    }
    components.sort_unstable_by_key(|component| component[0].index());
    components
}

fn induced_rooted_region<NIndex: GIndex>(
    source: &LinearGraphRegion<NIndex>,
    nodes: &[NodeIndex],
    root: NodeIndex,
) -> LinearGraphRegion<NIndex> {
    let node_set = nodes.iter().copied().collect::<HashSet<_>>();
    let mut graph = DiGraph::new();
    let mut node_map = HashMap::new();

    for source_node in nodes {
        node_map.insert(
            *source_node,
            graph.add_node(source.graph[*source_node].clone()),
        );
    }

    let mut edges = source
        .graph
        .edge_references()
        .filter(|edge| node_set.contains(&edge.source()) && node_set.contains(&edge.target()))
        .map(|edge| (edge.id(), edge.source(), edge.target(), *edge.weight()))
        .collect::<Vec<_>>();
    edges.sort_unstable_by_key(|(edge, _, _, _)| edge.index());

    for (_, from, to, label) in edges {
        graph.add_edge(node_map[&from], node_map[&to], label);
    }

    let root = node_map[&root];
    LinearGraphRegion::new(graph, root, root, source.alphabet.clone())
}

fn singleton_region<NIndex: GIndex>(
    state: NIndex,
    alphabet: Vec<CFGCounterUpdate>,
) -> LinearGraphRegion<NIndex> {
    let mut graph = DiGraph::new();
    let root = graph.add_node(state);
    LinearGraphRegion::new(graph, root, root, alphabet)
}

fn simple_edge_paths<NIndex: GIndex>(region: &LinearGraphRegion<NIndex>) -> Vec<Vec<EdgeIndex>> {
    fn visit<NIndex: GIndex>(
        region: &LinearGraphRegion<NIndex>,
        current: NodeIndex,
        visited: &mut HashSet<NodeIndex>,
        path: &mut Vec<EdgeIndex>,
        result: &mut Vec<Vec<EdgeIndex>>,
    ) {
        if current == region.end {
            result.push(path.clone());
            return;
        }

        let mut outgoing = region
            .graph
            .edges(current)
            .map(|edge| (edge.target(), edge.id()))
            .collect::<Vec<_>>();
        outgoing.sort_unstable_by_key(|(target, edge)| (target.index(), edge.index()));

        for (target, edge) in outgoing {
            if !visited.insert(target) {
                continue;
            }

            path.push(edge);
            visit(region, target, visited, path, result);
            path.pop();
            visited.remove(&target);
        }
    }

    let mut result = Vec::new();
    let mut visited = HashSet::new();
    visited.insert(region.start);
    visit(
        region,
        region.start,
        &mut visited,
        &mut Vec::new(),
        &mut result,
    );
    result
}

fn validate_structure<NIndex: GIndex, A>(
    linear_graph: &LinearGraph<'_, NIndex, A>,
) -> Result<(), RootedLinearGraphError>
where
    A: LinearGraphAutomaton<NIndex>,
{
    if linear_graph.sequence.is_empty() {
        return Err(RootedLinearGraphError::EmptyLinearGraph);
    }

    let mut graph_references = vec![0usize; linear_graph.graphs.len()];
    let mut path_references = vec![0usize; linear_graph.paths.len()];
    let mut repeat_path_references = vec![0usize; linear_graph.repeat_paths.len()];

    for (part_index, part) in linear_graph.sequence.iter().enumerate() {
        match *part {
            LinearGraphPart::Graph(graph_index) => {
                let Some(graph) = linear_graph.graphs.get(graph_index) else {
                    return Err(RootedLinearGraphError::MissingGraph {
                        part: part_index,
                        graph: graph_index,
                    });
                };
                graph_references[graph_index] += 1;
                if graph.graph.node_weight(graph.start).is_none()
                    || graph.graph.node_weight(graph.end).is_none()
                {
                    return Err(RootedLinearGraphError::InvalidGraphBoundary {
                        graph: graph_index,
                    });
                }
            }
            LinearGraphPart::Path(path_index) => {
                let Some(path) = linear_graph.paths.get(path_index) else {
                    return Err(RootedLinearGraphError::MissingPath {
                        part: part_index,
                        path: path_index,
                    });
                };
                path_references[path_index] += 1;
                if path.path.states.is_empty() {
                    return Err(RootedLinearGraphError::EmptyPath { path: path_index });
                }
                if path.path.states.len() != path.path.transitions.len() + 1 {
                    return Err(RootedLinearGraphError::InvalidPath { path: path_index });
                }
            }
            LinearGraphPart::RepeatPath(path_index) => {
                let Some(path) = linear_graph.repeat_paths.get(path_index) else {
                    return Err(RootedLinearGraphError::MissingRepeatPath {
                        part: part_index,
                        path: path_index,
                    });
                };
                repeat_path_references[path_index] += 1;
                if path.path.is_empty() || path.path.start() != path.path.end() {
                    return Err(RootedLinearGraphError::InvalidRepeatPath { path: path_index });
                }
            }
        }
    }

    for (graph, references) in graph_references.into_iter().enumerate() {
        if references != 1 {
            return Err(RootedLinearGraphError::InvalidGraphReferenceCount { graph, references });
        }
    }
    for (path, references) in path_references.into_iter().enumerate() {
        if references != 1 {
            return Err(RootedLinearGraphError::InvalidPathReferenceCount { path, references });
        }
    }
    for (path, references) in repeat_path_references.into_iter().enumerate() {
        if references != 1 {
            return Err(RootedLinearGraphError::InvalidPathReferenceCount { path, references });
        }
    }

    for (left_index, parts) in linear_graph.sequence.windows(2).enumerate() {
        if parts[0].end(linear_graph) != parts[1].start(linear_graph) {
            return Err(RootedLinearGraphError::DisconnectedParts {
                left_part: left_index,
                right_part: left_index + 1,
            });
        }
    }

    Ok(())
}

fn validate_rooted<NIndex: GIndex, A>(
    linear_graph: &LinearGraph<'_, NIndex, A>,
) -> Result<(), RootedLinearGraphError>
where
    A: LinearGraphAutomaton<NIndex>,
{
    for (graph_index, graph) in linear_graph.graphs.iter().enumerate() {
        if graph.graph.node_count() == 0 {
            return Err(RootedLinearGraphError::RegionIsEmpty { graph: graph_index });
        }
        if graph.start != graph.end {
            return Err(RootedLinearGraphError::RegionIsNotRooted { graph: graph_index });
        }
        if kosaraju_scc(&graph.graph).len() != 1 {
            return Err(RootedLinearGraphError::RegionIsNotStronglyConnected {
                graph: graph_index,
            });
        }
    }

    if !linear_graph
        .sequence
        .last()
        .is_some_and(LinearGraphPart::is_graph)
    {
        return Err(RootedLinearGraphError::MissingFinalRootedRegion);
    }

    Ok(())
}
