use itertools::Itertools;
use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

/// LTC (Loop Transition Chain) is more or less a GTS (Graph Transition System) with only a single loop for the graphs.
/// This implementation is specifically for VASS.
/// Why do we need a subtract and add vector for each element?
/// Because an LTC should already simply the transitions to single loop transitions and single intermediate transitions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LTC {
    pub elements: Vec<LTCElement>,
    pub dimension: usize,
}

impl LTC {
    pub fn new(dimension: usize) -> Self {
        LTC {
            elements: vec![],
            dimension,
        }
    }

    pub fn add_loop(&mut self, loop_subtract: Vec<i32>, loop_add: Vec<i32>) {
        assert!(
            loop_subtract.len() == self.dimension,
            "Loop subtract vector has to have the same dimension as the LTC"
        );
        assert!(
            loop_add.len() == self.dimension,
            "Loop add vector has to have the same dimension as the LTC"
        );

        // if let Some(LTCElement::Loop(_)) = self.elements.last() {
        //     panic!("Cannot have two loops in a row")
        // }

        self.elements
            .push(LTCElement::Loop((loop_subtract, loop_add)));
    }

    pub fn add_transition(&mut self, transition_subtract: Vec<i32>, transition_add: Vec<i32>) {
        assert!(
            transition_subtract.len() == self.dimension,
            "Transition subtract vector has to have the same dimension as the LTC"
        );
        assert!(
            transition_add.len() == self.dimension,
            "Transition add vector has to have the same dimension as the LTC"
        );

        self.elements.push(LTCElement::Transition((
            transition_subtract,
            transition_add,
        )));
    }

    /// Reachability from 0 to 0 in the whole numbers, so intermediate valuations may be negative.
    pub fn reach_z(&self, initial_valuation: &[i32], final_valuation: &[i32]) -> bool {
        self.reach(false, initial_valuation, final_valuation)
    }

    /// Reachability from 0 to 0 in the natural numbers, so no intermediate valuation may be negative.
    pub fn reach_n(&self, initial_valuation: &[i32], final_valuation: &[i32]) -> bool {
        self.reach(true, initial_valuation, final_valuation)
    }

    fn reach(
        &self,
        only_n_counters: bool,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> bool {
        let time = std::time::Instant::now();

        let config = Config::new();
        let ctx = Context::new(&config);
        let solver = Solver::new(&ctx);

        let zero = Int::from_i64(&ctx, 0);

        let mut formula = initial_valuation
            .iter()
            .map(|&x| Int::from_i64(&ctx, x as i64))
            .collect_vec();
        // currently unused, for path extraction later
        let mut loop_variables = vec![];

        for (i, element) in self.elements.iter().enumerate() {
            match element {
                LTCElement::Loop((subtract, add)) => {
                    let loop_variable = Int::new_const(&ctx, i as u32);

                    // for each counter, we subtract the subtract value, then assert that we are positive and add the add value
                    for i in 0..self.dimension {
                        formula[i] =
                            &formula[i] - &Int::from_i64(&ctx, subtract[i] as i64) * &loop_variable;

                        // if we want to solve reach in N, we need to assert after every subtraction that the counters are positive
                        if only_n_counters {
                            solver.assert(&formula[i].ge(&zero));
                        }

                        formula[i] =
                            &formula[i] + &Int::from_i64(&ctx, add[i] as i64) * &loop_variable;
                    }

                    loop_variables.push(loop_variable);
                }
                LTCElement::Transition((subtract, add)) => {
                    // for each counter, we subtract the subtract value, then assert that we are positive and add the add value
                    for i in 0..self.dimension {
                        formula[i] = &formula[i] - &Int::from_i64(&ctx, subtract[i] as i64);

                        // if we want to solve reach in N, we need to assert after every subtraction that the counters are positive
                        if only_n_counters {
                            solver.assert(&formula[i].ge(&zero));
                        }

                        formula[i] = &formula[i] + &Int::from_i64(&ctx, add[i] as i64);
                    }
                }
            }
        }

        for (f, target) in formula.into_iter().zip(
            final_valuation
                .iter()
                .map(|&x| Int::from_i64(&ctx, x as i64)),
        ) {
            solver.assert(&f._eq(&target));
        }

        println!("Solver setup took: {:?}", time.elapsed());

        let result = match solver.check() {
            z3::SatResult::Sat => true,
            z3::SatResult::Unsat => false,
            z3::SatResult::Unknown => panic!("Solver returned unknown"),
        };

        let stats = solver.get_statistics();
        println!("Solver statistics: {:?}", stats);
        println!("Solver took: {:?}", time.elapsed());

        result
    }
}

/// A single element in the LTC.
/// Either a loop or a transition.
/// A loop can be taken a any number of times including zero.
/// A transition must be taken exactly once.
/// The first vector needs to be subtracted from the counters and the second vector needs to be added to the counters.
/// Similar to a firing rule in a Petri net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LTCElement {
    Loop((Vec<i32>, Vec<i32>)),
    Transition((Vec<i32>, Vec<i32>)),
}
