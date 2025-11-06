use serde::{Deserialize, Serialize};

use crate::automaton::{petri_net::PlaceId, vass::counter::VASSCounterUpdate};

/// Petri net transition. The first element of the tuple is the weight and the
/// second element is the place id (starting from 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetriNetTransition {
    input: Vec<(usize, PlaceId)>,
    output: Vec<(usize, PlaceId)>,
}

impl PetriNetTransition {
    pub fn new(input: Vec<(usize, PlaceId)>, output: Vec<(usize, PlaceId)>) -> Self {
        Self { input, output }
    }

    /// Converts from a Subtract and Add representation to a PetriNetTransition.
    /// Note that the input update must all be negative or zero, and the output
    /// update must all be positive or zero.
    pub fn from_vass_updates(
        input: VASSCounterUpdate,
        output: VASSCounterUpdate,
    ) -> Self {
        let mut input_vec = vec![];
        let mut output_vec = vec![];

        for (i, &val) in input.iter().enumerate() {
            if val < 0 {
                input_vec.push(((-val) as usize, i + 1));
            }
        }

        for (i, &val) in output.iter().enumerate() {
            if val > 0 {
                output_vec.push(((val) as usize, i + 1));
            }
        }

        Self {
            input: input_vec,
            output: output_vec,
        }
    }

    pub fn input_to_vass_update(&self, place_count: usize) -> VASSCounterUpdate {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.input {
            vec[*place - 1] = -(*weight as i32);
        }

        vec.into()
    }

    pub fn output_to_vass_update(&self, place_count: usize) -> VASSCounterUpdate {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.output {
            vec[*place - 1] = *weight as i32;
        }

        vec.into()
    }
}

