use serde::{Deserialize, Serialize};

use crate::automaton::{petri_net::PlaceId, vass::counter::VASSCounterUpdate};

/// Petri net transition. The first element of the tuple is the weight and the
/// second element is the place id (starting from 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetriNetTransition {
    pub input: Vec<(usize, PlaceId)>,
    pub output: Vec<(usize, PlaceId)>,
}

impl PetriNetTransition {
    pub fn new(input: Vec<(usize, PlaceId)>, output: Vec<(usize, PlaceId)>) -> Self {
        Self { input, output }
    }

    /// Converts from a Subtract and Add representation to a PetriNetTransition.
    /// Note that the input update must all be negative or zero, and the output
    /// update must all be positive or zero.
    pub fn from_vass_updates<'a>(input: impl IntoIterator<Item = &'a i32>, output: impl IntoIterator<Item = &'a i32>) -> Self {
        let mut input_vec = vec![];
        let mut output_vec = vec![];

        for (i, &val) in input.into_iter().enumerate() {
            if val < 0 {
                input_vec.push(((-val) as usize, i + 1));
            } else if val > 0 {
                panic!("input update had a positive component");
            }
        }

        for (i, &val) in output.into_iter().enumerate() {
            if val > 0 {
                output_vec.push(((val) as usize, i + 1));
            } else if val < 0 {
                panic!("input update had a negative component");
            }
        }

        Self {
            input: input_vec,
            output: output_vec,
        }
    }

    pub fn from_vass_update<'a>(update: impl IntoIterator<Item = &'a i32>) -> Self {
        let mut input_vec = vec![];
        let mut output_vec = vec![];

        for (i, &val) in update.into_iter().enumerate() {
            if val < 0 {
                input_vec.push(((-val) as usize, i + 1));
            } else if val > 0 {
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

    pub fn get_update_for_place(&self, place: PlaceId) -> (usize, usize) {
        let input = self
            .input
            .iter()
            .find(|(_, p)| *p == place)
            .map(|(w, _)| *w)
            .unwrap_or(0);

        let output = self
            .output
            .iter()
            .find(|(_, p)| *p == place)
            .map(|(w, _)| *w)
            .unwrap_or(0);

        (input, output)
    }
}
