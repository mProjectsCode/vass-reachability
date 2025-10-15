use itertools::Itertools;
use z3::{
    Config, Context, Solver,
    ast::{Ast, Bool, Int},
};

use crate::automaton::vass::counter::{VASSCounterUpdate, VASSCounterValuation};

pub mod translation;

pub type LTCCounterUpdate = (VASSCounterUpdate, VASSCounterUpdate);

/// A single element in the LTC.
/// Either a loop or a transition.
/// A loop can be taken a any number of times including zero.
/// A transition must be taken exactly once.
/// The first vector needs to be subtracted from the counters and the second
/// vector needs to be added to the counters. Similar to a firing rule in a
/// Petri net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LTCElement {
    Loops(Vec<LTCCounterUpdate>),
    Transition(LTCCounterUpdate),
}

/// LTC (Loop Transition Chain) is more or less a GTS (Graph Transition System)
/// with only a single loop for the graphs. This implementation is specifically
/// for VASS. Why do we need a subtract vector and an add vector for each
/// element? Because an LTC should already simply the transitions to single loop
/// transitions and single intermediate transitions.
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

    pub fn add_loop(&mut self, loop_subtract: VASSCounterUpdate, loop_add: VASSCounterUpdate) {
        assert_eq!(
            loop_subtract.dimension(),
            self.dimension,
            "Loop subtract update has to have the same dimension as the LTC"
        );
        assert_eq!(
            loop_add.dimension(),
            self.dimension,
            "Loop add update has to have the same dimension as the LTC"
        );

        // if let Some(LTCElement::Loop(_)) = self.elements.last() {
        //     panic!("Cannot have two loops in a row")
        // }

        match self.elements.last_mut() {
            Some(LTCElement::Loops(loops)) => {
                loops.push((loop_subtract, loop_add));
            }
            _ => {
                self.elements
                    .push(LTCElement::Loops(vec![(loop_subtract, loop_add)]));
            }
        }
    }

    pub fn add_transition(
        &mut self,
        transition_subtract: VASSCounterUpdate,
        transition_add: VASSCounterUpdate,
    ) {
        assert_eq!(
            transition_subtract.dimension(),
            self.dimension,
            "Transition subtract update has to have the same dimension as the LTC"
        );
        assert_eq!(
            transition_add.dimension(),
            self.dimension,
            "Transition add update has to have the same dimension as the LTC"
        );

        self.elements.push(LTCElement::Transition((
            transition_subtract,
            transition_add,
        )));
    }

    pub fn add(&mut self, element: LTCElement) {
        self.elements.push(element);
    }

    /// Reachability from 0 to 0 in the whole numbers, so intermediate
    /// valuations may be negative.
    pub fn reach_z(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> LTCSolverResult {
        self.reach(false, false, initial_valuation, final_valuation)
    }

    /// Reachability from 0 to 0 in the natural numbers, so no intermediate
    /// valuation may be negative.
    pub fn reach_n(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> LTCSolverResult {
        self.reach(true, true, initial_valuation, final_valuation)
    }

    pub fn reach_n_relaxed(
        &self,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> LTCSolverResult {
        self.reach(true, false, initial_valuation, final_valuation)
    }

    fn reach(
        &self,
        n_reach: bool,
        assert_n_loops: bool,
        initial_valuation: &VASSCounterValuation,
        final_valuation: &VASSCounterValuation,
    ) -> LTCSolverResult {
        let time = std::time::Instant::now();

        let config = Config::new();
        let ctx = Context::new(&config);
        let solver = Solver::new(&ctx);

        let zero = Int::from_i64(&ctx, 0);

        let mut sums = initial_valuation
            .iter()
            .map(|&x| Int::from_i64(&ctx, x as i64))
            .collect_vec();
        // currently unused, for path extraction later
        let mut loop_variables = vec![];

        for (i, element) in self.elements.iter().enumerate() {
            match element {
                LTCElement::Loops(loops) => {
                    let ls = loops
                        .iter()
                        .enumerate()
                        .map(|(j, _)| Int::new_const(&ctx, format!("{i}_{j}")))
                        .collect_vec();
                    for l in ls.iter() {
                        solver.assert(&l.ge(&zero));
                    }

                    for i in 0..self.dimension {
                        if n_reach {
                            if assert_n_loops {
                                for (j, (subtract, add)) in loops.iter().enumerate() {
                                    let l = &ls[j];
                                    let sub_i = &Int::from_i64(&ctx, subtract[i] as i64);
                                    let add_i = &Int::from_i64(&ctx, add[i] as i64);

                                    // if we want to solve reach in N, we need to assert after every
                                    // subtraction
                                    // that the counters are positive
                                    let lm1 = l - &Int::from_i64(&ctx, 1);

                                    let c1 = &sums[i] - sub_i;
                                    let c2 = &sums[i] - sub_i * l + add_i * &lm1;

                                    solver.assert(&l.ge(&zero).implies(&c1.ge(&zero)));
                                    solver.assert(&l.ge(&zero).implies(&c2.ge(&zero)));

                                    sums[i] = &sums[i] - sub_i * l + add_i * l;
                                }
                            } else {
                                let mut c_in = vec![];
                                let mut c_out = vec![];

                                for (j, (subtract, add)) in loops.iter().enumerate() {
                                    let l = &ls[j];
                                    let sub_i = &Int::from_i64(&ctx, subtract[i] as i64);
                                    let add_i = &Int::from_i64(&ctx, add[i] as i64);

                                    let lm1 = l - &Int::from_i64(&ctx, 1);

                                    let c1 = &sums[i] - sub_i;
                                    let mut c2 = &sums[i] - sub_i * l + add_i * &lm1;

                                    for other in loops.iter().enumerate() {
                                        if other.0 != j {
                                            c2 = &c2
                                                - &Int::from_i64(&ctx, other.1.0[i] as i64)
                                                    * &ls[other.0]
                                                + &Int::from_i64(&ctx, other.1.1[i] as i64)
                                                    * &ls[other.0];
                                        }
                                    }

                                    let c1 = l.ge(&zero).implies(&c1.ge(&zero));
                                    let c2 = l.ge(&zero).implies(&c2.ge(&zero));

                                    c_in.push(c1);
                                    c_out.push(c2);
                                }

                                let c_in = c_in.iter().collect_vec();
                                let c_out = c_out.iter().collect_vec();

                                solver.assert(&Bool::or(&ctx, &c_in));
                                solver.assert(&Bool::or(&ctx, &c_out));

                                for (j, (subtract, add)) in loops.iter().enumerate() {
                                    let l = &ls[j];
                                    let sub_i = &Int::from_i64(&ctx, subtract[i] as i64);
                                    let add_i = &Int::from_i64(&ctx, add[i] as i64);

                                    sums[i] = &sums[i] - sub_i * l + add_i * l;
                                }
                            }
                        } else {
                            for (j, (subtract, add)) in loops.iter().enumerate() {
                                let l = &ls[j];
                                let sub_i = &Int::from_i64(&ctx, subtract[i] as i64);
                                let add_i = &Int::from_i64(&ctx, add[i] as i64);

                                sums[i] = &sums[i] - sub_i * l + add_i * l;
                            }
                        }
                    }

                    loop_variables.extend(ls);
                }
                // LTCElement::Loop((subtract, add)) => {
                //     let loop_variable = Int::new_const(&ctx, i as u32);
                //     solver.assert(&loop_variable.ge(&zero));

                //     // for each counter, we subtract the subtract value, then assert that we are
                //     // positive and add the add value
                //     for i in 0..self.dimension {
                //         let sub_i = &Int::from_i64(&ctx, subtract[i] as i64);
                //         let add_i = &Int::from_i64(&ctx, add[i] as i64);

                //         // if we want to solve reach in N, we need to assert after every
                // subtraction         // that the counters are positive
                //         if n_reach && assert_n_loops {
                //             let lm1 = &loop_variable - &Int::from_i64(&ctx, 1);

                //             let c1 = &sums[i] - sub_i;
                //             let c2 = &sums[i] - sub_i * &loop_variable + add_i * &lm1;
                //             solver.assert(&loop_variable.ge(&zero).implies(&c1.ge(&zero)));
                //             solver.assert(&loop_variable.ge(&zero).implies(&c2.ge(&zero)));
                //         }

                //         sums[i] = &sums[i] - sub_i * &loop_variable + add_i * &loop_variable;
                //     }

                //     loop_variables.push(loop_variable);
                // }
                LTCElement::Transition((subtract, add)) => {
                    // for each counter, we subtract the subtract value, then assert that we are
                    // positive and add the add value
                    for i in 0..self.dimension {
                        sums[i] = &sums[i] - &Int::from_i64(&ctx, subtract[i] as i64);

                        // if we want to solve reach in N, we need to assert after every subtraction
                        // that the counters are positive
                        if n_reach {
                            solver.assert(&sums[i].ge(&zero));
                        }

                        sums[i] = &sums[i] + &Int::from_i64(&ctx, add[i] as i64);
                    }
                }
            }
        }

        for (sum, target) in sums.into_iter().zip(final_valuation.iter()) {
            solver.assert(&sum._eq(&Int::from_i64(&ctx, *target as i64)));
        }

        // println!("Solver setup took: {:?}", time.elapsed());

        let result = match solver.check() {
            z3::SatResult::Sat => true,
            z3::SatResult::Unsat => false,
            z3::SatResult::Unknown => panic!("Solver returned unknown"),
        };

        // let stats = solver.get_statistics();
        // println!("Solver statistics: {:?}", stats);
        // println!("Solver took: {:?}", time.elapsed());

        LTCSolverResult::new(result, time.elapsed())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LTCSolverResult {
    pub result: bool,
    pub duration: std::time::Duration,
}

impl LTCSolverResult {
    pub fn new(result: bool, duration: std::time::Duration) -> Self {
        LTCSolverResult { result, duration }
    }

    pub fn is_success(&self) -> bool {
        self.result
    }

    pub fn is_failure(&self) -> bool {
        !self.result
    }
}
