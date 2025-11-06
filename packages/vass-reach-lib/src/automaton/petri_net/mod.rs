use initialized::InitializedPetriNet;
use serde::{Deserialize, Serialize};
use transition::PetriNetTransition;

use crate::automaton::vass::counter::VASSCounterValuation;

pub mod initialized;
pub mod transition;
pub mod spec;

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

    /// The first element of the tuple is the weight and the second element is
    /// the place id (starting from 1).
    pub fn add_transition(&mut self, input: Vec<(usize, PlaceId)>, output: Vec<(usize, PlaceId)>) {
        self.transitions
            .push(PetriNetTransition::new(input, output));
    }

    pub fn add_transition_struct(&mut self, transition: PetriNetTransition) {
        self.transitions.push(transition);
    }

    pub fn init(
        self,
        initial_marking: VASSCounterValuation,
        final_marking: VASSCounterValuation,
    ) -> InitializedPetriNet {
        InitializedPetriNet::new(self, initial_marking, final_marking)
    }
}