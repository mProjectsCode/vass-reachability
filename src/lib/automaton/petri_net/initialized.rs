use serde::{Deserialize, Serialize};

use crate::automaton::{
    AutBuild,
    petri_net::PetriNet,
    vass::{VASS, initialized::InitializedVASS},
};

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
