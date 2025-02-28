use hashbrown::HashMap;
use petgraph::{Direction, graph::NodeIndex, prelude::EdgeRef};

use crate::automaton::{
    AutBuild, Automaton, AutomatonEdge, AutomatonNode,
    dfa::{
        cfg::{CFGCounterUpdate, VASSCFG},
        node::DfaNode,
    },
    nfa::NFA,
    utils::{self, VASSValuation},
    vass::VASS,
};

#[derive(Debug, Clone)]
pub struct InitializedVASS<N: AutomatonNode, E: AutomatonEdge> {
    pub vass: VASS<N, E>,
    pub initial_valuation: Box<[i32]>,
    pub final_valuation: Box<[i32]>,
    pub initial_node: NodeIndex<u32>,
    pub final_node: NodeIndex<u32>,
}

impl<N: AutomatonNode, E: AutomatonEdge> InitializedVASS<N, E> {
    pub fn to_cfg(&self) -> VASSCFG<()> {
        let mut cfg = NFA::new(CFGCounterUpdate::alphabet(self.vass.dimension));

        let cfg_start = cfg.add_state(self.state_to_cfg_state(self.initial_node));
        cfg.set_start(cfg_start);

        let mut visited = HashMap::<NodeIndex, NodeIndex>::new();
        let mut stack = vec![(self.initial_node, cfg_start)];

        while let Some((vass_state, cfg_state)) = stack.pop() {
            visited.insert(vass_state, cfg_state);

            for vass_edge in self
                .vass
                .graph
                .edges_directed(vass_state, Direction::Outgoing)
            {
                let cfg_target = if let Some(&target) = visited.get(&vass_edge.target()) {
                    target
                } else {
                    let target = cfg.add_state(self.state_to_cfg_state(vass_edge.target()));
                    stack.push((vass_edge.target(), target));
                    target
                };

                let vass_label = &vass_edge.weight().1;
                let marking_vec = utils::vass_update_to_cfg_updates(vass_label);

                if marking_vec.is_empty() {
                    cfg.add_transition(cfg_state, cfg_target, None);
                } else {
                    let mut cfg_source = cfg_state;

                    for label in marking_vec.iter().take(marking_vec.len() - 1) {
                        let target = cfg.add_state(DfaNode::new(false, None));
                        cfg.add_transition(cfg_source, target, Some(*label));
                        cfg_source = target;
                    }

                    let label = marking_vec[marking_vec.len() - 1];
                    cfg.add_transition(cfg_source, cfg_target, Some(label));
                }
            }
        }

        cfg.determinize()
    }

    fn state_to_cfg_state(&self, state: NodeIndex<u32>) -> DfaNode<Option<N>> {
        DfaNode::new(
            state == self.final_node,
            Some(self.vass.graph[state].clone()),
        )
    }

    pub fn state_count(&self) -> usize {
        self.vass.state_count()
    }

    pub fn transition_count(&self) -> usize {
        self.vass.transition_count()
    }

    pub fn dimension(&self) -> usize {
        self.vass.dimension
    }
}

impl<N: AutomatonNode, E: AutomatonEdge> Automaton<E> for InitializedVASS<N, E> {
    fn accepts(&self, input: &[E]) -> bool {
        let mut current_state = Some(self.initial_node);
        let mut current_valuation = self.initial_valuation.clone();

        for symbol in input {
            if let Some(state) = current_state {
                let next_state = self
                    .vass
                    .graph
                    .edges_directed(state, Direction::Outgoing)
                    .find(|neighbor| {
                        let edge = neighbor.weight();
                        // check that we can take the edge
                        edge.0 == *symbol && current_valuation >= edge.1.neg()
                    })
                    .map(|edge| {
                        // subtract the valuation of the edge from the current valuation
                        current_valuation.add_mut(&edge.weight().1);
                        edge.target()
                    });
                current_state = next_state;
            } else {
                return false;
            }
        }

        match current_state {
            Some(state) => state == self.final_node && current_valuation == self.final_valuation,
            None => false,
        }
    }

    fn alphabet(&self) -> &Vec<E> {
        &self.vass.alphabet
    }
}
