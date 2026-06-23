//! Exact SMT transfer for one CFG edge and one objective template.
//!
//! The optimizer minimizes the target template after applying the edge update,
//! subject to source-state template lower bounds, counter non-negativity, and
//! the decrement guard. When Z3 cannot provide a usable optimum, transfer falls
//! back to the template's bottom lower bound.

use z3::{Optimize, SatResult, ast::Int};

use super::LinearTemplate;
use crate::automaton::cfg::update::CFGCounterUpdate;

pub(super) fn exact_successor_template_bound(
    templates: &[LinearTemplate],
    source_bounds: &[i32],
    update: &CFGCounterUpdate,
    objective_index: usize,
    cap: i32,
) -> i32 {
    ExactTemplateTransfer::new(templates, source_bounds, update).successor_template_bound(
        &templates[objective_index],
        update,
        cap,
    )
}

pub(super) struct ExactTemplateTransfer {
    optimizer: Optimize,
    counters: Vec<Int>,
}

impl ExactTemplateTransfer {
    pub(super) fn new(
        templates: &[LinearTemplate],
        source_bounds: &[i32],
        update: &CFGCounterUpdate,
    ) -> Self {
        let optimizer = Optimize::new();
        let counters = Self::z3_counter_variables(templates[0].coefficients.len());

        // These assertions describe an over-approximation of all valuations
        // that can appear before this edge. Objective-specific minimization can
        // then reuse the same hard constraints for every target template.
        Self::assert_non_negative_counters(&optimizer, &counters);
        Self::assert_enabled_update(&optimizer, update, &counters);
        Self::assert_source_bounds(&optimizer, templates, source_bounds, &counters);

        Self {
            optimizer,
            counters,
        }
    }

    pub(super) fn successor_template_bound(
        &self,
        objective_template: &LinearTemplate,
        update: &CFGCounterUpdate,
        cap: i32,
    ) -> i32 {
        self.optimizer.push();
        let objective = Self::successor_objective(objective_template, update, &self.counters);
        self.optimizer.minimize(&objective);
        let value =
            Self::minimized_objective_value(&self.optimizer, objective_template, &objective, cap);
        self.optimizer.pop();
        value
    }

    fn z3_counter_variables(dimension: usize) -> Vec<Int> {
        (0..dimension)
            .map(|counter| Int::new_const(format!("template_transfer_c{counter}")))
            .collect()
    }

    fn assert_non_negative_counters(optimizer: &Optimize, counters: &[Int]) {
        for counter in counters {
            optimizer.assert(counter.ge(Int::from_i64(0)));
        }
    }

    fn assert_enabled_update(optimizer: &Optimize, update: &CFGCounterUpdate, counters: &[Int]) {
        // VASS decrements are enabled only when the decremented counter has credit.
        if update.op() < 0 {
            optimizer.assert(counters[update.counter().to_usize()].ge(Int::from_i64(1)));
        }
    }

    fn assert_source_bounds(
        optimizer: &Optimize,
        templates: &[LinearTemplate],
        source_bounds: &[i32],
        counters: &[Int],
    ) {
        for (template, bound) in templates.iter().zip(source_bounds.iter()) {
            optimizer.assert(
                template
                    .z3_expression(counters)
                    .ge(Int::from_i64(*bound as i64)),
            );
        }
    }

    fn successor_objective(
        objective_template: &LinearTemplate,
        update: &CFGCounterUpdate,
        counters: &[Int],
    ) -> Int {
        objective_template.z3_expression(counters)
            + Int::from_i64(
                (objective_template.coefficients[update.counter().to_usize()] * update.op()) as i64,
            )
    }

    fn minimized_objective_value(
        optimizer: &Optimize,
        template: &LinearTemplate,
        objective: &Int,
        cap: i32,
    ) -> i32 {
        match optimizer.check(&[]) {
            SatResult::Sat => optimizer
                .get_model()
                .and_then(|model| model.eval(objective, true))
                .and_then(|value| value.as_i64())
                .and_then(|value| i32::try_from(value).ok())
                .map(|value| template.clamp_lower_bound(value, cap))
                .unwrap_or_else(|| template.bottom_bound()),
            SatResult::Unsat | SatResult::Unknown => template.bottom_bound(),
        }
    }
}
