//! Counterexample-guided synthesis of additional weighted templates.
//!
//! Synthesis runs after a LinearGraph SMT model cannot be turned into an
//! concrete N-run. Candidate coefficient vectors are accepted only after the
//! forward analysis proves a bound that excludes at least one modeled boundary
//! valuation.

use petgraph::graph::NodeIndex;

use super::{LinearTemplate, MainCFGTemplateLowerBounds, analysis::analyze_templates};
use crate::automaton::{cfg::vasscfg::VASSCFG, vass::counter::VASSCounterValuation};

pub(in crate::automaton::linear_graph::extender) fn synthesize_template_for_boundaries(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current: &MainCFGTemplateLowerBounds,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
) -> Option<(LinearTemplate, MainCFGTemplateLowerBounds)> {
    // The initial domain already contains singleton templates. Synthesis looks
    // only for relational templates and stops at the first candidate that cuts
    // the current spurious model.
    candidate_templates(
        initial_valuation.dimension(),
        max_coefficient,
        max_candidates,
        &current.templates,
    )
    .into_iter()
    .find_map(|template| {
        let analysis = analyze_with_extra_template(cfg, initial_valuation, current, &template);
        template_excludes_model(&analysis, &template, model_boundaries)
            .then_some((template, analysis))
    })
}

pub(super) fn candidate_templates(
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &[LinearTemplate],
) -> Vec<LinearTemplate> {
    // Enumeration order is intentionally simple and deterministic. The caller
    // caps the number of candidates so synthesis remains a bounded refinement
    // step rather than an exhaustive template search.
    let mut enumerator = CandidateTemplateEnumerator {
        coefficients: vec![0; dimension],
        max_coefficient,
        max_candidates,
        existing,
        candidates: Vec::new(),
    };
    enumerator.enumerate_from(0);
    enumerator.candidates
}

fn analyze_with_extra_template(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current: &MainCFGTemplateLowerBounds,
    template: &LinearTemplate,
) -> MainCFGTemplateLowerBounds {
    let mut templates = current.templates.clone();
    templates.push(template.clone());
    analyze_templates(cfg, initial_valuation, templates)
}

fn template_excludes_model(
    analysis: &MainCFGTemplateLowerBounds,
    template: &LinearTemplate,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
) -> bool {
    let template_index = analysis.templates.len() - 1;

    // A candidate is useful only when the recomputed fixed point proves a
    // stronger bound than the model valuation satisfies at some boundary.
    model_boundaries.iter().any(|(state, valuation)| {
        analysis
            .state_bounds(*state)
            .is_some_and(|bounds| template.value(valuation) < bounds[template_index])
    })
}

struct CandidateTemplateEnumerator<'a> {
    coefficients: Vec<i32>,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &'a [LinearTemplate],
    candidates: Vec<LinearTemplate>,
}

impl CandidateTemplateEnumerator<'_> {
    fn enumerate_from(&mut self, position: usize) {
        if self.candidates.len() >= self.max_candidates {
            return;
        }

        if position == self.coefficients.len() {
            self.push_current_candidate();
            return;
        }

        for coefficient in 0..=self.max_coefficient {
            self.coefficients[position] = coefficient;
            self.enumerate_from(position + 1);
        }
    }

    fn push_current_candidate(&mut self) {
        if !self.current_has_relational_support() || self.current_already_exists() {
            return;
        }

        self.candidates
            .push(LinearTemplate::from_coefficients(self.coefficients.clone()));
    }

    fn current_has_relational_support(&self) -> bool {
        // Singleton templates are part of the default domain, and the all-zero
        // vector carries no information.
        self.coefficients
            .iter()
            .filter(|coefficient| **coefficient != 0)
            .count()
            >= 2
    }

    fn current_already_exists(&self) -> bool {
        self.existing
            .iter()
            .any(|template| template.coefficients.as_ref() == self.coefficients)
    }
}
