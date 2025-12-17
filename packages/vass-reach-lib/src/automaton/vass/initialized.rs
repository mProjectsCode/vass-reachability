use std::iter::repeat;

use itertools::Itertools;
use petgraph::{Direction, graph::NodeIndex, prelude::EdgeRef};

use crate::automaton::{
    Automaton, AutomatonEdge, AutomatonNode, FromLetter, Frozen, InitializedAutomaton, Language,
    SingleFinalStateAutomaton,
    cfg::{update::CFGCounterUpdate, vasscfg::VASSCFG},
    dfa::node::DfaNode,
    index_map::IndexMap,
    nfa::{NFA, NFAEdge},
    petri_net::{PetriNet, initialized::InitializedPetriNet, transition::PetriNetTransition},
    utils::{self},
    vass::{
        VASS, VASSEdge,
        counter::{VASSCounterUpdate, VASSCounterValuation},
    },
};

#[derive(Debug, Clone)]
pub struct InitializedVASS<N: AutomatonNode, E: AutomatonEdge + FromLetter> {
    pub vass: VASS<N, E>,
    pub initial_valuation: VASSCounterValuation,
    pub final_valuation: VASSCounterValuation,
    pub initial_node: NodeIndex<u32>,
    pub final_node: NodeIndex<u32>,
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> InitializedVASS<N, E> {
    pub fn to_cfg(&self) -> VASSCFG<()> {
        let mut cfg = NFA::new(CFGCounterUpdate::alphabet(self.vass.dimension));

        let cfg_start = cfg.add_node(self.state_to_cfg_state(self.initial_node));
        cfg.set_start(cfg_start);

        let mut visited = IndexMap::new(self.vass.state_count());
        let mut stack = vec![(self.initial_node, cfg_start)];

        while let Some((vass_state, cfg_state)) = stack.pop() {
            visited.insert(vass_state, cfg_state);

            for vass_edge in self.outgoing_edge_indices(vass_state) {
                let vass_target = self.edge_target_unchecked(vass_edge);
                let cfg_target = if let Some(target) = visited.get_option(vass_target) {
                    *target
                } else {
                    let target = cfg.add_node(self.state_to_cfg_state(vass_target));
                    stack.push((vass_target, target));
                    target
                };

                let vass_label = &self.get_edge_unchecked(vass_edge).update;
                let marking_vec = utils::vass_update_to_cfg_updates(vass_label);

                if marking_vec.is_empty() {
                    cfg.add_edge(cfg_state, cfg_target, NFAEdge::Epsilon);
                } else {
                    let mut cfg_source = cfg_state;

                    for label in marking_vec.iter().take(marking_vec.len() - 1) {
                        let target = cfg.add_node(DfaNode::default());
                        cfg.add_edge(cfg_source, target, NFAEdge::Symbol(*label));
                        cfg_source = target;
                    }

                    let label = marking_vec[marking_vec.len() - 1];
                    cfg.add_edge(cfg_source, cfg_target, NFAEdge::Symbol(label));
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

        let node = vas.add_node(());

        for i in 0..self.state_count() {
            let (a, b) = self.vas_translation_ab(i as i32);

            let (other_a, other_b) = self.vas_translation_ab((self.state_count() - i - 1) as i32);

            vas.add_edge(
                node,
                node,
                VASSEdge::new(
                    self.vass.alphabet.len() + i * 2,
                    VASSCounterUpdate::new(
                        repeat(0)
                            .take(self.dimension())
                            .chain([-a, other_a - b, other_b])
                            .collect(),
                    ),
                ),
            );

            vas.add_edge(
                node,
                node,
                VASSEdge::new(
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

            let edge = self
                .vass
                .graph
                .edge_weight(e)
                .expect("edge index to be present");

            vas.add_edge(
                node,
                node,
                VASSEdge::new(
                    e.index(),
                    edge.update.extend([to_a - from_b, to_b, -from_a]),
                ),
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
            let edge = self
                .vass
                .graph
                .edge_weight(e)
                .expect("edge index to be present");

            net.add_transition_struct(PetriNetTransition::from_vass_update(&edge.update));
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

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Automaton for InitializedVASS<N, E> {
    type NIndex = <VASS<N, E> as Automaton>::NIndex;
    type EIndex = <VASS<N, E> as Automaton>::EIndex;
    type N = N;
    type E = VASSEdge<E>;

    fn node_count(&self) -> usize {
        self.vass.node_count()
    }

    fn edge_count(&self) -> usize {
        self.vass.edge_count()
    }

    fn get_node(&self, index: Self::NIndex) -> Option<&N> {
        self.vass.get_node(index)
    }

    fn get_edge(&self, index: Self::EIndex) -> Option<&VASSEdge<E>> {
        self.vass.get_edge(index)
    }

    fn get_node_unchecked(&self, index: Self::NIndex) -> &N {
        self.vass.get_node_unchecked(index)
    }

    fn get_edge_unchecked(&self, index: Self::EIndex) -> &VASSEdge<E> {
        self.vass.get_edge_unchecked(index)
    }

    fn edge_endpoints(&self, edge: Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)> {
        self.vass.edge_endpoints(edge)
    }

    fn edge_endpoints_unchecked(&self, edge: Self::EIndex) -> (Self::NIndex, Self::NIndex) {
        self.vass.edge_endpoints_unchecked(edge)
    }

    fn outgoing_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.vass.outgoing_edge_indices(node)
    }

    fn incoming_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.vass.incoming_edge_indices(node)
    }

    fn connecting_edge_indices(
        &self,
        from: Self::NIndex,
        to: Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex> {
        self.vass.connecting_edge_indices(from, to)
    }

    fn add_node(&mut self, data: N) -> Self::NIndex {
        self.vass.add_node(data)
    }

    fn add_edge(
        &mut self,
        from: Self::NIndex,
        to: Self::NIndex,
        label: VASSEdge<E>,
    ) -> Self::EIndex {
        self.vass.add_edge(from, to, label)
    }

    fn remove_node(&mut self, node: Self::NIndex) {
        self.vass.remove_node(node);
    }

    fn remove_edge(&mut self, edge: Self::EIndex) {
        self.vass.remove_edge(edge);
    }

    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool,
    {
        for index in self.iter_node_indices().rev() {
            if !f(Frozen::from(&mut *self), index) {
                self.remove_node(index);
            }
        }
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> InitializedAutomaton
    for InitializedVASS<N, E>
{
    fn get_initial(&self) -> Self::NIndex {
        self.initial_node
    }

    fn set_initial(&mut self, node: Self::NIndex) {
        self.initial_node = node;
    }

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        node == self.final_node
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> SingleFinalStateAutomaton
    for InitializedVASS<N, E>
{
    fn get_final(&self) -> Self::NIndex {
        self.final_node
    }

    fn set_final(&mut self, node: Self::NIndex) {
        self.final_node = node;
    }
}

impl<N: AutomatonNode, E: AutomatonEdge + FromLetter> Language for InitializedVASS<N, E> {
    type Letter = <VASSEdge<E> as AutomatonEdge>::Letter;

    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a E::Letter>) -> bool
    where
        E::Letter: 'a,
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
                        edge.matches(symbol) && current_valuation.can_apply_update(&edge.update)
                    })
                    .map(|edge| {
                        // subtract the valuation of the edge from the current valuation
                        current_valuation.apply_update(&edge.weight().update);
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

    fn alphabet(&self) -> &[E::Letter] {
        &self.vass.alphabet
    }
}
