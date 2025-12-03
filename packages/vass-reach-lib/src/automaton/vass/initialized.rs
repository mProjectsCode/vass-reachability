use std::iter::repeat;

use itertools::Itertools;
use petgraph::{Direction, graph::NodeIndex, prelude::EdgeRef};

use crate::automaton::{
    AutBuild, Automaton, AutomatonEdge, AutomatonNode,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    index_map::IndexMap,
    nfa::NFA,
    petri_net::{PetriNet, initialized::InitializedPetriNet, transition::PetriNetTransition},
    utils::{self},
    vass::{
        VASS,
        counter::{VASSCounterUpdate, VASSCounterValuation},
    },
};

#[derive(Debug, Clone)]
pub struct InitializedVASS<N: AutomatonNode, E: AutomatonEdge> {
    pub vass: VASS<N, E>,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    pub initial_node: NodeIndex<u32>,
    pub final_node: NodeIndex<u32>,
}

impl<N: AutomatonNode, E: AutomatonEdge> InitializedVASS<N, E> {
    pub fn to_cfg(&self) -> VASSCFG<()> {
        let mut cfg = NFA::new(CFGCounterUpdate::alphabet(self.vass.dimension));

        let cfg_start = cfg.add_state(self.state_to_cfg_state(self.initial_node));
        cfg.set_start(cfg_start);

        let mut visited = IndexMap::new(self.vass.state_count());
        let mut stack = vec![(self.initial_node, cfg_start)];

        while let Some((vass_state, cfg_state)) = stack.pop() {
            visited.insert(vass_state, cfg_state);

            for vass_edge in self
                .vass
                .graph
                .edges_directed(vass_state, Direction::Outgoing)
            {
                let cfg_target = if let Some(target) = visited.get_option(vass_edge.target()) {
                    *target
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
                        let target = cfg.add_state(DfaNode::default());
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
            false,
            Some(self.vass.graph[state].clone()),
        )
    }

    /// Converts the VASS into a VAS, a Vector Addition System (without states),
    /// using Hopcroft's and Pansiot's construction from 1978.
    pub fn to_vas(&self) -> InitializedVASS<(), usize> {
        let new_alphabet = (0..(self.transition_count() + self.dimension() * 2)).collect_vec();
        let mut vas = VASS::new(self.dimension() + 3, new_alphabet);

        let node = vas.add_state(());

        for i in 0..self.state_count() {
            let (a, b) = self.vas_translation_ab(i as i32);

            let (other_a, other_b) = self.vas_translation_ab((self.state_count() - i - 1) as i32);

            vas.add_transition(
                node,
                node,
                (
                    self.vass.alphabet.len() + i * 2,
                    VASSCounterUpdate::new(
                        repeat(0)
                            .take(self.dimension())
                            .chain([-a, other_a - b, other_b])
                            .collect(),
                    ),
                ),
            );

            vas.add_transition(
                node,
                node,
                (
                    self.vass.alphabet.len() + i * 2,
                    VASSCounterUpdate::new(
                        repeat(0)
                            .take(self.dimension())
                            .chain([b, -other_a, a - other_b])
                            .collect(),
                    ),
                ),
            );
        }

        for e in self.vass.graph.edge_indices() {
            let (from, to) = self
                .vass
                .graph
                .edge_endpoints(e)
                .expect("edge index to be present");

            let (from_a, from_b) = self.vas_translation_ab(from.index() as i32);
            let (to_a, to_b) = self.vas_translation_ab(to.index() as i32);

            let (_, update) = self
                .vass
                .graph
                .edge_weight(e)
                .expect("edge index to be present");

            vas.add_transition(
                node,
                node,
                (e.index(), update.extend([to_a - from_b, to_b, -from_a])),
            );
        }

        let (initial_a, initial_b) = self.vas_translation_ab(self.initial_node.index() as i32);
        let (final_a, final_b) = self.vas_translation_ab(self.final_node.index() as i32);

        vas.init(
            self.initial_valuation.extend([initial_a, initial_b, 0]),
            self.final_valuation.extend([final_a, final_b, 0]),
            node,
            node,
        )
    }

    fn vas_translation_ab(&self, node: i32) -> (i32, i32) {
        let a = node + 1;
        let b = (self.state_count() as i32 + 1) * (self.state_count() as i32 - node);
        (a, b)
    }

    /// Converts a VAS into a PetriNet. Panics if this is not a VAS, so has more
    /// than one state.
    pub fn to_petri_net(&self) -> InitializedPetriNet {
        assert_eq!(self.state_count(), 1);

        let mut net = PetriNet::new(self.dimension());

        for e in self.vass.graph.edge_indices() {
            let (_, update) = self
                .vass
                .graph
                .edge_weight(e)
                .expect("edge index to be present");

            net.add_transition_struct(PetriNetTransition::from_vass_update(
                update
            ));
        }

        net.init(self.initial_valuation.clone(), self.final_valuation.clone())
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
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a E>) -> bool
    where
        E: 'a,
    {
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
                        edge.0 == *symbol && current_valuation.can_apply_update(&edge.1)
                    })
                    .map(|edge| {
                        // subtract the valuation of the edge from the current valuation
                        current_valuation.apply_update(&edge.weight().1);
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
