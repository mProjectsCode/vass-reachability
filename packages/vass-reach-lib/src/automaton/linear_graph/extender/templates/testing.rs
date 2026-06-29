//! Stable wrappers used by integration tests.
//!
//! The production template types remain internal to the extender. These helpers
//! expose coefficient vectors and state-bound snapshots so integration tests
//! can cover the algorithm described in `docs/linear-template-invariants.md`
//! without making the full invariant domain part of the public API.

use petgraph::graph::NodeIndex;

use super::{
    LinearTemplate, MainCFGTemplateLowerBounds,
    analysis::{
        analyze_templates, analyze_with_incremental_template, default_templates,
        main_cfg_template_lower_bounds, successor_bounds,
    },
    synthesis::{
        TemplateSynthesisOptions, candidate_templates, synthesize_template_for_boundaries,
    },
    transfer::exact_successor_template_bound,
};
use crate::{
    automaton::{
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        vass::counter::VASSCounterValuation,
    },
    config::LinearGraphTemplateFamily,
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
    TemplateTestCodec::snapshot(main_cfg_template_lower_bounds(
        cfg,
        initial_valuation,
        true,
        usize::MAX,
        &DefaultTemplateFamilies::all(),
    ))
}

pub fn default_template_coefficients(dimension: usize) -> Vec<Vec<i32>> {
    TemplateTestCodec::coefficients(default_templates(
        dimension,
        &DefaultTemplateFamilies::all(),
    ))
}

pub fn default_template_coefficients_with_families(
    dimension: usize,
    families: &[LinearGraphTemplateFamily],
) -> Vec<Vec<i32>> {
    TemplateTestCodec::coefficients(default_templates(dimension, families))
}

pub fn candidate_template_coefficients(
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &[Vec<i32>],
) -> Vec<Vec<i32>> {
    let existing = TemplateTestCodec::templates_from_coefficients(existing);
    TemplateTestCodec::coefficients(candidate_templates(
        dimension,
        max_coefficient,
        max_candidates,
        &existing,
    ))
}

pub fn guided_candidate_template_coefficients(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
) -> Vec<Vec<i32>> {
    let current = main_cfg_template_lower_bounds(
        cfg,
        initial_valuation,
        true,
        usize::MAX,
        &DefaultTemplateFamilies::all(),
    );
    TemplateTestCodec::coefficients(super::synthesis::candidate_templates_for_boundaries(
        initial_valuation.dimension(),
        max_coefficient,
        max_candidates,
        &current,
        model_boundaries,
    ))
}

pub fn exact_successor_bound_from_coefficients(
    templates: &[Vec<i32>],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
) -> i32 {
    let templates = TemplateTestCodec::templates_from_coefficients(templates);
    exact_successor_template_bound(&templates, source_bounds, update, objective_index, cap)
}

pub fn successor_bound_from_coefficients_with_exact_transfer(
    templates: &[Vec<i32>],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
    exact_transfer_enabled: bool,
) -> i32 {
    successor_bound_from_coefficients_with_exact_transfer_limit(
        templates,
        source_bounds,
        update,
        objective_index,
        cap,
        exact_transfer_enabled,
        usize::MAX,
    )
}

pub fn successor_bound_from_coefficients_with_exact_transfer_limit(
    templates: &[Vec<i32>],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
    exact_transfer_enabled: bool,
    exact_transfer_max_templates: usize,
) -> i32 {
    let templates = TemplateTestCodec::templates_from_coefficients(templates);
    successor_bounds(
        &templates,
        source_bounds,
        update,
        cap,
        exact_transfer_enabled,
        exact_transfer_max_templates,
    )[objective_index]
}

pub fn analyze_template_bounds_snapshot(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: &[Vec<i32>],
) -> TemplateAnalysisSnapshot {
    TemplateTestCodec::snapshot(analyze_templates(
        cfg,
        initial_valuation,
        TemplateTestCodec::templates_from_coefficients(templates),
        true,
        usize::MAX,
    ))
}

pub fn analyze_incremental_template_bounds_snapshot(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current_templates: &[Vec<i32>],
    extra_template: Vec<i32>,
) -> TemplateAnalysisSnapshot {
    let current = analyze_templates(
        cfg,
        initial_valuation,
        TemplateTestCodec::templates_from_coefficients(current_templates),
        true,
        usize::MAX,
    );
    TemplateTestCodec::snapshot(analyze_with_incremental_template(
        cfg,
        initial_valuation,
        &current,
        LinearTemplate::from_coefficients(extra_template),
        true,
        usize::MAX,
    ))
}

pub fn synthesize_template_coefficients(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
) -> Option<Vec<i32>> {
    let current = main_cfg_template_lower_bounds(
        cfg,
        initial_valuation,
        true,
        usize::MAX,
        &DefaultTemplateFamilies::all(),
    );
    synthesize_template_for_boundaries(
        cfg,
        initial_valuation,
        &current,
        model_boundaries,
        TemplateSynthesisOptions {
            max_coefficient,
            max_candidates,
            exact_transfer_enabled: true,
            exact_transfer_max_templates: usize::MAX,
        },
    )
    .map(|(template, _)| template.coefficients.into_vec())
}

struct TemplateTestCodec;

impl TemplateTestCodec {
    fn snapshot(analysis: MainCFGTemplateLowerBounds) -> TemplateAnalysisSnapshot {
        TemplateAnalysisSnapshot {
            templates: Self::coefficients(analysis.templates),
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
}

struct DefaultTemplateFamilies;

impl DefaultTemplateFamilies {
    fn all() -> Vec<LinearGraphTemplateFamily> {
        vec![
            LinearGraphTemplateFamily::Singleton,
            LinearGraphTemplateFamily::Pair,
            LinearGraphTemplateFamily::All,
        ]
    }
}
