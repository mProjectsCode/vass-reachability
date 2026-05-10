use super::MultiGraphPath;
use crate::automaton::{
    cfg::update::CFGCounterUpdate, implicit_cfg_product::state::MultiGraphState, path::Path,
    scc::SccClassifier,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) struct PathSignature {
    pub(super) items: Vec<SignatureItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(super) enum SignatureItem {
    Fixed {
        start: MultiGraphState,
        end: MultiGraphState,
    },
    Region {
        component: usize,
        start: MultiGraphState,
        end: MultiGraphState,
    },
}

#[derive(Debug, Clone)]
pub(super) struct SegmentedPath {
    pub(super) signature: PathSignature,
    pub(super) segments: Vec<PathSegment>,
}

#[derive(Debug, Clone)]
pub(super) enum PathSegment {
    Fixed(MultiGraphPath),
    Region {
        component: usize,
        path: MultiGraphPath,
    },
}

/// Splits a seed path into exact fixed portions and expandable SCC portions.
///
/// Only cyclic SCCs become regions. Singleton acyclic SCCs are folded into the
/// surrounding fixed path.
pub(super) fn segment_path(
    path: &MultiGraphPath,
    classifier: &SccClassifier<MultiGraphState>,
) -> SegmentedPath {
    let components = path
        .states
        .iter()
        .map(|state| classifier.classify(state))
        .collect::<Vec<_>>();

    let mut signature = PathSignature { items: Vec::new() };
    let mut segments = Vec::new();
    let mut current_path = Path::<MultiGraphState, CFGCounterUpdate>::new(path.start().clone());
    let mut state_index = 0usize;

    while state_index + 1 < path.states.len() {
        let component = &components[state_index];

        let mut run_end = state_index;
        while run_end + 1 < path.states.len() && components[run_end + 1] == *component {
            run_end += 1;
        }

        let run = path.slice(state_index..run_end);

        if component.is_trivial() {
            current_path.concat(run);
        } else {
            push_fixed_segment(&mut signature, &mut segments, &mut current_path);

            signature.items.push(SignatureItem::Region {
                component: component.component,
                start: run.start().clone(),
                end: run.end().clone(),
            });
            segments.push(PathSegment::Region {
                component: component.component,
                path: run.clone(),
            });
            current_path = Path::<MultiGraphState, CFGCounterUpdate>::new(run.end().clone());
        }

        if run_end < path.transitions.len() {
            current_path.add(path.transitions[run_end], path.states[run_end + 1].clone());
        }

        state_index = run_end + 1;
    }

    push_fixed_segment(&mut signature, &mut segments, &mut current_path);

    SegmentedPath {
        signature,
        segments,
    }
}

fn push_fixed_segment(
    signature: &mut PathSignature,
    segments: &mut Vec<PathSegment>,
    path: &mut MultiGraphPath,
) {
    if path.is_empty() {
        return;
    }

    signature.items.push(SignatureItem::Fixed {
        start: path.start().clone(),
        end: path.end().clone(),
    });

    let next = Path::<MultiGraphState, CFGCounterUpdate>::new(path.end().clone());
    let fixed = std::mem::replace(path, next);
    segments.push(PathSegment::Fixed(fixed));
}
