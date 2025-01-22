use serde::{Deserialize, Serialize};

use super::{
    vass::{InitializedVASS, VASS},
    AutBuild,
};

type PlaceId = usize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetriNet {
    place_count: usize,
    transitions: Vec<PetriNetTransition>,
}

impl PetriNet {
    pub fn new(place_count: usize) -> Self {
        Self {
            place_count,
            transitions: vec![],
        }
    }

    /// The first element of the tuple is the weight and the second element is the place id (starting from 1).
    pub fn add_transition(&mut self, input: Vec<(usize, PlaceId)>, output: Vec<(usize, PlaceId)>) {
        self.transitions
            .push(PetriNetTransition::new(input, output));
    }

    pub fn init(
        self,
        initial_marking: Box<[usize]>,
        final_marking: Box<[usize]>,
    ) -> InitializedPetriNet {
        InitializedPetriNet::new(self, initial_marking, final_marking)
    }
}

/// Petri net transition. The first element of the tuple is the weight and the second element is the place id (starting from 1).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PetriNetTransition {
    input: Vec<(usize, PlaceId)>,
    output: Vec<(usize, PlaceId)>,
}

impl PetriNetTransition {
    fn new(input: Vec<(usize, PlaceId)>, output: Vec<(usize, PlaceId)>) -> Self {
        Self { input, output }
    }

    fn input_to_slice(&self, place_count: usize) -> Box<[i32]> {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.input {
            vec[*place - 1] = -(*weight as i32);
        }

        vec
    }

    fn output_to_slice(&self, place_count: usize) -> Box<[i32]> {
        let mut vec = vec![0; place_count].into_boxed_slice();

        for (weight, place) in &self.output {
            vec[*place - 1] = *weight as i32;
        }

        vec
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializedPetriNet {
    net: PetriNet,
    initial_marking: Box<[usize]>,
    final_marking: Box<[usize]>,
}

impl InitializedPetriNet {
    pub fn new(net: PetriNet, initial_marking: Box<[usize]>, final_marking: Box<[usize]>) -> Self {
        Self {
            net,
            initial_marking,
            final_marking,
        }
    }

    pub fn to_vass(&self) -> InitializedVASS<usize, usize> {
        let mut vass = VASS::new(
            self.net.place_count,
            (0..self.net.transitions.len()).collect(),
        );
        let center_state = vass.add_state(0);

        for (i, transition) in self.net.transitions.iter().enumerate() {
            let state = vass.add_state(i + 1);
            let input_vec = transition.input_to_slice(self.net.place_count);
            let output_vec = transition.output_to_slice(self.net.place_count);

            vass.add_transition(center_state, state, (i, input_vec));
            vass.add_transition(state, center_state, (i, output_vec));
        }

        vass.init(
            self.initial_marking.iter().map(|x| *x as i32).collect(),
            self.final_marking.iter().map(|x| *x as i32).collect(),
            center_state,
            center_state,
        )
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn from_json(json: &str) -> Self {
        serde_json::from_str(json).unwrap()
    }

    pub fn to_file(&self, path: &str) {
        std::fs::write(path, self.to_json()).unwrap();
    }

    pub fn from_file(path: &str) -> Self {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }
}
