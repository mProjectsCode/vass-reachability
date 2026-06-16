//! Stable wrappers used by integration tests.
//!
//! The production template types remain internal to the extender. These helpers
//! expose coefficient vectors and state-bound snapshots so integration tests
//! can cover the algorithm described in `docs/linear-template-invariants.md`
//! without making the full invariant domain part of the public API.

use petgraph::graph::NodeIndex;

use super::{
    LinearTemplate, MainCFGTemplateLowerBounds,
    analysis::{analyze_templates, default_templates, main_cfg_template_lower_bounds},
    synthesis::{candidate_templates, synthesize_template_for_boundaries},
    transfer::exact_successor_template_bound,
};
use crate::automaton::{
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    vass::counter::VASSCounterValuation,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateAnalysisSnapshot {
    pub templates: Vec<Vec<i32>>,
    pub state_bounds: Vec<Option<Vec<i32>>>,
}

pub fn main_cfg_template_lower_bounds_snapshot(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
) -> TemplateAnalysisSnapshot {
    snapshot(main_cfg_template_lower_bounds(cfg, initial_valuation))
}

pub fn default_template_coefficients(dimension: usize) -> Vec<Vec<i32>> {
    coefficients(default_templates(dimension))
}

pub fn candidate_template_coefficients(
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &[Vec<i32>],
) -> Vec<Vec<i32>> {
    let existing = templates_from_coefficients(existing);
    coefficients(candidate_templates(
        dimension,
        max_coefficient,
        max_candidates,
        &existing,
    ))
}

pub fn exact_successor_bound_from_coefficients(
    templates: &[Vec<i32>],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
) -> i32 {
    let templates = templates_from_coefficients(templates);
    exact_successor_template_bound(&templates, source_bounds, update, objective_index, cap)
}

pub fn analyze_template_bounds_snapshot(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: &[Vec<i32>],
) -> TemplateAnalysisSnapshot {
    snapshot(analyze_templates(
        cfg,
        initial_valuation,
        templates_from_coefficients(templates),
    ))
}

pub fn synthesize_template_coefficients(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
) -> Option<Vec<i32>> {
    let current = main_cfg_template_lower_bounds(cfg, initial_valuation);
    synthesize_template_for_boundaries(
        cfg,
        initial_valuation,
        &current,
        model_boundaries,
        max_coefficient,
        max_candidates,
    )
    .map(|(template, _)| template.coefficients.into_vec())
}

fn snapshot(analysis: MainCFGTemplateLowerBounds) -> TemplateAnalysisSnapshot {
    TemplateAnalysisSnapshot {
        templates: coefficients(analysis.templates),
        state_bounds: analysis
            .state_bounds
            .into_iter()
            .map(|bounds| bounds.map(|bounds| bounds.into_vec()))
            .collect(),
    }
}

fn coefficients(templates: Vec<LinearTemplate>) -> Vec<Vec<i32>> {
    templates
        .into_iter()
        .map(|template| template.coefficients.into_vec())
        .collect()
}

fn templates_from_coefficients(coefficients: &[Vec<i32>]) -> Vec<LinearTemplate> {
    coefficients
        .iter()
        .map(|coefficients| LinearTemplate::from_coefficients(coefficients.clone()))
        .collect()
}
