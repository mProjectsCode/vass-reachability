use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};
use z3::{
    ast::{Ast, Int},
    Config, Context, Solver,
};

use super::{
    cfg::CFGCounterUpdate,
    dfa::{DfaNodeData, VASSCFG},
    nfa::NFA,
    path::{Path, TransitionSequence},
    utils::dyck_transitions_to_ltc_transition,
    AutBuild, AutomatonNode,
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

    pub fn add_loop(&mut self, loop_subtract: Box<[i32]>, loop_add: Box<[i32]>) {
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

    pub fn add_transition(&mut self, transition_subtract: Box<[i32]>, transition_add: Box<[i32]>) {
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
        self.reach(false, false, initial_valuation, final_valuation)
    }

    /// Reachability from 0 to 0 in the natural numbers, so no intermediate valuation may be negative.
    pub fn reach_n(&self, initial_valuation: &[i32], final_valuation: &[i32]) -> LTCSolverResult {
        self.reach(true, true, initial_valuation, final_valuation)
    }

    pub fn reach_n_relaxed(
        &self,
        initial_valuation: &[i32],
        final_valuation: &[i32],
    ) -> LTCSolverResult {
        self.reach(true, false, initial_valuation, final_valuation)
    }

    fn reach(
        &self,
        n_reach: bool,
        assert_n_loops: bool,
        initial_valuation: &[i32],
        final_valuation: &[i32],
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
                LTCElement::Loop((subtract, add)) => {
                    let loop_variable = Int::new_const(&ctx, i as u32);
                    solver.assert(&loop_variable.ge(&zero));

                    // for each counter, we subtract the subtract value, then assert that we are positive and add the add value
                    for i in 0..self.dimension {
                        sums[i] =
                            &sums[i] - &Int::from_i64(&ctx, subtract[i] as i64) * &loop_variable;

                        // if we want to solve reach in N, we need to assert after every subtraction that the counters are positive
                        if n_reach && assert_n_loops {
                            solver.assert(&sums[i].ge(&zero));
                        }

                        sums[i] = &sums[i] + &Int::from_i64(&ctx, add[i] as i64) * &loop_variable;
                    }

                    loop_variables.push(loop_variable);
                }
                LTCElement::Transition((subtract, add)) => {
                    // for each counter, we subtract the subtract value, then assert that we are positive and add the add value
                    for i in 0..self.dimension {
                        sums[i] = &sums[i] - &Int::from_i64(&ctx, subtract[i] as i64);

                        // if we want to solve reach in N, we need to assert after every subtraction that the counters are positive
                        if n_reach {
                            solver.assert(&sums[i].ge(&zero));
                        }

                        sums[i] = &sums[i] + &Int::from_i64(&ctx, add[i] as i64);
                    }
                }
            }
        }

        for (sum, target) in sums.into_iter().zip(final_valuation) {
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

/// A single element in the LTC.
/// Either a loop or a transition.
/// A loop can be taken a any number of times including zero.
/// A transition must be taken exactly once.
/// The first vector needs to be subtracted from the counters and the second vector needs to be added to the counters.
/// Similar to a firing rule in a Petri net.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LTCElement {
    Loop((Box<[i32]>, Box<[i32]>)),
    Transition((Box<[i32]>, Box<[i32]>)),
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

#[derive(Debug, Clone, PartialEq, Eq)]
enum LTCTranslationElement {
    Path(TransitionSequence),
    Loop(TransitionSequence),
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
        let mut stack: TransitionSequence = vec![];
        // This is used to track the node where the transition sequence in the `stack` started
        let mut stack_start_node: Option<NodeIndex> = None;
        let mut ltc_translation = vec![];

        for transition in &path.transitions {
            if let Some(last_node) = stack_start_node {
                if transition.1 == last_node {
                    stack.push(*transition);

                    // only push the loop if the last element is not the same
                    // that just means we ran the last loop again
                    let _loop = LTCTranslationElement::Loop(stack);
                    if ltc_translation.last() != Some(&_loop) {
                        // We don't need to update the `stack_start_node` here, because we just did a full loop
                        ltc_translation.push(_loop);
                    }
                    stack = vec![];
                    continue;
                }
            }

            let existing_pos = stack.iter().position(|x| x.1 == transition.1);

            stack.push(*transition);

            if let Some(pos) = existing_pos {
                let transition_loop = stack.split_off(pos + 1);
                // push the remaining transitions before the loop
                if !stack.is_empty() {
                    stack_start_node = Some(stack.last().unwrap().1);
                    ltc_translation.push(LTCTranslationElement::Path(stack));
                }
                if !transition_loop.is_empty() {
                    // only push the loop if the last element is not the same
                    // that just means we ran the last loop again
                    let last = transition_loop.last().unwrap().1;
                    let _loop = LTCTranslationElement::Loop(transition_loop);
                    if ltc_translation.last() != Some(&_loop) {
                        stack_start_node = Some(last);
                        ltc_translation.push(_loop);
                    }
                }

                stack = vec![];
            }
        }

        if !stack.is_empty() {
            if let Some(LTCTranslationElement::Loop(l)) = ltc_translation.last() {
                if stack != *l {
                    ltc_translation.push(LTCTranslationElement::Path(stack));
                }
            } else {
                ltc_translation.push(LTCTranslationElement::Path(stack));
            }
        }

        LTCTranslation {
            elements: ltc_translation,
        }
    }

    pub fn expand<N: AutomatonNode>(self, cfg: &VASSCFG<N>) -> Self {
        let mut new_elements = vec![];

        for translation in self.elements.into_iter() {
            let LTCTranslationElement::Path(transitions) = translation else {
                new_elements.push(translation);
                continue;
            };

            let mut stack = vec![];
            // let last = transitions.pop().expect("Path should not be empty");

            for (edge, node) in transitions {
                stack.push((edge, node));

                let loop_in_node = cfg.find_loop_rooted_in_node(node);

                if let Some(l) = loop_in_node {
                    new_elements.push(LTCTranslationElement::Path(stack));
                    stack = vec![];

                    new_elements.push(LTCTranslationElement::Loop(l.transitions));
                }
            }

            // stack.push(last);
            if !stack.is_empty() {
                new_elements.push(LTCTranslationElement::Path(stack));
            }
        }

        LTCTranslation {
            elements: new_elements,
        }
    }

    pub fn to_dfa(
        &self,
        relaxed: bool,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> VASSCFG<()> {
        let mut nfa = NFA::<(), CFGCounterUpdate>::new(CFGCounterUpdate::alphabet(dimension));

        let start = nfa.add_state(DfaNodeData::new(false, ()));
        nfa.set_start(start);
        let mut current_end = start;

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(edges) => {
                    let start = nfa.add_state(DfaNodeData::new(false, ()));
                    nfa.add_transition(current_end, start, None);
                    current_end = start;

                    for edge in edges {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current_end, new, Some(get_edge_weight(edge.0)));
                        current_end = new;
                    }
                }
                LTCTranslationElement::Loop(edges) => {
                    // CORRECT:
                    // let loop_start = nfa.add_state(DfaNodeData::new(false, ()));
                    // nfa.add_transition(current_end, loop_start, None);
                    // current_end = loop_start;

                    // FALSE: This is the code that would be needed when we deal with double loops in LTCs
                    // but this is something we should do, as it would solve petri nets 3/unknown_15 and 3/unknown_47
                    // let loop_start = current_end;
                    // let mut current = current_end;

                    let loop_start = if relaxed {
                        // don't add a transition to the loop start, so that the loops can be taken in any order
                        current_end
                    } else {
                        // create an empty transition to the loop start, so that the loops have be taken in the order that they are in the LTC
                        let loop_start = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current_end, loop_start, None);
                        current_end = loop_start;
                        loop_start
                    };

                    for edge in edges.iter().take(edges.len() - 1) {
                        let new = nfa.add_state(DfaNodeData::new(false, ()));
                        nfa.add_transition(current_end, new, Some(get_edge_weight(edge.0)));
                        current_end = new;
                    }

                    let last_edge = edges.last().unwrap();
                    nfa.add_transition(current_end, loop_start, Some(get_edge_weight(last_edge.0)));

                    current_end = loop_start;
                }
            }
        }

        nfa.graph[current_end].accepting = true;

        // dbg!(&nfa);

        let mut dfa = nfa.determinize_no_state_data();
        dfa.add_failure_state(());
        dfa = dfa.invert();

        dfa
    }

    pub fn to_ltc(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> LTC {
        let mut ltc = LTC::new(dimension);

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge.0).into())
                        .collect_vec();
                    let (min_counters, counters) =
                        dyck_transitions_to_ltc_transition(&edge_weights, dimension);
                    ltc.add_transition(min_counters, counters);
                }
                LTCTranslationElement::Loop(edges) => {
                    let edge_weights: Vec<i32> = edges
                        .iter()
                        .map(|&edge| get_edge_weight(edge.0).into())
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
