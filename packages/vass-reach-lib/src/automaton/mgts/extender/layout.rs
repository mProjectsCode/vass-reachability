use std::sync::Arc;

use hashbrown::HashSet;

use super::{
    MultiGraphPath,
    segmentation::{PathSegment, SegmentedPath, segment_path},
};
use crate::{
    automaton::{
        cfg::update::CFGCounterUpdate,
        implicit_cfg_product::{ImplicitCFGProduct, state::MultiGraphState},
        mgts::{
            MGTS,
            part::{MGTSPart, MarkedGraph},
        },
        path::Path,
        scc::{PrecomputedSccs, SccClassifier},
    },
    solver::mgts_reach::MGTSSolution,
};

#[derive(Debug, Clone)]
pub(super) struct InterpolationLayout<'a> {
    automaton: &'a ImplicitCFGProduct,
    dimension: usize,
    items: Vec<InterpolationItem>,
    pub(super) regions: Vec<SccRegion<'a>>,
}

#[derive(Debug, Clone)]
enum InterpolationItem {
    FixedPath(MultiGraphPath),
    FixedGraph(Arc<MarkedGraph<MultiGraphState>>),
    Region(usize),
}

#[derive(Debug, Clone)]
pub(super) struct SccRegion<'a> {
    seed: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    full_size: usize,
    full_graph: Arc<MarkedGraph<MultiGraphState>>,
}

#[derive(Debug)]
pub(super) struct CandidateBuildResult<'a> {
    pub(super) mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    graph_to_full_region: Vec<Option<usize>>,
}

#[derive(Debug)]
pub(super) struct CandidateSeed<'a> {
    pub(super) path_indices: Vec<usize>,
    pub(super) layout: InterpolationLayout<'a>,
    pub(super) seed_mgts: MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
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
        automaton: &'a ImplicitCFGProduct,
        dimension: usize,
    ) -> Vec<CandidateSeed<'a>> {
        let sccs = Arc::new(PrecomputedSccs::from_reachable(
            automaton,
            automaton.initial(),
        ));
        let classifier = SccClassifier::new(Arc::clone(&sccs));
        let primary = segment_path(primary_path, &classifier);
        let mut compatible_auxiliaries = auxiliary_paths
            .iter()
            .enumerate()
            .filter_map(|(auxiliary_index, path)| {
                let segmented = segment_path(path, &classifier);

                (segmented.signature == primary.signature)
                    .then_some((auxiliary_index + 1, segmented))
            })
            .collect::<Vec<_>>();

        compatible_auxiliaries
            .sort_by_key(|(_, segmented)| std::cmp::Reverse(segmented_path_size(segmented)));

        candidate_auxiliary_counts(compatible_auxiliaries.len())
            .into_iter()
            .filter_map(|auxiliary_count| {
                let group = std::iter::once((0, primary.clone()))
                    .chain(compatible_auxiliaries.iter().take(auxiliary_count).cloned())
                    .collect();

                Self::from_segmented_group(group, automaton, dimension, Arc::clone(&sccs))
            })
            .map(|(path_indices, layout)| {
                let seed_mgts = layout.build_seed_mgts();
                CandidateSeed {
                    path_indices,
                    layout,
                    seed_mgts,
                }
            })
            .collect()
    }

    /// Converts one compatible group of segmented paths into a concrete
    /// interpolation layout.
    ///
    /// Fixed segments stay exact or become a shared path-union graph. Region
    /// segments keep a seed MGTS plus one shared full-SCC graph so candidate
    /// builds can toggle each region without rebuilding graph payloads.
    fn from_segmented_group(
        group: Vec<(usize, SegmentedPath)>,
        automaton: &'a ImplicitCFGProduct,
        dimension: usize,
        sccs: Arc<PrecomputedSccs<MultiGraphState>>,
    ) -> Option<(Vec<usize>, Self)> {
        let first = group.first()?.1.clone();
        let mut path_indices = Vec::with_capacity(group.len());
        let mut items = Vec::new();
        let mut regions = Vec::new();

        for (path_index, _) in &group {
            path_indices.push(*path_index);
        }

        for segment_index in 0..first.segments.len() {
            match &first.segments[segment_index] {
                PathSegment::Fixed(_) => {
                    let paths = group
                        .iter()
                        .map(|(_, segmented)| match &segmented.segments[segment_index] {
                            PathSegment::Fixed(path) => path.clone(),
                            PathSegment::Region { .. } => unreachable!("signature mismatch"),
                        })
                        .collect::<Vec<_>>();

                    items.push(build_fixed_item(automaton, &paths));
                }
                PathSegment::Region { component, path } => {
                    let paths = group
                        .iter()
                        .map(|(_, segmented)| match &segmented.segments[segment_index] {
                            PathSegment::Region {
                                component: other_component,
                                path,
                            } => {
                                assert_eq!(component, other_component);
                                path.clone()
                            }
                            PathSegment::Fixed(_) => unreachable!("signature mismatch"),
                        })
                        .collect::<Vec<_>>();

                    let seed = build_seed_region(automaton, dimension, &paths);
                    let full_component = sccs.component(*component);
                    let full_size = full_component.nodes.len();
                    let full_graph = Arc::new(MarkedGraph::from_subset(
                        automaton,
                        &full_component.nodes,
                        path.start().clone(),
                        path.end().clone(),
                    ));

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
    pub(super) fn build_seed_mgts(&self) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
        let mask = vec![false; self.regions.len()];
        self.build_candidate(&mask).mgts
    }

    /// Materializes a candidate MGTS from a region mask and records which graph
    /// parts correspond to full regions.
    pub(super) fn build_candidate(&self, mask: &[bool]) -> CandidateBuildResult<'a> {
        let mut mgts = MGTS::empty(self.automaton, self.dimension);
        let mut graph_to_full_region = Vec::new();

        for item in &self.items {
            match item {
                InterpolationItem::FixedPath(path) => {
                    mgts.add_path(path.clone().into());
                }
                InterpolationItem::FixedGraph(graph) => {
                    mgts.add_graph(Arc::clone(graph));
                    graph_to_full_region.push(None);
                }
                InterpolationItem::Region(region_index) => {
                    let region = &self.regions[*region_index];

                    if mask[*region_index] {
                        mgts.add_graph(Arc::clone(&region.full_graph));
                        graph_to_full_region.push(Some(*region_index));
                    } else {
                        append_mgts(&mut mgts, &region.seed, &mut graph_to_full_region);
                    }
                }
            }
        }

        mgts.assert_consistent();

        CandidateBuildResult {
            mgts,
            graph_to_full_region,
        }
    }
}

fn build_fixed_item(automaton: &ImplicitCFGProduct, paths: &[MultiGraphPath]) -> InterpolationItem {
    if paths.iter().all(|path| path == &paths[0]) {
        return InterpolationItem::FixedPath(paths[0].clone());
    }

    InterpolationItem::FixedGraph(Arc::new(MarkedGraph::from_path_union(automaton, paths)))
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
    automaton: &'a ImplicitCFGProduct,
    dimension: usize,
    paths: &[MultiGraphPath],
) -> MGTS<'a, MultiGraphState, ImplicitCFGProduct> {
    if paths.len() == 1 {
        return MGTS::from_path_roll_up(paths[0].clone(), automaton, dimension);
    }

    let all_empty = paths.iter().all(Path::is_empty);
    let unique_nodes = Path::<MultiGraphState, CFGCounterUpdate>::sorted_union_states(paths);
    if all_empty && unique_nodes.len() <= 1 {
        return MGTS::empty(automaton, dimension);
    }

    let mut mgts = MGTS::empty(automaton, dimension);
    mgts.add_graph(MarkedGraph::from_subset(
        automaton,
        &unique_nodes,
        paths[0].start().clone(),
        paths[0].end().clone(),
    ));
    mgts
}

impl SccRegion<'_> {
    pub(super) fn gain(&self) -> usize {
        self.full_size.saturating_sub(self.seed.size())
    }
}

impl CandidateBuildResult<'_> {
    pub(super) fn used_full_regions(&self, solution: &MGTSSolution) -> HashSet<usize> {
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

fn append_mgts<'a>(
    target: &mut MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    source: &MGTS<'a, MultiGraphState, ImplicitCFGProduct>,
    graph_to_full_region: &mut Vec<Option<usize>>,
) {
    for part in &source.sequence {
        match part {
            MGTSPart::Path(path_index) => {
                target.add_path(source.path(*path_index).clone());
            }
            MGTSPart::Graph(graph_index) => {
                target.add_graph(Arc::clone(&source.graphs[*graph_index]));
                graph_to_full_region.push(None);
            }
        }
    }
}
