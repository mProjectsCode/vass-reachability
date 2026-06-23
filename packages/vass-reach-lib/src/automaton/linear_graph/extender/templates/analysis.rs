//! Forward fixed-point analysis for template lower bounds.
//!
//! This module implements the "Abstract Domain", "Initial Bounds", "Exact SMT
//! Transfer", "Joining Control-Flow Paths", and "LinearGraph Integration"
//! sections of `docs/linear-template-invariants.md`.

use std::collections::VecDeque;

use hashbrown::HashMap;

use super::{
    super::ProductViewLinearGraph, LinearTemplate, MainCFGTemplateLowerBounds,
    transfer::ExactTemplateTransfer,
};
use crate::{
    automaton::{
        Alphabet, Automaton, ExplicitEdgeAutomaton, InitializedAutomaton, TransitionSystem,
        cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
        implicit_cfg_product::state::MultiGraphState,
        linear_graph::part::{LinearGraphPart, LinearGraphRegion},
        vass::counter::VASSCounterValuation,
    },
    config::LinearGraphTemplateFamily,
    solver::linear_graph_reach::{
        LinearGraphBoundPoint, LinearGraphBoundaryConstraints, LinearTemplateLowerBound,
    },
};

pub(in crate::automaton::linear_graph::extender) fn main_cfg_template_lower_bounds(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    exact_transfer_enabled: bool,
    initial_template_families: &[LinearGraphTemplateFamily],
) -> MainCFGTemplateLowerBounds {
    TemplateAnalysis::new(
        cfg,
        initial_valuation,
        default_templates(initial_valuation.dimension(), initial_template_families),
        exact_transfer_enabled,
    )
    .run()
}

pub(super) fn analyze_templates(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    templates: Vec<LinearTemplate>,
    exact_transfer_enabled: bool,
) -> MainCFGTemplateLowerBounds {
    TemplateAnalysis::new(cfg, initial_valuation, templates, exact_transfer_enabled).run()
}

pub(super) fn analyze_with_incremental_template(
    cfg: &VASSCFG<()>,
    initial_valuation: &VASSCounterValuation,
    current: &MainCFGTemplateLowerBounds,
    template: LinearTemplate,
    exact_transfer_enabled: bool,
) -> MainCFGTemplateLowerBounds {
    IncrementalTemplateAnalysis::new(
        cfg,
        initial_valuation,
        current,
        template,
        exact_transfer_enabled,
    )
    .run()
}

/// Builds the small default domain: singleton counters, pairwise sums, and
/// the all-counter sum for dimensions larger than two.
pub(super) fn default_templates(
    dimension: usize,
    families: &[LinearGraphTemplateFamily],
) -> Vec<LinearTemplate> {
    DefaultTemplateDomain::new(dimension, families).templates()
}

pub(in crate::automaton::linear_graph::extender) fn path_sensitive_linear_graph_template_lower_bounds(
    linear_graph: &ProductViewLinearGraph<'_>,
    main_bounds: &MainCFGTemplateLowerBounds,
    initial_valuation: &VASSCounterValuation,
    final_valuation: &VASSCounterValuation,
    exact_transfer_enabled: bool,
) -> HashMap<LinearGraphBoundPoint<MultiGraphState>, LinearGraphBoundaryConstraints> {
    LinearGraphTemplateBounder::new(
        linear_graph,
        main_bounds,
        initial_valuation,
        final_valuation,
        exact_transfer_enabled,
    )
    .boundary_constraints()
}

struct TemplateAnalysis<'a> {
    cfg: &'a VASSCFG<()>,
    initial_valuation: &'a VASSCounterValuation,
    templates: Vec<LinearTemplate>,
    cap: i32,
    transfer: TemplateTransfer,
}

impl<'a> TemplateAnalysis<'a> {
    fn new(
        cfg: &'a VASSCFG<()>,
        initial_valuation: &'a VASSCounterValuation,
        templates: Vec<LinearTemplate>,
        exact_transfer_enabled: bool,
    ) -> Self {
        Self {
            cfg,
            initial_valuation,
            templates,
            cap: AnalysisCap::for_cfg(cfg),
            transfer: TemplateTransfer::new(exact_transfer_enabled),
        }
    }

    fn run(self) -> MainCFGTemplateLowerBounds {
        let mut state_bounds = self.initial_state_bounds();

        // Worklist propagation computes the greatest lower bounds representable by
        // this finite capped domain. Joins use min, so every stored fact remains
        // valid for all incoming control-flow paths.
        self.propagate_bounds(&mut state_bounds);

        MainCFGTemplateLowerBounds::new(self.templates, state_bounds)
    }

    fn initial_state_bounds(&self) -> Vec<Option<Box<[i32]>>> {
        let mut state_bounds = vec![None; self.cfg.node_count()];
        state_bounds[self.cfg.get_initial().index()] = Some(TemplateBounds::for_valuation(
            &self.templates,
            self.initial_valuation,
            self.cap,
        ));
        state_bounds
    }

    fn propagate_bounds(&self, state_bounds: &mut [Option<Box<[i32]>>]) {
        let initial = self.cfg.get_initial();
        let mut queue = VecDeque::from([initial]);
        let mut queued = vec![false; self.cfg.node_count()];
        queued[initial.index()] = true;

        while let Some(source) = queue.pop_front() {
            queued[source.index()] = false;
            let source_bounds = state_bounds[source.index()]
                .as_ref()
                .expect("queued states have a lower bound")
                .clone();

            for update in self.cfg.alphabet() {
                let Some(target) = self.cfg.successor(&source, update) else {
                    continue;
                };

                let candidate = self.transfer.successor_bounds(
                    &self.templates,
                    &source_bounds,
                    update,
                    self.cap,
                );

                if TemplateBounds::merge_state(&mut state_bounds[target.index()], candidate)
                    && !queued[target.index()]
                {
                    queued[target.index()] = true;
                    queue.push_back(target);
                }
            }
        }
    }
}

struct AnalysisCap;

impl AnalysisCap {
    fn for_cfg(cfg: &VASSCFG<()>) -> i32 {
        // Clamping makes the abstract domain finite and only weakens each lower bound.
        Self::for_size(cfg.node_count())
    }

    fn for_size(size: usize) -> i32 {
        i32::try_from(size).unwrap_or(i32::MAX)
    }
}

pub(super) fn successor_bounds(
    templates: &[LinearTemplate],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    cap: i32,
    exact_transfer_enabled: bool,
) -> Box<[i32]> {
    TemplateTransfer::new(exact_transfer_enabled).successor_bounds(
        templates,
        source_bounds,
        update,
        cap,
    )
}

struct IncrementalTemplateAnalysis<'a> {
    cfg: &'a VASSCFG<()>,
    initial_valuation: &'a VASSCounterValuation,
    current: &'a MainCFGTemplateLowerBounds,
    templates: Vec<LinearTemplate>,
    new_template_index: usize,
    cap: i32,
    transfer: TemplateTransfer,
}

impl<'a> IncrementalTemplateAnalysis<'a> {
    fn new(
        cfg: &'a VASSCFG<()>,
        initial_valuation: &'a VASSCounterValuation,
        current: &'a MainCFGTemplateLowerBounds,
        template: LinearTemplate,
        exact_transfer_enabled: bool,
    ) -> Self {
        let cap = AnalysisCap::for_cfg(cfg);
        let mut templates = current.templates.clone();
        templates.push(template);
        let new_template_index = templates.len() - 1;

        Self {
            cfg,
            initial_valuation,
            current,
            templates,
            new_template_index,
            cap,
            transfer: TemplateTransfer::new(exact_transfer_enabled),
        }
    }

    fn run(self) -> MainCFGTemplateLowerBounds {
        let new_template_bounds = self.fixed_point();
        let state_bounds = self
            .current
            .state_bounds
            .iter()
            .zip(new_template_bounds)
            .map(|(current_bounds, new_bound)| {
                current_bounds.as_ref().map(|current_bounds| {
                    let mut bounds = current_bounds.to_vec();
                    bounds.push(new_bound.unwrap_or(0));
                    bounds.into_boxed_slice()
                })
            })
            .collect();

        MainCFGTemplateLowerBounds::new(self.templates, state_bounds)
    }

    fn fixed_point(&self) -> Vec<Option<i32>> {
        let initial = self.cfg.get_initial();
        let mut new_bounds = vec![None; self.cfg.node_count()];
        new_bounds[initial.index()] = Some(
            self.new_template()
                .clamp_lower_bound(self.new_template().value(self.initial_valuation), self.cap),
        );

        let mut queue = VecDeque::from([initial]);
        let mut queued = vec![false; self.cfg.node_count()];
        queued[initial.index()] = true;

        while let Some(source) = queue.pop_front() {
            queued[source.index()] = false;

            let Some(source_bounds) = self.source_bounds(&new_bounds, source) else {
                continue;
            };

            for update in self.cfg.alphabet() {
                let Some(target) = self.cfg.successor(&source, update) else {
                    continue;
                };

                let candidate = self.transfer.successor_template_bound(
                    &self.templates,
                    &source_bounds,
                    update,
                    self.new_template_index,
                    self.cap,
                );

                if TemplateBounds::merge_template(&mut new_bounds[target.index()], candidate)
                    && !queued[target.index()]
                {
                    queued[target.index()] = true;
                    queue.push_back(target);
                }
            }
        }

        new_bounds
    }

    fn source_bounds(
        &self,
        new_bounds: &[Option<i32>],
        source: petgraph::graph::NodeIndex,
    ) -> Option<Box<[i32]>> {
        let mut source_bounds = self.current.state_bounds(source)?.to_vec();
        source_bounds.push(
            self.new_template()
                .clamp_lower_bound(new_bounds[source.index()]?, self.cap),
        );
        Some(source_bounds.into_boxed_slice())
    }

    fn new_template(&self) -> &LinearTemplate {
        &self.templates[self.new_template_index]
    }
}

#[derive(Debug, Clone, Copy)]
struct TemplateTransfer {
    exact_transfer_enabled: bool,
}

impl TemplateTransfer {
    fn new(exact_transfer_enabled: bool) -> Self {
        Self {
            exact_transfer_enabled,
        }
    }

    fn successor_bounds(
        self,
        templates: &[LinearTemplate],
        source_bounds: &[i32],
        update: &CFGCounterUpdate,
        cap: i32,
    ) -> Box<[i32]> {
        let exact_transfer = (self.exact_transfer_enabled && !templates.is_empty())
            .then(|| ExactTemplateTransfer::new(templates, source_bounds, update));

        (0..templates.len())
            .map(|template_index| {
                if let Some(exact_transfer) = &exact_transfer {
                    exact_transfer.successor_template_bound(&templates[template_index], update, cap)
                } else {
                    self.independent_successor_template_bound(
                        &templates[template_index],
                        source_bounds[template_index],
                        update,
                        cap,
                    )
                }
            })
            .collect()
    }

    fn successor_template_bound(
        self,
        templates: &[LinearTemplate],
        source_bounds: &[i32],
        update: &CFGCounterUpdate,
        objective_index: usize,
        cap: i32,
    ) -> i32 {
        if self.exact_transfer_enabled {
            ExactTemplateTransfer::new(templates, source_bounds, update).successor_template_bound(
                &templates[objective_index],
                update,
                cap,
            )
        } else {
            self.independent_successor_template_bound(
                &templates[objective_index],
                source_bounds[objective_index],
                update,
                cap,
            )
        }
    }

    fn independent_successor_template_bound(
        self,
        template: &LinearTemplate,
        source_bound: i32,
        update: &CFGCounterUpdate,
        cap: i32,
    ) -> i32 {
        let counter = update.counter().to_usize();
        let coefficient = template.coefficients[counter];
        let delta = coefficient * update.op();

        template.clamp_lower_bound(source_bound + delta, cap)
    }
}

struct DefaultTemplateDomain<'a> {
    dimension: usize,
    families: &'a [LinearGraphTemplateFamily],
}

impl<'a> DefaultTemplateDomain<'a> {
    fn new(dimension: usize, families: &'a [LinearGraphTemplateFamily]) -> Self {
        Self {
            dimension,
            families,
        }
    }

    fn templates(self) -> Vec<LinearTemplate> {
        self.coefficients()
            .into_iter()
            .map(LinearTemplate::from_coefficients)
            .collect()
    }

    fn coefficients(self) -> Vec<Vec<i32>> {
        let mut coefficients = Vec::new();

        for family in self.families {
            match family {
                LinearGraphTemplateFamily::Singleton => self.add_singletons(&mut coefficients),
                LinearGraphTemplateFamily::Pair => self.add_pairs(&mut coefficients),
                LinearGraphTemplateFamily::All => self.add_all_counter_sum(&mut coefficients),
            }
        }

        Self::deduplicate(coefficients)
    }

    fn add_singletons(&self, coefficients: &mut Vec<Vec<i32>>) {
        coefficients.extend((0..self.dimension).map(|counter| {
            let mut template = vec![0; self.dimension];
            template[counter] = 1;
            template
        }));
    }

    fn add_pairs(&self, coefficients: &mut Vec<Vec<i32>>) {
        coefficients.extend((0..self.dimension).flat_map(|left| {
            (left + 1..self.dimension).map(move |right| {
                let mut template = vec![0; self.dimension];
                template[left] = 1;
                template[right] = 1;
                template
            })
        }));
    }

    fn add_all_counter_sum(&self, coefficients: &mut Vec<Vec<i32>>) {
        if self.dimension > 2 {
            coefficients.push(vec![1; self.dimension]);
        }
    }

    fn deduplicate(templates: Vec<Vec<i32>>) -> Vec<Vec<i32>> {
        let mut unique = Vec::new();
        for template in templates {
            if !unique.contains(&template) {
                unique.push(template);
            }
        }
        unique
    }
}

struct LinearGraphTemplateBounder<'a> {
    linear_graph: &'a ProductViewLinearGraph<'a>,
    main_bounds: &'a MainCFGTemplateLowerBounds,
    initial_valuation: &'a VASSCounterValuation,
    final_valuation: &'a VASSCounterValuation,
    cap: i32,
    transfer: TemplateTransfer,
}

impl<'a> LinearGraphTemplateBounder<'a> {
    fn new(
        linear_graph: &'a ProductViewLinearGraph<'a>,
        main_bounds: &'a MainCFGTemplateLowerBounds,
        initial_valuation: &'a VASSCounterValuation,
        final_valuation: &'a VASSCounterValuation,
        exact_transfer_enabled: bool,
    ) -> Self {
        Self {
            linear_graph,
            main_bounds,
            initial_valuation,
            final_valuation,
            cap: AnalysisCap::for_size(linear_graph.size()),
            transfer: TemplateTransfer::new(exact_transfer_enabled),
        }
    }

    fn boundary_constraints(
        self,
    ) -> HashMap<LinearGraphBoundPoint<MultiGraphState>, LinearGraphBoundaryConstraints> {
        let mut boundary_constraints = HashMap::new();
        self.add_forward_boundary_bounds(&mut boundary_constraints);
        self.add_backward_boundary_bounds(&mut boundary_constraints);
        boundary_constraints
    }

    fn templates(&self) -> &[LinearTemplate] {
        &self.main_bounds.templates
    }

    fn add_forward_boundary_bounds(
        &self,
        boundary_constraints: &mut HashMap<
            LinearGraphBoundPoint<MultiGraphState>,
            LinearGraphBoundaryConstraints,
        >,
    ) {
        let Some(first) = self.linear_graph.sequence.first() else {
            return;
        };

        let mut current_bounds =
            TemplateBounds::for_valuation(self.templates(), self.initial_valuation, self.cap);

        self.insert_boundary_lower_bounds(
            boundary_constraints,
            LinearGraphBoundPoint::Boundary {
                index: 0,
                state: first.start(self.linear_graph).clone(),
            },
            &current_bounds,
        );

        for (index, part) in self.linear_graph.sequence.iter().enumerate() {
            current_bounds = self.transfer_part(part, &current_bounds);

            self.insert_boundary_lower_bounds(
                boundary_constraints,
                LinearGraphBoundPoint::Boundary {
                    index: index + 1,
                    state: part.end(self.linear_graph).clone(),
                },
                &current_bounds,
            );
        }
    }

    fn add_backward_boundary_bounds(
        &self,
        boundary_constraints: &mut HashMap<
            LinearGraphBoundPoint<MultiGraphState>,
            LinearGraphBoundaryConstraints,
        >,
    ) {
        let Some(last) = self.linear_graph.sequence.last() else {
            return;
        };

        let mut current_bounds =
            TemplateBounds::for_valuation(self.templates(), self.final_valuation, self.cap);

        self.insert_boundary_lower_bounds(
            boundary_constraints,
            LinearGraphBoundPoint::Boundary {
                index: self.linear_graph.sequence.len(),
                state: last.end(self.linear_graph).clone(),
            },
            &current_bounds,
        );

        for (index, part) in self.linear_graph.sequence.iter().enumerate().rev() {
            current_bounds = self.transfer_part_backwards(part, &current_bounds);

            self.insert_boundary_lower_bounds(
                boundary_constraints,
                LinearGraphBoundPoint::Boundary {
                    index,
                    state: part.start(self.linear_graph).clone(),
                },
                &current_bounds,
            );
        }
    }

    fn transfer_part(&self, part: &LinearGraphPart, source_bounds: &[i32]) -> Box<[i32]> {
        match part {
            LinearGraphPart::Path(index) => self.transfer_path_updates(
                source_bounds.to_vec().into_boxed_slice(),
                self.linear_graph.path(*index).path.transitions.iter(),
            ),
            LinearGraphPart::Graph(index) => {
                self.transfer_graph_region(self.linear_graph.graph(*index), source_bounds)
            }
            LinearGraphPart::RepeatPath(index) => self.transfer_repeat_path(
                source_bounds,
                self.linear_graph
                    .repeat_path(*index)
                    .path
                    .transitions
                    .iter(),
            ),
        }
    }

    fn transfer_part_backwards(&self, part: &LinearGraphPart, target_bounds: &[i32]) -> Box<[i32]> {
        match part {
            LinearGraphPart::Path(index) => self.transfer_path_updates_backwards(
                target_bounds.to_vec().into_boxed_slice(),
                self.linear_graph.path(*index).path.transitions.iter().rev(),
            ),
            LinearGraphPart::Graph(index) => {
                self.transfer_graph_region_backwards(self.linear_graph.graph(*index), target_bounds)
            }
            LinearGraphPart::RepeatPath(index) => self.transfer_repeat_path_backwards(
                target_bounds,
                self.linear_graph
                    .repeat_path(*index)
                    .path
                    .transitions
                    .iter()
                    .rev(),
            ),
        }
    }

    fn transfer_path_updates<'b>(
        &self,
        mut bounds: Box<[i32]>,
        updates: impl Iterator<Item = &'b CFGCounterUpdate>,
    ) -> Box<[i32]> {
        for update in updates {
            bounds = self
                .transfer
                .successor_bounds(self.templates(), &bounds, update, self.cap);
        }

        bounds
    }

    fn transfer_path_updates_backwards<'b>(
        &self,
        mut bounds: Box<[i32]>,
        updates: impl Iterator<Item = &'b CFGCounterUpdate>,
    ) -> Box<[i32]> {
        for update in updates {
            bounds = self.transfer.successor_bounds(
                self.templates(),
                &bounds,
                &update.reverse(),
                self.cap,
            );
        }

        bounds
    }

    fn transfer_repeat_path<'b>(
        &self,
        source_bounds: &[i32],
        updates: impl Iterator<Item = &'b CFGCounterUpdate> + Clone,
    ) -> Box<[i32]> {
        let mut bounds = source_bounds.to_vec().into_boxed_slice();

        loop {
            let after_one_iteration = self.transfer_path_updates(bounds.clone(), updates.clone());
            let mut joined = bounds.clone();
            let changed = TemplateBounds::merge_into(&mut joined, &after_one_iteration);

            if !changed {
                return joined;
            }

            bounds = joined;
        }
    }

    fn transfer_repeat_path_backwards<'b>(
        &self,
        target_bounds: &[i32],
        updates: impl Iterator<Item = &'b CFGCounterUpdate> + Clone,
    ) -> Box<[i32]> {
        let mut bounds = target_bounds.to_vec().into_boxed_slice();

        loop {
            let before_one_iteration =
                self.transfer_path_updates_backwards(bounds.clone(), updates.clone());
            let mut joined = bounds.clone();
            let changed = TemplateBounds::merge_into(&mut joined, &before_one_iteration);

            if !changed {
                return joined;
            }

            bounds = joined;
        }
    }

    fn transfer_graph_region(
        &self,
        graph: &LinearGraphRegion<MultiGraphState>,
        source_bounds: &[i32],
    ) -> Box<[i32]> {
        let mut state_bounds = vec![None; graph.node_count()];
        state_bounds[graph.start.index()] = Some(source_bounds.to_vec().into_boxed_slice());

        let mut queue = VecDeque::from([graph.start]);
        let mut queued = vec![false; graph.node_count()];
        queued[graph.start.index()] = true;

        while let Some(source) = queue.pop_front() {
            queued[source.index()] = false;

            let source_bounds = state_bounds[source.index()]
                .as_ref()
                .expect("queued graph-region states have a lower bound")
                .clone();

            for edge in graph.outgoing_edge_indices(&source) {
                let update = graph.get_edge_unchecked(&edge);
                let target = graph.edge_target_unchecked(&edge);
                let candidate = self.transfer.successor_bounds(
                    self.templates(),
                    &source_bounds,
                    update,
                    self.cap,
                );

                if TemplateBounds::merge_state(&mut state_bounds[target.index()], candidate)
                    && !queued[target.index()]
                {
                    queued[target.index()] = true;
                    queue.push_back(target);
                }
            }
        }

        state_bounds[graph.end.index()]
            .clone()
            .unwrap_or_else(|| TemplateBounds::bottom(self.templates()).into_boxed_slice())
    }

    fn transfer_graph_region_backwards(
        &self,
        graph: &LinearGraphRegion<MultiGraphState>,
        target_bounds: &[i32],
    ) -> Box<[i32]> {
        let mut state_bounds = vec![None; graph.node_count()];
        state_bounds[graph.end.index()] = Some(target_bounds.to_vec().into_boxed_slice());

        let mut queue = VecDeque::from([graph.end]);
        let mut queued = vec![false; graph.node_count()];
        queued[graph.end.index()] = true;

        while let Some(target) = queue.pop_front() {
            queued[target.index()] = false;

            let target_bounds = state_bounds[target.index()]
                .as_ref()
                .expect("queued graph-region states have a lower bound")
                .clone();

            for edge in graph.incoming_edge_indices(&target) {
                let update = graph.get_edge_unchecked(&edge).reverse();
                let source = graph.edge_source_unchecked(&edge);
                let candidate = self.transfer.successor_bounds(
                    self.templates(),
                    &target_bounds,
                    &update,
                    self.cap,
                );

                if TemplateBounds::merge_state(&mut state_bounds[source.index()], candidate)
                    && !queued[source.index()]
                {
                    queued[source.index()] = true;
                    queue.push_back(source);
                }
            }
        }

        state_bounds[graph.start.index()]
            .clone()
            .unwrap_or_else(|| TemplateBounds::bottom(self.templates()).into_boxed_slice())
    }

    fn insert_boundary_lower_bounds(
        &self,
        boundary_constraints: &mut HashMap<
            LinearGraphBoundPoint<MultiGraphState>,
            LinearGraphBoundaryConstraints,
        >,
        point: LinearGraphBoundPoint<MultiGraphState>,
        bounds: &[i32],
    ) {
        let entry = boundary_constraints.entry(point).or_default();
        entry
            .lower_bounds
            .extend(TemplateBounds::constraints(self.main_bounds, bounds).lower_bounds);
    }
}

struct TemplateBounds;

impl TemplateBounds {
    fn for_valuation(
        templates: &[LinearTemplate],
        valuation: &VASSCounterValuation,
        cap: i32,
    ) -> Box<[i32]> {
        templates
            .iter()
            .map(|template| template.clamp_lower_bound(template.value(valuation), cap))
            .collect()
    }

    fn bottom(templates: &[LinearTemplate]) -> Vec<i32> {
        templates.iter().map(LinearTemplate::bottom_bound).collect()
    }

    fn merge_template(current: &mut Option<i32>, candidate: i32) -> bool {
        let Some(current) = current else {
            *current = Some(candidate);
            return true;
        };

        let previous = *current;
        *current = (*current).min(candidate);
        *current != previous
    }

    fn merge_state(current: &mut Option<Box<[i32]>>, candidate: Box<[i32]>) -> bool {
        let Some(current) = current else {
            *current = Some(candidate);
            return true;
        };

        let previous = current.clone();
        Self::merge_into(current, &candidate);
        *current != previous
    }

    fn merge_into(current: &mut [i32], candidate: &[i32]) -> bool {
        let previous = current.to_vec();
        for (current_bound, candidate_bound) in current.iter_mut().zip(candidate.iter()) {
            *current_bound = (*current_bound).min(*candidate_bound);
        }

        current != previous.as_slice()
    }

    fn constraints(
        main_bounds: &MainCFGTemplateLowerBounds,
        bounds: &[i32],
    ) -> LinearGraphBoundaryConstraints {
        let lower_bounds = main_bounds
            .templates
            .iter()
            .zip(bounds.iter())
            .filter(|(template, bound)| **bound != template.bottom_bound())
            .map(|(template, bound)| LinearTemplateLowerBound {
                coefficients: template.coefficients.clone(),
                bound: *bound,
            })
            .collect::<Vec<_>>();

        LinearGraphBoundaryConstraints { lower_bounds }
    }
}
