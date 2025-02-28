use serde::{Deserialize, Serialize};

use crate::automaton::petri_net::PlaceId;

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

    pub fn input_to_slice(&self, place_count: usize) -> Box<[i32]> {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.input {
            vec[*place - 1] = -(*weight as i32);
        }

        vec
    }

    pub fn output_to_slice(&self, place_count: usize) -> Box<[i32]> {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.output {
            vec[*place - 1] = *weight as i32;
        }

        vec
    }
}
