use ndarray::{Array, Dim, Dimension, IxDyn, IxDynImpl};
use petgraph::graph::NodeIndex;

use super::{
    dfa::{DfaNodeData, DFA},
    AutBuild, Automaton,
};

/// a modulo automaton tracks counters modulo some number `mu`. It accepts a run if the counters are all 0 at the end of the run.
#[derive(Debug, Clone)]
pub struct ModuloDFA {
    mu: usize,
    counter_count: usize,
    dfa: DFA<Vec<usize>, i32>,
}

fn dim_to_array(x: Dim<IxDynImpl>) -> Vec<usize> {
    let mut arr = vec![0];
    for d in x.as_array_view().iter() {
        arr.push(*d);
    }
    arr
}

impl ModuloDFA {
    pub fn new(counter_count: usize, mu: usize, invert: bool) -> Self {
        let mut alphabet = vec![];
        for i in 1..=counter_count {
            alphabet.push(i as i32);
            alphabet.push(-(i as i32));
        }

        let mut dfa = DFA::new(alphabet);

        let nodes =
            Array::<NodeIndex<u32>, IxDyn>::from_shape_fn(IxDyn(&vec![mu; counter_count]), |x| {
                let arr = dim_to_array(x);
                let is_0 = arr.iter().all(|&x| x == 0);

                let state = dfa.add_state(DfaNodeData::new(!is_0 ^ invert, arr));

                if is_0 {
                    dfa.set_start(state);
                }

                state
            });

        // dbg!(&nodes);

        for (index, node) in nodes.indexed_iter() {
            for d in 0..counter_count {
                // add the transition for adding one to counter d
                let mut new_index = index.clone();
                new_index[d] = (new_index[d] + 1) % mu;
                dfa.add_transition(*node, nodes[new_index], (d + 1) as i32);

                // add the transition for subtracting one from counter d
                let mut new_index = index.clone();
                new_index[d] = (new_index[d] + mu - 1) % mu;
                dfa.add_transition(*node, nodes[new_index], -(d as i32 + 1));
            }
        }

        dfa.override_complete();

        Self {
            mu,
            dfa,
            counter_count,
        }
    }

    pub fn mu(&self) -> usize {
        self.mu
    }

    pub fn dfa(&self) -> &DFA<Vec<usize>, i32> {
        &self.dfa
    }

    pub fn counter_count(&self) -> usize {
        self.counter_count
    }
}

impl Automaton<i32> for ModuloDFA {
    fn accepts(&self, input: &[i32]) -> bool {
        self.dfa.accepts(input)
    }

    fn alphabet(&self) -> &Vec<i32> {
        self.dfa.alphabet()
    }
}
