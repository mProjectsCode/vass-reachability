use std::vec;

use itertools::Itertools;
use petgraph::graph::NodeIndex;

use crate::automaton::{
    GIndex, ModifiableAutomaton,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    implicit_cfg_product::{ImplicitCFGProduct, path::MultiGraphPath},
    ltc::{LTC, LTCElement},
    nfa::{NFA, NFAEdge},
    path::{Path, transition_sequence::TransitionSequence},
    utils::cfg_updates_to_counter_updates,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LTCTranslationElement<NIndex: GIndex> {
    Loops(Vec<TransitionSequence<NIndex, CFGCounterUpdate>>),
    Path(TransitionSequence<NIndex, CFGCounterUpdate>),
}

impl<NIndex: GIndex> LTCTranslationElement<NIndex> {
    pub fn to_ltc_element(&self, dimension: usize) -> LTCElement {
        match self {
            LTCTranslationElement::Path(path) => {
                let (min_counters, counters) =
                    cfg_updates_to_counter_updates(path.iter_letters().cloned(), dimension);
                LTCElement::Transition((min_counters, counters))
            }
            LTCTranslationElement::Loops(loops) => {
                let element = loops
                    .iter()
                    .map(|ts| {
                        let (min_counters, counters) =
                            cfg_updates_to_counter_updates(ts.iter_letters().cloned(), dimension);
                        (min_counters, counters)
                    })
                    .collect_vec();
                LTCElement::Loops(element)
            }
        }
    }

    pub fn to_fancy_string(&self) -> String {
        match self {
            LTCTranslationElement::Path(edges) => {
                format!("Path: {}", edges.to_fancy_string())
            }
            LTCTranslationElement::Loops(loops) => {
                format!(
                    "Loop: {}",
                    loops
                        .iter()
                        .map(|x| x.to_fancy_string())
                        .collect_vec()
                        .join(", ")
                )
            }
        }
    }

    pub fn unwrap_path(self) -> TransitionSequence<NIndex, CFGCounterUpdate> {
        match self {
            LTCTranslationElement::Path(path) => path,
            _ => panic!("Expected Path, found {:?}", self),
        }
    }

    pub fn unwrap_loops(self) -> Vec<TransitionSequence<NIndex, CFGCounterUpdate>> {
        match self {
            LTCTranslationElement::Loops(loops) => loops,
            _ => panic!("Expected Loops, found {:?}", self),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LTCTranslation<NIndex: GIndex> {
    elements: Vec<LTCTranslationElement<NIndex>>,
}

impl<NIndex: GIndex> LTCTranslation<NIndex> {
    pub fn new() -> Self {
        LTCTranslation { elements: vec![] }
    }

    // pub fn expand<N: AutomatonNode>(self, cfg: &impl CFG<N, NIndex = NIndex,
    // EIndex = EIndex>) -> Self {     let mut new_elements = vec![];

    //     for translation in self.elements.into_iter() {
    //         let LTCTranslationElement::Path(transitions) = translation else {
    //             new_elements.push(translation);
    //             continue;
    //         };

    //         let mut stack = TransitionSequence::new();
    //         // let last = transitions.pop().expect("Path should not be empty");

    //         for (edge, node) in transitions {
    //             stack.add(edge, node);

    //             let loop_in_node = cfg.find_loop_rooted_in_node(node);

    //             if let Some(l) = loop_in_node {
    //                 new_elements.push(LTCTranslationElement::Path(stack));
    //                 stack = TransitionSequence::new();

    //
    // new_elements.push(LTCTranslationElement::Loops(vec![l.into()]));
    //             }
    //         }

    //         // stack.push(last);
    //         if !stack.is_empty() {
    //             new_elements.push(LTCTranslationElement::Path(stack));
    //         }
    //     }

    //     LTCTranslation {
    //         elements: new_elements,
    //     }
    // }

    pub fn to_dfa(&self, dimension: usize, relaxed: bool) -> VASSCFG<()> {
        let mut nfa = NFA::<(), CFGCounterUpdate>::new(CFGCounterUpdate::alphabet(dimension));

        let start = nfa.add_node(DfaNode::default());
        nfa.set_initial(start);
        let mut current_end = start;

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(elements) => {
                    for (update, _) in elements {
                        let new = nfa.add_node(DfaNode::default());
                        nfa.add_edge(&current_end, &new, NFAEdge::Symbol(*update));
                        current_end = new;
                    }
                }
                LTCTranslationElement::Loops(loops) => {
                    for ts in loops {
                        let loop_start = if relaxed {
                            // don't add a transition to the loop start, so that the loops can be
                            // taken in any order
                            current_end
                        } else {
                            // create an empty transition to the loop start, so that the loops have
                            // be taken in the order that they are in
                            // the LTC
                            let loop_start = nfa.add_node(DfaNode::default());
                            nfa.add_edge(&current_end, &loop_start, NFAEdge::Epsilon);
                            current_end = loop_start;
                            loop_start
                        };

                        for letter in ts.iter_letters().take(ts.len() - 1) {
                            let new = nfa.add_node(DfaNode::default());
                            nfa.add_edge(&current_end, &new, NFAEdge::Symbol(*letter));
                            current_end = new;
                        }

                        let last_ts_entry = ts.last().unwrap();
                        nfa.add_edge(&current_end, &loop_start, NFAEdge::Symbol(last_ts_entry.0));

                        current_end = loop_start;
                    }
                }
            }
        }

        nfa.graph[current_end].accepting = true;

        // dbg!(&nfa);

        let mut dfa = nfa.determinize();
        // dfa.add_failure_state(());
        dfa.invert_mut();

        dfa
    }

    pub fn to_ltc(&self, dimension: usize) -> LTC {
        let mut ltc = LTC::new(dimension);

        for translation in &self.elements {
            ltc.add(translation.to_ltc_element(dimension));
        }

        ltc
    }

    pub fn to_fancy_string(&self) -> String {
        self.elements
            .iter()
            .map(|x| x.to_fancy_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl<NIndex: GIndex> From<&Path<NIndex, CFGCounterUpdate>> for LTCTranslation<NIndex> {
    fn from(path: &Path<NIndex, CFGCounterUpdate>) -> Self {
        let mut stack: TransitionSequence<NIndex, CFGCounterUpdate> = TransitionSequence::new();
        // This is used to track the node where the transition sequence in the `stack`
        // started
        let mut stack_start_node: Option<NIndex> = Some(path.start().clone());
        let mut ltc_translation = vec![];

        for (update, node_index) in path.iter() {
            if let Some(last_node) = stack_start_node.take()
                && *node_index == last_node
            {
                stack.add(*update, node_index.clone());

                // We don't need to update the `stack_start_node` here, because we just did
                // a full loop
                match ltc_translation.last_mut() {
                    Some(&mut LTCTranslationElement::Loops(ref mut l)) => {
                        if l.last() != Some(&stack) {
                            l.push(stack);
                        }
                    }
                    _ => {
                        ltc_translation.push(LTCTranslationElement::Loops(vec![stack]));
                    }
                }

                stack = TransitionSequence::new();
                continue;
            }

            let existing_pos = stack.iter().position(|x| x.1 == *node_index);

            stack.add(*update, node_index.clone());

            if let Some(pos) = existing_pos {
                let transition_loop = stack.split_off(pos + 1);
                // push the remaining transitions before the loop
                if !stack.is_empty() {
                    stack_start_node = Some(stack.last().unwrap().1.clone());
                    ltc_translation.push(LTCTranslationElement::Path(stack));
                }
                if !transition_loop.is_empty() {
                    // only push the loop if the last element is not the same
                    // that just means we ran the last loop again
                    let tl_last = transition_loop.end().unwrap();
                    match ltc_translation.last_mut() {
                        Some(&mut LTCTranslationElement::Loops(ref mut l)) => {
                            if l.last() != Some(&transition_loop) {
                                stack_start_node = Some(tl_last.clone());
                                l.push(transition_loop);
                            }
                        }
                        _ => {
                            stack_start_node = Some(tl_last.clone());
                            ltc_translation
                                .push(LTCTranslationElement::Loops(vec![transition_loop]));
                        }
                    }
                }

                stack = TransitionSequence::new();
            }
        }

        if !stack.is_empty() {
            if let Some(LTCTranslationElement::Loops(l)) = ltc_translation.last_mut() {
                if l.last() != Some(&stack) {
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
}

impl<NIndex: GIndex> From<Path<NIndex, CFGCounterUpdate>> for LTCTranslation<NIndex> {
    fn from(path: Path<NIndex, CFGCounterUpdate>) -> Self {
        (&path).into()
    }
}

impl LTCTranslation<NodeIndex> {
    pub fn from_multi_graph_path(state: &ImplicitCFGProduct, path: &MultiGraphPath) -> Self {
        path.to_path_in_cfg(state.main_cfg(), state.main_cfg_index())
            .into()
    }
}

impl<NIndex: GIndex> Default for LTCTranslation<NIndex> {
    fn default() -> Self {
        Self::new()
    }
}
