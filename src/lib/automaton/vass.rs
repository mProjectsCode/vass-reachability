use hashbrown::HashMap;

use petgraph::{
    graph::{EdgeIndex, NodeIndex},
    stable_graph::StableDiGraph,
    visit::EdgeRef,
    Direction,
};

use super::{
    cfg::CFGCounterUpdate,
    dfa::{DfaNodeData, DFA},
    nfa::NFA,
    utils::VASSValuation,
    AutBuild, Automaton, AutomatonEdge, AutomatonNode,
};

pub type VassEdge<E> = (E, Box<[i32]>);

// todo epsilon transitions
#[derive(Debug, Clone)]
pub struct VASS<N: AutomatonNode, E: AutomatonEdge> {
    pub graph: StableDiGraph<N, VassEdge<E>>,
    pub alphabet: Vec<E>,
    pub dimension: usize,
}

impl<N: AutomatonNode, E: AutomatonEdge> VASS<N, E> {
    pub fn new(dimension: usize, alphabet: Vec<E>) -> Self {
        let graph = StableDiGraph::new();
        VASS {
            alphabet,
            graph,
            dimension,
        }
    }

    pub fn init(
        self,
        initial_valuation: Box<[i32]>,
        final_valuation: Box<[i32]>,
        initial_node: NodeIndex<u32>,
        final_node: NodeIndex<u32>,
    ) -> InitializedVASS<N, E> {
        assert!(
            initial_valuation.len() == self.dimension,
            "Initial valuation has to have the same length as the dimension"
        );
        assert!(
            final_valuation.len() == self.dimension,
            "Final valuation has to have the same length as the dimension"
        );

        InitializedVASS {
            vass: self,
            initial_valuation,
            final_valuation,
            initial_node,
            final_node,
        }
    }

    pub fn state_count(&self) -> usize {
        self.graph.node_count()
    }

    pub fn transition_count(&self) -> usize {
        self.graph.edge_count()
    }
}

impl<N: AutomatonNode, E: AutomatonEdge> AutBuild<NodeIndex, EdgeIndex, N, VassEdge<E>>
    for VASS<N, E>
{
    fn add_state(&mut self, data: N) -> NodeIndex<u32> {
        self.graph.add_node(data)
    }

    fn add_transition(
        &mut self,
        from: NodeIndex<u32>,
        to: NodeIndex<u32>,
        label: VassEdge<E>,
    ) -> EdgeIndex<u32> {
        assert!(
            label.1.len() == self.dimension,
            "Update has to have the same length as the dimension"
        );

        let existing_edge = self
            .graph
            .edges_directed(from, Direction::Outgoing)
            .find(|edge| *edge.weight() == label);
        if let Some(edge) = existing_edge {
            let target = edge.target();
            if target != to {
                panic!("Transition conflict, adding the new transition causes this automaton to no longer be a VASS, as VASS have to be deterministic. Existing: {:?} -{:?}-> {:?}. New: {:?} -{:?}-> {:?}", from, label, target, from, label, to);
            }
        }

        self.graph.add_edge(from, to, label)
    }
}

pub fn marking_to_vec(marking: &[i32]) -> Vec<CFGCounterUpdate> {
    let mut vec = vec![];

    for (d, m) in marking.iter().enumerate() {
        let index = (d + 1) as i32;

        let label = if *m > 0 {
            CFGCounterUpdate::new(index).unwrap()
        } else {
            CFGCounterUpdate::new(-index).unwrap()
        };

        for _ in 0..m.abs() {
            vec.push(label);
        }
    }

    vec
}

#[derive(Debug, Clone)]
pub struct InitializedVASS<N: AutomatonNode, E: AutomatonEdge> {
    pub vass: VASS<N, E>,
    pub initial_valuation: Box<[i32]>,
    pub final_valuation: Box<[i32]>,
    pub initial_node: NodeIndex<u32>,
    pub final_node: NodeIndex<u32>,
}

impl<N: AutomatonNode, E: AutomatonEdge> InitializedVASS<N, E> {
    pub fn to_cfg(&self) -> DFA<Vec<Option<N>>, CFGCounterUpdate> {
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
                let marking_vec = marking_to_vec(vass_label);

                if marking_vec.is_empty() {
                    cfg.add_transition(cfg_state, cfg_target, None);
                } else {
                    let mut cfg_source = cfg_state;

                    for label in marking_vec.iter().take(marking_vec.len() - 1) {
                        let target = cfg.add_state(DfaNodeData::new(false, None));
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

    fn state_to_cfg_state(&self, state: NodeIndex<u32>) -> DfaNodeData<Option<N>> {
        DfaNodeData::new(
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
