use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};
use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

use super::{
    dfa::{DfaNodeData, DFA},
    nfa::NFA,
    path::Path,
    utils::dyck_transitions_to_ltc_transition,
    vass::dimension_to_cfg_alphabet,
    AutBuild,
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
    pub fn reach_z(&self, initial_valuation: &[i32], final_valuation: &[i32]) -> LTCSolverResult {
        self.reach(false, initial_valuation, final_valuation)
    }

    /// Reachability from 0 to 0 in the natural numbers, so no intermediate valuation may be negative.
    pub fn reach_n(&self, initial_valuation: &[i32], final_valuation: &[i32]) -> LTCSolverResult {
        self.reach(true, initial_valuation, final_valuation)
    }

    fn reach(
        &self,
        only_n_counters: bool,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> LTCSolverResult {
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LTCSolverResult {
    pub result: bool,
    pub duration: std::time::Duration,
}

impl LTCSolverResult {
    pub fn new(result: bool, duration: std::time::Duration) -> Self {
        LTCSolverResult { result, duration }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LTCTranslationElement {
    Path(Vec<EdgeIndex<u32>>),
    Loop(Vec<EdgeIndex<u32>>),
}

impl LTCTranslationElement {
    pub fn path_from_transitions(transitions: Vec<(EdgeIndex<u32>, NodeIndex<u32>)>) -> Self {
        LTCTranslationElement::Path(transitions.iter().map(|(edge, _)| *edge).collect())
    }

    pub fn loop_from_transitions(transitions: Vec<(EdgeIndex<u32>, NodeIndex<u32>)>) -> Self {
        LTCTranslationElement::Loop(transitions.iter().map(|(edge, _)| *edge).collect())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LTCTranslation {
    elements: Vec<LTCTranslationElement>,
}

impl LTCTranslation {
    pub fn new() -> Self {
        LTCTranslation { elements: vec![] }
    }

    pub fn from_path(path: &Path) -> Self {
        let mut stack: Vec<(EdgeIndex<u32>, NodeIndex<u32>)> = vec![];
        let mut ltc_translation = vec![];

        for transition in &path.transitions {
            let existing_pos = stack.iter().position(|x| x.1 == transition.1);

            if let Some(pos) = existing_pos {
                let transition_loop = stack.split_off(pos);
                // push the remaining transitions before the loop
                if !stack.is_empty() {
                    ltc_translation.push(LTCTranslationElement::path_from_transitions(stack));
                }
                // only push the loop if the last element is not the same
                // that just means we ran the last loop again
                let _loop = LTCTranslationElement::loop_from_transitions(transition_loop);
                if ltc_translation.last() != Some(&_loop) {
                    ltc_translation.push(_loop);
                }
                stack = vec![*transition];
            } else {
                stack.push(*transition);
            }
        }

        if !stack.is_empty() {
            if let Some(LTCTranslationElement::Loop(l)) = ltc_translation.last() {
                let edges = stack.iter().map(|(edge, _)| *edge).collect_vec();
                if edges != *l {
                    ltc_translation.push(LTCTranslationElement::Path(edges));
                }
            } else {
                ltc_translation.push(LTCTranslationElement::path_from_transitions(stack));
            }
        }

        LTCTranslation {
            elements: ltc_translation,
        }
    }

    pub fn to_dfa(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32,
    ) -> DFA<(), i32> {
        let mut nfa = NFA::<(), i32>::new(dimension_to_cfg_alphabet(dimension));

        let start = nfa.add_state(DfaNodeData::new(false, ()));
        nfa.set_start(start);
        let mut current_end = start;

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(edges) => {
                    let mut current = nfa.add_state(DfaNodeData::new(false, ()));
                    nfa.add_transition(current_end, current, None);

                    for edge in edges {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current, new, Some(get_edge_weight(*edge)));
                        current = new;
                    }

                    current_end = current;
                }
                LTCTranslationElement::Loop(edges) => {
                    let loop_start = nfa.add_state(DfaNodeData::new(false, ()));
                    let mut current = loop_start;
                    nfa.add_transition(current_end, current, None);

                    // This is the code that would be needed when we deal with double loops in LTCs
                    // let loop_start = current_end;
                    // let mut current = current_end;

                    for edge in edges.iter().take(edges.len() - 1) {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current, new, Some(get_edge_weight(*edge)));
                        current = new;
                    }

                    let last_edge = edges.last().unwrap();
                    nfa.add_transition(current, loop_start, Some(get_edge_weight(*last_edge)));

                    current_end = loop_start;
                }
            }
        }

        nfa.graph[current_end].accepting = true;

        let mut dfa = nfa.determinize_no_state_data();
        dfa.add_failure_state(());
        dfa = dfa.invert();

        dfa
    }

    pub fn to_ltc(&self, dimension: usize, get_edge_weight: impl Fn(EdgeIndex<u32>) -> i32) -> LTC {
        let mut ltc = LTC::new(dimension);

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_transition(min_counters, counters);
                }
                LTCTranslationElement::Loop(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge))
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_loop(min_counters, counters);
                }
            }
        }

        ltc
    }
}

impl Default for LTCTranslation {
    fn default() -> Self {
        Self::new()
    }
}
