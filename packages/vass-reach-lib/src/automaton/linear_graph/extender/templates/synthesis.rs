//! Counterexample-guided synthesis of additional weighted templates.
//!
//! Synthesis runs after a LinearGraph SMT model cannot be turned into an
//! concrete N-run. Candidate coefficient vectors are accepted only after the
//! forward analysis proves a bound that excludes at least one modeled boundary
//! valuation.

use std::cmp::Reverse;

use petgraph::graph::NodeIndex;

use super::{
    LinearTemplate, MainCFGTemplateLowerBounds, analysis::analyze_with_incremental_template,
};
use crate::automaton::{cfg::vasscfg::VASSCFG, vass::counter::VASSCounterValuation};

pub(in crate::automaton::linear_graph::extender) struct TemplateSynthesisOptions {
    pub(in crate::automaton::linear_graph::extender) max_coefficient: i32,
    pub(in crate::automaton::linear_graph::extender) max_candidates: usize,
    pub(in crate::automaton::linear_graph::extender) exact_transfer_enabled: bool,
    pub(in crate::automaton::linear_graph::extender) exact_transfer_max_templates: usize,
}

pub(in crate::automaton::linear_graph::extender) fn synthesize_template_for_boundaries(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current: &MainCFGTemplateLowerBounds,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    options: TemplateSynthesisOptions,
) -> Option<(LinearTemplate, MainCFGTemplateLowerBounds)> {
    TemplateSynthesizer::new(cfg, initial_valuation, current, model_boundaries, options)
        .synthesize()
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
    CandidateTemplateSearch::unguided(dimension, max_coefficient, max_candidates, existing)
        .candidates()
}

pub(super) fn candidate_templates_for_boundaries(
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    current: &MainCFGTemplateLowerBounds,
    model_boundaries: &[(NodeIndex, VASSCounterValuation)],
) -> Vec<LinearTemplate> {
    CandidateTemplateSearch::guided(
        dimension,
        max_coefficient,
        max_candidates,
        current,
        model_boundaries,
    )
    .candidates()
}

struct TemplateSynthesizer<'a> {
    cfg: &'a VASSCFG<()>,
    initial_valuation: &'a VASSCounterValuation,
    current: &'a MainCFGTemplateLowerBounds,
    model_boundaries: &'a [(NodeIndex, VASSCounterValuation)],
    max_coefficient: i32,
    max_candidates: usize,
    exact_transfer_enabled: bool,
    exact_transfer_max_templates: usize,
}

impl<'a> TemplateSynthesizer<'a> {
    fn new(
        cfg: &'a VASSCFG<()>,
        initial_valuation: &'a VASSCounterValuation,
        current: &'a MainCFGTemplateLowerBounds,
        model_boundaries: &'a [(NodeIndex, VASSCounterValuation)],
        options: TemplateSynthesisOptions,
    ) -> Self {
        Self {
            cfg,
            initial_valuation,
            current,
            model_boundaries,
            max_coefficient: options.max_coefficient,
            max_candidates: options.max_candidates,
            exact_transfer_enabled: options.exact_transfer_enabled,
            exact_transfer_max_templates: options.exact_transfer_max_templates,
        }
    }

    fn synthesize(&self) -> Option<(LinearTemplate, MainCFGTemplateLowerBounds)> {
        self.candidate_templates().into_iter().find_map(|template| {
            let analysis = self.analyze_with_extra_template(&template);
            self.template_excludes_model(&analysis, &template)
                .then_some((template, analysis))
        })
    }

    fn candidate_templates(&self) -> Vec<LinearTemplate> {
        // The initial domain already contains singleton templates. Synthesis
        // looks only for relational templates and stops at the first candidate
        // that cuts the current spurious model. Boundary valuations guide the
        // search order so useful sparse weighted templates are more likely to
        // appear before the cap.
        CandidateTemplateSearch::guided(
            self.initial_valuation.dimension(),
            self.max_coefficient,
            self.max_candidates,
            self.current,
            self.model_boundaries,
        )
        .candidates()
    }

    fn analyze_with_extra_template(&self, template: &LinearTemplate) -> MainCFGTemplateLowerBounds {
        analyze_with_incremental_template(
            self.cfg,
            self.initial_valuation,
            self.current,
            template.clone(),
            self.exact_transfer_enabled,
            self.exact_transfer_max_templates,
        )
    }

    fn template_excludes_model(
        &self,
        analysis: &MainCFGTemplateLowerBounds,
        template: &LinearTemplate,
    ) -> bool {
        let template_index = analysis.templates.len() - 1;

        // A candidate is useful only when the recomputed fixed point proves a
        // stronger bound than the model valuation satisfies at some boundary.
        self.model_boundaries.iter().any(|(state, valuation)| {
            analysis
                .state_bounds(*state)
                .is_some_and(|bounds| template.value(valuation) < bounds[template_index])
        })
    }
}

struct CandidateTemplateSearch<'a> {
    dimension: usize,
    max_coefficient: i32,
    max_candidates: usize,
    existing: &'a [LinearTemplate],
    scores: Vec<u64>,
}

impl<'a> CandidateTemplateSearch<'a> {
    fn unguided(
        dimension: usize,
        max_coefficient: i32,
        max_candidates: usize,
        existing: &'a [LinearTemplate],
    ) -> Self {
        Self {
            dimension,
            max_coefficient,
            max_candidates,
            existing,
            scores: vec![0; dimension],
        }
    }

    fn guided(
        dimension: usize,
        max_coefficient: i32,
        max_candidates: usize,
        current: &'a MainCFGTemplateLowerBounds,
        model_boundaries: &[(NodeIndex, VASSCounterValuation)],
    ) -> Self {
        Self {
            dimension,
            max_coefficient,
            max_candidates,
            existing: &current.templates,
            scores: CounterPriorityScorer::new(dimension, current, model_boundaries).scores(),
        }
    }

    fn candidates(&self) -> Vec<LinearTemplate> {
        let mut enumerator = CandidateTemplateEnumerator {
            coefficients: vec![0; self.dimension],
            counter_order: self.counter_order(),
            coefficient_orders: self.coefficient_orders(),
            max_candidates: self.max_candidates,
            existing: self.existing,
            candidates: Vec::new(),
        };
        enumerator.enumerate_from(0);
        enumerator.candidates
    }

    fn counter_order(&self) -> Vec<usize> {
        let mut counter_order = (0..self.dimension).collect::<Vec<_>>();
        counter_order.sort_by_key(|counter| (Reverse(self.scores[*counter]), *counter));
        counter_order
    }

    fn coefficient_orders(&self) -> Vec<Vec<i32>> {
        (0..self.dimension)
            .map(|counter| {
                let coefficients = 0..=self.max_coefficient;
                if self.scores[counter] > 0 {
                    coefficients.rev().collect()
                } else {
                    coefficients.collect()
                }
            })
            .collect()
    }
}

struct CandidateTemplateEnumerator<'a> {
    coefficients: Vec<i32>,
    counter_order: Vec<usize>,
    coefficient_orders: Vec<Vec<i32>>,
    max_candidates: usize,
    existing: &'a [LinearTemplate],
    candidates: Vec<LinearTemplate>,
}

impl CandidateTemplateEnumerator<'_> {
    fn enumerate_from(&mut self, position: usize) {
        if self.candidates.len() >= self.max_candidates {
            return;
        }

        if position == self.counter_order.len() {
            self.push_current_candidate();
            return;
        }

        let counter = self.counter_order[position];
        for index in 0..self.coefficient_orders[counter].len() {
            let coefficient = self.coefficient_orders[counter][index];
            self.coefficients[counter] = coefficient;
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

struct CounterPriorityScorer<'a> {
    dimension: usize,
    current: &'a MainCFGTemplateLowerBounds,
    model_boundaries: &'a [(NodeIndex, VASSCounterValuation)],
}

impl<'a> CounterPriorityScorer<'a> {
    fn new(
        dimension: usize,
        current: &'a MainCFGTemplateLowerBounds,
        model_boundaries: &'a [(NodeIndex, VASSCounterValuation)],
    ) -> Self {
        Self {
            dimension,
            current,
            model_boundaries,
        }
    }

    fn scores(&self) -> Vec<u64> {
        let singleton_indices = self.singleton_template_indices();
        let mut scores = vec![0; self.dimension];

        for (state, valuation) in self.model_boundaries {
            let values = valuation.iter().copied().collect::<Vec<_>>();
            self.add_value_spread_scores(&mut scores, &values);

            let Some(bounds) = self.current.state_bounds(*state) else {
                continue;
            };

            self.add_bound_margin_scores(&mut scores, &values, bounds, &singleton_indices);
        }

        scores
    }

    fn add_value_spread_scores(&self, scores: &mut [u64], values: &[i32]) {
        let max_value = values.iter().copied().max().unwrap_or(0);

        for counter in 0..self.dimension {
            scores[counter] += i32::saturating_sub(max_value, values[counter]).max(0) as u64;
        }
    }

    fn add_bound_margin_scores(
        &self,
        scores: &mut [u64],
        values: &[i32],
        bounds: &[i32],
        singleton_indices: &[Option<usize>],
    ) {
        let margins = singleton_indices
            .iter()
            .enumerate()
            .filter_map(|(counter, template_index)| {
                template_index.map(|template_index| {
                    (
                        counter,
                        i32::saturating_sub(values[counter], bounds[template_index]),
                    )
                })
            })
            .collect::<Vec<_>>();
        let Some(max_margin) = margins.iter().map(|(_, margin)| *margin).max() else {
            return;
        };

        for (counter, margin) in margins {
            scores[counter] += i32::saturating_sub(max_margin, margin).max(0) as u64;
        }
    }

    fn singleton_template_indices(&self) -> Vec<Option<usize>> {
        (0..self.dimension)
            .map(|counter| {
                self.current.templates.iter().position(|template| {
                    template.coefficients[counter] == 1
                        && template
                            .coefficients
                            .iter()
                            .enumerate()
                            .all(|(other, coefficient)| {
                                (other == counter && *coefficient == 1)
                                    || (other != counter && *coefficient == 0)
                            })
                })
            })
            .collect()
    }
}
