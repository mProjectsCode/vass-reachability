use std::sync::Arc;

use hashbrown::{HashMap, HashSet};

use super::MultiGraphPath;
use crate::{
    automaton::{
        InitializedAutomaton,
        cfg::update::CFGCounterUpdate,
        implicit_cfg_product::{state::MultiGraphState, view::ImplicitCFGProductView},
        linear_graph::{
            LinearGraph,
            part::{LinearGraphPart, LinearGraphRegion},
        },
        path::Path,
        scc::{SCCDag, SCCDagEdge},
    },
    solver::linear_graph_reach::LinearGraphSolution,
};

type ProductViewLinearGraph<'a> = LinearGraph<'a, MultiGraphState, ImplicitCFGProductView<'a>>;

#[derive(Debug, Clone)]
pub(super) struct InterpolationLayout<'a> {
    automaton: &'a ImplicitCFGProductView<'a>,
    dimension: usize,
    items: Vec<InterpolationItem>,
    pub(super) regions: Vec<SccRegion<'a>>,
}

#[derive(Debug, Clone)]
enum InterpolationItem {
    FixedPath(MultiGraphPath),
    Region(usize),
}

#[derive(Debug, Clone)]
pub(super) struct SccRegion<'a> {
    seed: ProductViewLinearGraph<'a>,
    full_size: usize,
    full_graph: Arc<LinearGraphRegion<MultiGraphState>>,
}

#[derive(Debug)]
pub(super) struct CandidateBuildResult<'a> {
    pub(super) linear_graph: ProductViewLinearGraph<'a>,
    graph_to_full_region: Vec<Option<usize>>,
}

#[derive(Debug)]
pub(super) struct CandidateSeed<'a> {
    pub(super) path_indices: Vec<usize>,
    pub(super) layout: InterpolationLayout<'a>,
    pub(super) seed_linear_graph: ProductViewLinearGraph<'a>,
}

#[derive(Debug, Clone)]
struct SegmentedPath {
    segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
struct IndexedSegmentedPath {
    path_index: usize,
    segmented: SegmentedPath,
}

#[derive(Debug, Clone)]
enum PathSegment {
    Fixed(MultiGraphPath),
    Region { path: MultiGraphPath },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DagRoute {
    edges: Vec<DagRouteEdge>,
    accepting: MultiGraphState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DagRouteEdge {
    source_component: usize,
    edge: SCCDagEdge<MultiGraphState, CFGCounterUpdate>,
}

struct DagContext<'d> {
    dag: &'d SCCDag<MultiGraphState, CFGCounterUpdate>,
    component_of_state: HashMap<MultiGraphState, usize>,
}

impl<'d> DagContext<'d> {
    fn new(dag: &'d SCCDag<MultiGraphState, CFGCounterUpdate>) -> Self {
        let component_of_state = dag
            .components
            .iter()
            .enumerate()
            .flat_map(|(component, scc)| {
                scc.nodes
                    .iter()
                    .cloned()
                    .map(move |state| (state, component))
            })
            .collect();

        Self {
            dag,
            component_of_state,
        }
    }

    fn component(&self, state: &MultiGraphState) -> Option<usize> {
        self.component_of_state.get(state).copied()
    }
}

impl<'a> InterpolationLayout<'a> {
    /// Builds candidate layouts from the primary path plus compatible
    /// auxiliary paths.
    ///
    /// Auxiliary paths whose fixed/expandable-region shape differs from the
    /// primary path are dropped. Every returned candidate contains the primary
    /// path.
    ///
    /// Candidate groups are a size-ranked best-effort sample: trying every
    /// auxiliary subset is exponential, so this keeps the largest prefixes of
    /// compatible paths and lets the caller spend a bounded number of solver
    /// checks on them.
    pub(super) fn from_compatible_path_groups(
        primary_path: &MultiGraphPath,
        auxiliary_paths: &[MultiGraphPath],
        automaton: &'a ImplicitCFGProductView<'a>,
        dimension: usize,
        dag: &SCCDag<MultiGraphState, CFGCounterUpdate>,
    ) -> Vec<CandidateSeed<'a>> {
        let dag = DagContext::new(dag);
        let Some(primary_route) = dag_route_for_path(&dag, primary_path) else {
            return Vec::new();
        };
        let Some(full_linear_graph) =
            build_full_linear_graph_from_dag_route(automaton, dimension, &dag, &primary_route)
        else {
            return Vec::new();
        };
        let Some(primary) =
            segment_path_by_full_linear_graph(primary_path, &full_linear_graph, &dag)
        else {
            return Vec::new();
        };

        let compatible_auxiliaries =
            compatible_auxiliary_paths(auxiliary_paths, &dag, &primary_route, &full_linear_graph);

        candidate_auxiliary_counts(compatible_auxiliaries.len())
            .into_iter()
            .filter_map(|auxiliary_count| {
                let group = std::iter::once(IndexedSegmentedPath {
                    path_index: 0,
                    segmented: primary.clone(),
                })
                .chain(compatible_auxiliaries.iter().take(auxiliary_count).cloned())
                .collect();

                Self::from_segmented_group(group, &full_linear_graph, automaton, dimension)
            })
            .map(|(path_indices, layout)| {
                let seed_linear_graph = layout.build_seed_linear_graph();
                CandidateSeed {
                    path_indices,
                    layout,
                    seed_linear_graph,
                }
            })
            .collect()
    }

    /// Converts one compatible group of segmented paths into a concrete
    /// interpolation layout.
    ///
    /// Fixed segments stay on the exact DAG-route connector paths. Region
    /// segments keep a seed LinearGraph plus the route's full-SCC graph so
    /// candidate builds can toggle each region without changing the
    /// DAG-route shape.
    fn from_segmented_group(
        group: Vec<IndexedSegmentedPath>,
        full_linear_graph: &ProductViewLinearGraph<'a>,
        automaton: &'a ImplicitCFGProductView<'a>,
        dimension: usize,
    ) -> Option<(Vec<usize>, Self)> {
        let first = &group.first()?.segmented;
        let path_indices = group.iter().map(|path| path.path_index).collect::<Vec<_>>();
        let mut items = Vec::new();
        let mut regions = Vec::new();

        for (segment_index, segment) in first.segments.iter().enumerate() {
            match segment {
                PathSegment::Fixed(path) => {
                    items.push(InterpolationItem::FixedPath(path.clone()));
                }
                PathSegment::Region { .. } => {
                    let paths = group
                        .iter()
                        .map(|path| match &path.segmented.segments[segment_index] {
                            PathSegment::Region { path } => path.clone(),
                            PathSegment::Fixed(_) => unreachable!("signature mismatch"),
                        })
                        .collect::<Vec<_>>();

                    let seed = build_seed_region(automaton, dimension, &paths);
                    let full_graph = full_graph_for_segment(full_linear_graph, segment_index)?;
                    let full_size = full_graph.graph.node_count();

                    let region = regions.len();
                    regions.push(SccRegion {
                        seed,
                        full_size,
                        full_graph,
                    });
                    items.push(InterpolationItem::Region(region));
                }
            }
        }

        Some((
            path_indices,
            Self {
                automaton,
                dimension,
                items,
                regions,
            },
        ))
    }

    /// Builds the lower-bound candidate where every SCC region uses the
    /// selected seed-language graph.
    pub(super) fn build_seed_linear_graph(&self) -> ProductViewLinearGraph<'a> {
        let mask = vec![false; self.regions.len()];
        self.build_candidate(&mask).linear_graph
    }

    /// Materializes a candidate LinearGraph from a region mask and records
    /// which graph parts correspond to full regions.
    pub(super) fn build_candidate(&self, mask: &[bool]) -> CandidateBuildResult<'a> {
        let mut linear_graph = LinearGraph::empty(self.automaton, self.dimension);
        let mut graph_to_full_region = Vec::new();

        for item in &self.items {
            match item {
                InterpolationItem::FixedPath(path) => {
                    linear_graph.add_path(path.clone().into());
                }
                InterpolationItem::Region(region_index) => {
                    let region = &self.regions[*region_index];

                    if mask[*region_index] {
                        linear_graph.add_graph(Arc::clone(&region.full_graph));
                        graph_to_full_region.push(Some(*region_index));
                    } else {
                        append_linear_graph(
                            &mut linear_graph,
                            &region.seed,
                            &mut graph_to_full_region,
                        );
                    }
                }
            }
        }

        linear_graph.assert_consistent();

        CandidateBuildResult {
            linear_graph,
            graph_to_full_region,
        }
    }
}

fn compatible_auxiliary_paths(
    auxiliary_paths: &[MultiGraphPath],
    dag: &DagContext<'_>,
    primary_route: &DagRoute,
    full_linear_graph: &ProductViewLinearGraph<'_>,
) -> Vec<IndexedSegmentedPath> {
    let mut paths = auxiliary_paths
        .iter()
        .enumerate()
        .filter_map(|(auxiliary_index, path)| {
            let route = dag_route_for_path(dag, path)?;
            if &route != primary_route {
                return None;
            }

            let segmented = segment_path_by_full_linear_graph(path, full_linear_graph, dag)?;
            Some(IndexedSegmentedPath {
                path_index: auxiliary_index + 1,
                segmented,
            })
        })
        .collect::<Vec<_>>();

    paths.sort_by_key(|path| std::cmp::Reverse(segmented_path_size(&path.segmented)));
    paths
}

fn dag_route_for_path(dag: &DagContext<'_>, path: &MultiGraphPath) -> Option<DagRoute> {
    let mut edges = Vec::new();

    for edge_index in 0..path.transitions.len() {
        let source_component = dag.component(&path.states[edge_index])?;
        let target_component = dag.component(&path.states[edge_index + 1])?;

        if source_component == target_component {
            continue;
        }

        let mut crossing = MultiGraphPath::new(path.states[edge_index].clone());
        crossing.add(
            path.transitions[edge_index],
            path.states[edge_index + 1].clone(),
        );

        let edge = dag
            .dag
            .outgoing_edges(source_component)
            .iter()
            .find(|edge| edge.target_component == target_component && edge.path == crossing)?
            .clone();

        edges.push(DagRouteEdge {
            source_component,
            edge,
        });
    }

    let accepting_component = dag.component(path.end())?;
    if !dag.dag.components[accepting_component]
        .accepting_nodes
        .contains(path.end())
    {
        return None;
    }

    Some(DagRoute {
        edges,
        accepting: path.end().clone(),
    })
}

fn build_full_linear_graph_from_dag_route<'a>(
    automaton: &'a ImplicitCFGProductView<'a>,
    dimension: usize,
    dag: &DagContext<'_>,
    route: &DagRoute,
) -> Option<ProductViewLinearGraph<'a>> {
    let mut linear_graph = LinearGraph::empty(automaton, dimension);
    let mut current_path = MultiGraphPath::new(automaton.get_initial());
    let mut entry_node = automaton.get_initial();
    let mut component_index = dag.dag.root_component;

    for route_edge in &route.edges {
        if route_edge.source_component != component_index {
            return None;
        }

        let component = &dag.dag.components[component_index];
        let exit_node = route_edge.edge.path.start().clone();

        if component.is_trivial() {
            if entry_node != exit_node {
                return None;
            }
        } else {
            if !current_path.is_empty() {
                linear_graph.add_path(current_path.clone().into());
            }

            linear_graph.add_graph(LinearGraphRegion::from_subset(
                automaton,
                &component.nodes,
                entry_node.clone(),
                exit_node.clone(),
            ));
            current_path = MultiGraphPath::new(exit_node);
        }

        current_path.concat(route_edge.edge.path.clone());
        entry_node = route_edge.edge.path.end().clone();
        component_index = route_edge.edge.target_component;
    }

    let component = &dag.dag.components[component_index];
    if !component.accepting_nodes.contains(&route.accepting) {
        return None;
    }

    if component.is_trivial() {
        if entry_node != route.accepting {
            return None;
        }
    } else {
        if !current_path.is_empty() {
            linear_graph.add_path(current_path.clone().into());
        }

        linear_graph.add_graph(LinearGraphRegion::from_subset(
            automaton,
            &component.nodes,
            entry_node,
            route.accepting.clone(),
        ));
        current_path = MultiGraphPath::new(route.accepting.clone());
    }

    if !current_path.is_empty() {
        linear_graph.add_path(current_path.into());
    }

    linear_graph.assert_consistent();
    Some(linear_graph)
}

fn segment_path_by_full_linear_graph(
    path: &MultiGraphPath,
    full_linear_graph: &ProductViewLinearGraph<'_>,
    dag: &DagContext<'_>,
) -> Option<SegmentedPath> {
    let mut segments = Vec::with_capacity(full_linear_graph.sequence.len());
    let mut state_index = 0usize;

    for part in &full_linear_graph.sequence {
        match part {
            LinearGraphPart::Path(path_index) => {
                let fixed = &full_linear_graph.path(*path_index).path;
                let end_index = state_index.checked_add(fixed.len())?;
                if end_index > path.len() {
                    return None;
                }

                let segment = path.slice(state_index..end_index);
                if segment != *fixed {
                    return None;
                }

                segments.push(PathSegment::Fixed(segment));
                state_index = end_index;
            }
            LinearGraphPart::Graph(graph_index) => {
                let graph = full_linear_graph.graph(*graph_index);
                if path.states.get(state_index)? != graph.product_start() {
                    return None;
                }

                let component = dag.component(graph.product_start())?;
                let mut run_end = state_index;
                while run_end + 1 < path.states.len()
                    && dag.component(&path.states[run_end + 1])? == component
                {
                    run_end += 1;
                }

                if &path.states[run_end] != graph.product_end() {
                    return None;
                }

                segments.push(PathSegment::Region {
                    path: path.slice(state_index..run_end),
                });
                state_index = run_end;
            }
            LinearGraphPart::RepeatPath(_) => return None,
        }
    }

    if state_index != path.len() {
        return None;
    }

    Some(SegmentedPath { segments })
}

fn full_graph_for_segment<'a>(
    full_linear_graph: &ProductViewLinearGraph<'a>,
    segment_index: usize,
) -> Option<Arc<LinearGraphRegion<MultiGraphState>>> {
    let LinearGraphPart::Graph(graph_index) = full_linear_graph.sequence.get(segment_index)? else {
        return None;
    };

    Some(Arc::clone(&full_linear_graph.graphs[*graph_index]))
}

fn candidate_auxiliary_counts(auxiliary_len: usize) -> Vec<usize> {
    let mut sizes = Vec::new();
    let mut size = auxiliary_len;

    loop {
        sizes.push(size);
        if size == 0 {
            break;
        }
        size /= 2;
    }

    sizes
}

fn segmented_path_size(path: &SegmentedPath) -> usize {
    path.segments
        .iter()
        .map(|segment| match segment {
            PathSegment::Fixed(path) => path.state_len(),
            PathSegment::Region { path, .. } => path.state_len(),
        })
        .sum()
}

fn build_seed_region<'a>(
    automaton: &'a ImplicitCFGProductView<'a>,
    dimension: usize,
    paths: &[MultiGraphPath],
) -> ProductViewLinearGraph<'a> {
    if paths.len() == 1 {
        return LinearGraph::from_path_roll_up(paths[0].clone(), automaton, dimension);
    }

    let all_empty = paths.iter().all(Path::is_empty);
    let unique_nodes = Path::<MultiGraphState, CFGCounterUpdate>::sorted_union_states(paths);
    if all_empty && unique_nodes.len() <= 1 {
        return LinearGraph::empty(automaton, dimension);
    }

    let mut linear_graph = LinearGraph::empty(automaton, dimension);
    linear_graph.add_graph(LinearGraphRegion::from_subset(
        automaton,
        &unique_nodes,
        paths[0].start().clone(),
        paths[0].end().clone(),
    ));
    linear_graph
}

impl SccRegion<'_> {
    pub(super) fn gain(&self) -> usize {
        self.full_size.saturating_sub(self.seed.size())
    }
}

impl CandidateBuildResult<'_> {
    pub(super) fn used_full_regions(&self, solution: &LinearGraphSolution) -> HashSet<usize> {
        solution
            .sub_graph_parikh_images
            .iter()
            .enumerate()
            .filter_map(|(graph_index, image)| {
                if image.is_empty() {
                    return None;
                }

                self.graph_to_full_region
                    .get(graph_index)
                    .and_then(|region| *region)
            })
            .collect()
    }
}

fn append_linear_graph<'a>(
    target: &mut ProductViewLinearGraph<'a>,
    source: &ProductViewLinearGraph<'a>,
    graph_to_full_region: &mut Vec<Option<usize>>,
) {
    for part in &source.sequence {
        match part {
            LinearGraphPart::Path(path_index) => {
                target.add_path(source.path(*path_index).clone());
            }
            LinearGraphPart::Graph(graph_index) => {
                target.add_graph(Arc::clone(&source.graphs[*graph_index]));
                graph_to_full_region.push(None);
            }
            LinearGraphPart::RepeatPath(path_index) => {
                target.add_repeat_path(source.repeat_path(*path_index).clone());
            }
        }
    }
}
