//! Forward fixed-point analysis for template lower bounds.
//!
//! This module implements the "Abstract Domain", "Initial Bounds", "Exact SMT
//! Transfer", "Joining Control-Flow Paths", and "LinearGraph Integration"
//! sections of `docs/linear-template-invariants.md`.

use std::collections::VecDeque;

use hashbrown::HashMap;

use super::{
    super::ProductViewLinearGraph, LinearTemplate, MainCFGTemplateLowerBounds,
    transfer::exact_successor_template_bound,
};
use crate::{
    automaton::{
        Alphabet, Automaton, InitializedAutomaton, TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::state::MultiGraphState,
        vass::counter::VASSCounterValuation,
    },
    solver::linear_graph_reach::LinearTemplateLowerBound,
};

pub(in crate::automaton::linear_graph::extender) fn main_cfg_template_lower_bounds(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
) -> MainCFGTemplateLowerBounds {
    analyze_templates(
        cfg,
        initial_valuation,
        default_templates(initial_valuation.dimension()),
    )
}

pub(super) fn analyze_templates(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: Vec<LinearTemplate>,
) -> MainCFGTemplateLowerBounds {
    let cap = analysis_cap(cfg);
    let mut state_bounds = initial_state_bounds(cfg, initial_valuation, &templates, cap);

    // Worklist propagation computes the greatest lower bounds representable by
    // this finite capped domain. Joins use min, so every stored fact remains
    // valid for all incoming control-flow paths.
    propagate_bounds(cfg, &templates, &mut state_bounds, cap);

    MainCFGTemplateLowerBounds::new(templates, state_bounds)
}

/// Builds the small default domain: singleton counters, pairwise sums, and
/// the all-counter sum for dimensions larger than two.
pub(super) fn default_templates(dimension: usize) -> Vec<LinearTemplate> {
    default_template_supports(dimension)
        .into_iter()
        .map(|support| LinearTemplate::from_support(dimension, &support))
        .collect()
}

pub(in crate::automaton::linear_graph::extender) fn linear_graph_boundary_template_lower_bounds(
    linear_graph: &ProductViewLinearGraph<'_>,
    main_bounds: &MainCFGTemplateLowerBounds,
    main_cfg_index: usize,
) -> HashMap<MultiGraphState, Vec<LinearTemplateLowerBound>> {
    // LinearGraph states are product states. The invariant belongs to the main
    // CFG component, so each boundary state is projected before bounds are
    // passed to the reachability solver.
    linear_graph
        .iter_parts()
        .flat_map(|part| part.iter_nodes(linear_graph))
        .filter_map(|state| {
            let main_state = state.cfg_state(main_cfg_index);
            main_bounds
                .state_bounds(main_state)
                .map(|bounds| (state.clone(), positive_template_bounds(main_bounds, bounds)))
        })
        .collect()
}

fn analysis_cap(cfg: &VASSCFG<()>) -> i32 {
    // Clamping makes the abstract domain finite and only weakens each lower bound.
    i32::try_from(cfg.node_count()).unwrap_or(i32::MAX)
}

fn initial_state_bounds(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: &[LinearTemplate],
    cap: i32,
) -> Vec<Option<Box<[i32]>>> {
    let initial_bound = templates
        .iter()
        .map(|template| template.value(initial_valuation).clamp(0, cap))
        .collect::<Vec<_>>()
        .into_boxed_slice();

    let mut state_bounds = vec![None; cfg.node_count()];
    state_bounds[cfg.get_initial().index()] = Some(initial_bound);
    state_bounds
}

fn propagate_bounds(
    cfg: &VASSCFG<()>,
    templates: &[LinearTemplate],
    state_bounds: &mut [Option<Box<[i32]>>],
    cap: i32,
) {
    let initial = cfg.get_initial();
    let mut queue = VecDeque::from([initial]);
    let mut queued = vec![false; cfg.node_count()];
    queued[initial.index()] = true;

    while let Some(source) = queue.pop_front() {
        queued[source.index()] = false;
        let source_bounds = state_bounds[source.index()]
            .as_ref()
            .expect("queued states have a lower bound")
            .clone();

        for update in cfg.alphabet() {
            let Some(target) = cfg.successor(&source, update) else {
                continue;
            };

            let candidate = successor_bounds(templates, &source_bounds, update, cap);

            // Multiple incoming edges are joined componentwise with min. A
            // smaller lower bound is weaker, which is the conservative fact
            // true for every path reaching `target`.
            if merge_state_bounds(&mut state_bounds[target.index()], candidate)
                && !queued[target.index()]
            {
                queued[target.index()] = true;
                queue.push_back(target);
            }
        }
    }
}

fn successor_bounds(
    templates: &[LinearTemplate],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    cap: i32,
) -> Box<[i32]> {
    // Each target template gets its own exact optimization objective. The hard
    // constraints are the current source-state template facts plus the VASS
    // transition enabling guard.
    (0..templates.len())
        .map(|template_index| {
            exact_successor_template_bound(templates, source_bounds, update, template_index, cap)
        })
        .collect()
}

fn merge_state_bounds(current: &mut Option<Box<[i32]>>, candidate: Box<[i32]>) -> bool {
    let Some(current) = current else {
        *current = Some(candidate);
        return true;
    };

    let previous = current.clone();
    for (current_bound, candidate_bound) in current.iter_mut().zip(candidate.iter()) {
        *current_bound = (*current_bound).min(*candidate_bound);
    }

    *current != previous
}

fn default_template_supports(dimension: usize) -> Vec<Vec<usize>> {
    let single_counters = (0..dimension).map(|counter| vec![counter]);
    let pairs =
        (0..dimension).flat_map(|left| (left + 1..dimension).map(move |right| vec![left, right]));
    let all_counters = (dimension > 2).then(|| (0..dimension).collect::<Vec<_>>());

    single_counters.chain(pairs).chain(all_counters).collect()
}

fn positive_template_bounds(
    main_bounds: &MainCFGTemplateLowerBounds,
    bounds: &[i32],
) -> Vec<LinearTemplateLowerBound> {
    main_bounds
        .templates
        .iter()
        .zip(bounds.iter())
        .filter(|(_, bound)| **bound > 0)
        .map(|(template, bound)| LinearTemplateLowerBound {
            coefficients: template.coefficients.clone(),
            bound: *bound,
        })
        .collect()
}
