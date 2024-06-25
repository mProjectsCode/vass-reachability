use ndarray::{Array, Dim, Dimension, IntoDimension, IxDyn, IxDynImpl, ShapeBuilder};
use petgraph::{graph::NodeIndex, visit::NodeRef};

use super::{
    dfa::{DfaNodeData, DFA},
    AutBuild, Automaton,
};

/// a modulo automaton tracks counters modulo some number `mu`. It accepts a run if the counters are all 0 at the end of the run.
#[derive(Debug, Clone)]
pub struct ModuloDFA<const D: usize> {
    mu: usize,
    dfa: DFA<[usize; D], i32>,
}

fn dim_to_array<const D: usize>(x: Dim<IxDynImpl>) -> [usize; D] {
    assert_eq!(x.ndim(), D, "dimension mismatch");

    let mut arr = [0; D];
    for (i, &d) in x.as_array_view().iter().enumerate() {
        arr[i] = d;
    }
    arr
}

impl<const D: usize> ModuloDFA<D> {
    pub fn new(mu: usize) -> Self {
        let mut alphabet = vec![];
        for i in 1..=D {
            alphabet.push(i as i32);
            alphabet.push(-(i as i32));
        }

        let mut dfa = DFA::new(alphabet);

        // let mut nodes = Array::<u32, IxDyn>::zeros(IxDyn(&[mu; D]));
        let nodes = Array::<NodeIndex<u32>, IxDyn>::from_shape_fn(IxDyn(&[mu; D]), |x| {
            let arr = dim_to_array::<D>(x);
            let is_0 = arr.iter().all(|&x| x == 0);

            let state = dfa.add_state(DfaNodeData::new(!is_0, arr));

            if is_0 {
                dfa.set_start(state);
            }

            state
        });

        dbg!(&nodes);

        for (index, node) in nodes.indexed_iter() {
            for d in 0..D {
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

        Self { mu, dfa }
    }
}

impl<const D: usize> Automaton<i32> for ModuloDFA<D> {
    fn accepts(&self, input: &[i32]) -> bool {
        self.dfa.accepts(input)
    }
}
