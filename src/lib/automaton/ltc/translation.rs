use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::automaton::{
    AutBuild, AutomatonNode,
    dfa::{
        cfg::{CFGCounterUpdate, VASSCFG},
        node::DfaNode,
    },
    ltc::{LTC, LTCElement},
    nfa::NFA,
    path::{Path, transition_sequence::TransitionSequence},
    utils::cfg_updates_to_ltc_transition,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LTCTranslationElement {
    Path(TransitionSequence),
    Loop(TransitionSequence),
}

impl LTCTranslationElement {
    pub fn to_ltc_element(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> LTCElement {
        match self {
            LTCTranslationElement::Path(edges) => {
                let edge_weights = edges.iter().map(|&edge| get_edge_weight(edge.0));
                let (min_counters, counters) =
                    cfg_updates_to_ltc_transition(edge_weights, dimension);
                LTCElement::Transition((min_counters, counters))
            }
            LTCTranslationElement::Loop(edges) => {
                let edge_weights = edges.iter().map(|&edge| get_edge_weight(edge.0));
                let (min_counters, counters) =
                    cfg_updates_to_ltc_transition(edge_weights, dimension);
                LTCElement::Loop((min_counters, counters))
            }
        }
    }

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EdgeIndex) -> String) -> String {
        match self {
            LTCTranslationElement::Path(edges) => {
                format!("Path: {}", edges.to_fancy_string(get_edge_string))
            }
            LTCTranslationElement::Loop(edges) => {
                format!("Loop: {}", edges.to_fancy_string(get_edge_string))
            }
        }
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

    pub fn expand<N: AutomatonNode>(self, cfg: &VASSCFG<N>) -> Self {
        let mut new_elements = vec![];

        for translation in self.elements.into_iter() {
            let LTCTranslationElement::Path(transitions) = translation else {
                new_elements.push(translation);
                continue;
            };

            let mut stack = TransitionSequence::new();
            // let last = transitions.pop().expect("Path should not be empty");

            for (edge, node) in transitions {
                stack.add(edge, node);

                let loop_in_node = cfg.find_loop_rooted_in_node(node);

                if let Some(l) = loop_in_node {
                    new_elements.push(LTCTranslationElement::Path(stack));
                    stack = TransitionSequence::new();

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

        let start = nfa.add_state(DfaNode::new(false, ()));
        nfa.set_start(start);
        let mut current_end = start;

        for translation in &self.elements {
            match translation {
                LTCTranslationElement::Path(edges) => {
                    for edge in edges {
                        let new = nfa.add_state(DfaNode::new(false, ()));
                        nfa.add_transition(current_end, new, Some(get_edge_weight(edge.0)));
                        current_end = new;
                    }
                }
                LTCTranslationElement::Loop(edges) => {
                    let loop_start = if relaxed {
                        // don't add a transition to the loop start, so that the loops can be taken
                        // in any order
                        current_end
                    } else {
                        // create an empty transition to the loop start, so that the loops have be
                        // taken in the order that they are in the LTC
                        let loop_start = nfa.add_state(DfaNode::new(false, ()));
                        nfa.add_transition(current_end, loop_start, None);
                        current_end = loop_start;
                        loop_start
                    };

                    for edge in edges.iter().take(edges.len() - 1) {
                        let new = nfa.add_state(DfaNode::new(false, ()));
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

        let mut dfa = nfa.determinize();
        dfa.add_failure_state(());
        dfa.invert_mut();

        dfa
    }

    pub fn to_ltc(
        &self,
        dimension: usize,
        get_edge_weight: impl Fn(EdgeIndex<u32>) -> CFGCounterUpdate,
    ) -> LTC {
        let mut ltc = LTC::new(dimension);

        for translation in &self.elements {
            ltc.add(translation.to_ltc_element(dimension, &get_edge_weight));
        }

        ltc
    }

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EdgeIndex) -> String) -> String {
        self.elements
            .iter()
            .map(|x| x.to_fancy_string(&get_edge_string))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

impl From<&Path> for LTCTranslation {
    fn from(path: &Path) -> Self {
        let mut stack = TransitionSequence::new();
        // This is used to track the node where the transition sequence in the `stack`
        // started
        let mut stack_start_node: Option<NodeIndex> = None;
        let mut ltc_translation = vec![];

        for (edge_index, node_index) in &path.transitions {
            if let Some(last_node) = stack_start_node {
                if *node_index == last_node {
                    stack.add(*edge_index, *node_index);

                    // only push the loop if the last element is not the same
                    // that just means we ran the last loop again
                    let _loop = LTCTranslationElement::Loop(stack);
                    if ltc_translation.last() != Some(&_loop) {
                        // We don't need to update the `stack_start_node` here, because we just did
                        // a full loop
                        ltc_translation.push(_loop);
                    }
                    stack = TransitionSequence::new();
                    continue;
                }
            }

            let existing_pos = stack.iter().position(|x| x.1 == *node_index);

            stack.add(*edge_index, *node_index);

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
                    let last = transition_loop.end().unwrap();
                    let _loop = LTCTranslationElement::Loop(transition_loop);
                    if ltc_translation.last() != Some(&_loop) {
                        stack_start_node = Some(last);
                        ltc_translation.push(_loop);
                    }
                }

                stack = TransitionSequence::new();
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
}

impl Default for LTCTranslation {
    fn default() -> Self {
        Self::new()
    }
}
