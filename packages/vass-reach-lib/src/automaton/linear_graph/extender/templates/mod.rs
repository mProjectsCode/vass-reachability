//! Forward linear-template invariants used to strengthen LinearGraph queries.
//!
//! The domain tracks lower bounds for expressions of the form
//! `a_0 c_0 + ... + a_n c_n` at main-CFG states. Bounds are propagated forward
//! from the initial valuation and candidate-local boundary bounds are also
//! propagated backward from the final valuation before reachability checks. See
//! `docs/linear-template-invariants.md` for the full algorithm and soundness
//! argument.

mod analysis;
mod synthesis;
pub mod testing;
mod transfer;

pub(super) use analysis::{
    main_cfg_template_lower_bounds, path_sensitive_linear_graph_template_lower_bounds,
};
pub(super) use synthesis::synthesize_template_for_boundaries;
use z3::ast::Int;

use crate::automaton::vass::counter::VASSCounterValuation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LinearTemplate {
    /// Non-negative coefficients for the counter vector dot product.
    pub(super) coefficients: Box<[i32]>,
}

impl LinearTemplate {
    fn from_coefficients(coefficients: Vec<i32>) -> Self {
        debug_assert!(
            coefficients.iter().all(|coefficient| *coefficient >= 0),
            "signed templates are not currently supported"
        );
        Self {
            coefficients: coefficients.into_boxed_slice(),
        }
    }

    fn value(&self, valuation: &VASSCounterValuation) -> i32 {
        self.coefficients
            .iter()
            .zip(valuation.iter())
            .map(|(coefficient, value)| coefficient * value)
            .sum()
    }

    fn bottom_bound(&self) -> i32 {
        0
    }

    fn clamp_lower_bound(&self, bound: i32, cap: i32) -> i32 {
        bound.clamp(0, cap)
    }

    fn z3_expression(&self, counters: &[Int]) -> Int {
        counters
            .iter()
            .zip(self.coefficients.iter())
            .filter(|(_, coefficient)| **coefficient != 0)
            .fold(Int::from_i64(0), |sum, (counter, coefficient)| {
                sum + counter * Int::from_i64(*coefficient as i64)
            })
    }
}

#[derive(Debug, Clone)]
pub(super) struct MainCFGTemplateLowerBounds {
    /// Template domain shared by every stored state bound vector.
    pub(super) templates: Vec<LinearTemplate>,
    /// `None` means the CFG state has not been reached by the forward analysis.
    /// Otherwise the vector is aligned with `templates`.
    state_bounds: Vec<Option<Box<[i32]>>>,
}

impl MainCFGTemplateLowerBounds {
    fn new(templates: Vec<LinearTemplate>, state_bounds: Vec<Option<Box<[i32]>>>) -> Self {
        Self {
            templates,
            state_bounds,
        }
    }

    fn state_bounds(&self, state: petgraph::graph::NodeIndex) -> Option<&[i32]> {
        self.state_bounds[state.index()].as_deref()
    }
}
