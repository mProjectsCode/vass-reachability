use serde::{Deserialize, Serialize};

use crate::automaton::{
    ModifiableAutomaton,
    petri_net::{
        PetriNet,
        spec::{PetriNetSpec, ToSpecFormat},
    },
    vass::{VASS, VASSEdge, counter::VASSCounterValuation, initialized::InitializedVASS},
};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitializedPetriNet {
    pub net: PetriNet,
    pub initial_marking: VASSCounterValuation,
    pub final_marking: VASSCounterValuation,
}

impl InitializedPetriNet {
    pub fn new(
        net: PetriNet,
        initial_marking: VASSCounterValuation,
        final_marking: VASSCounterValuation,
    ) -> Self {
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
        let center_state = vass.add_node(0);

        for (i, transition) in self.net.transitions.iter().enumerate() {
            let state = vass.add_node(i + 1);
            let input_vec = transition.input_to_vass_update(self.net.place_count);
            let output_vec = transition.output_to_vass_update(self.net.place_count);

            vass.add_edge(center_state, state, VASSEdge::new(i, input_vec));
            vass.add_edge(state, center_state, VASSEdge::new(i, output_vec));
        }

        vass.init(
            self.initial_marking.clone(),
            self.final_marking.clone(),
            center_state,
            center_state,
        )
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    pub fn from_json(json: &str) -> anyhow::Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json_file(&self, path: &str) -> anyhow::Result<()> {
        Ok(std::fs::write(path, self.to_json()?)?)
    }

    pub fn to_spec_file(&self, path: &str) -> anyhow::Result<()> {
        Ok(std::fs::write(path, self.to_spec_format())?)
    }

    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        let path = std::path::Path::new(path);
        match path.extension() {
            Some(ext) if ext == "json" => {
                let json_str = std::fs::read_to_string(path)?;
                Ok(Self::from_json(&json_str)?)
            }
            Some(ext) if ext == "spec" => {
                let spec_str = std::fs::read_to_string(path)?;
                let spec = PetriNetSpec::parse(&spec_str)?;
                Ok(InitializedPetriNet::try_from(spec)?)
            }
            _ => Err(anyhow::anyhow!(
                "Unsupported file extension: {:?}",
                path.extension()
            )),
        }
    }

    pub fn parse_from_spec(spec_str: &str) -> anyhow::Result<Self> {
        let spec = PetriNetSpec::parse(spec_str)?;
        InitializedPetriNet::try_from(spec)
    }
}

impl TryFrom<PetriNetSpec<'_>> for InitializedPetriNet {
    type Error = anyhow::Error;

    fn try_from(spec: PetriNetSpec) -> Result<Self, Self::Error> {
        let mut net = PetriNet::new(spec.variables.len());
        for rule in spec.rules {
            net.add_transition_struct(rule.to_transition(&spec.variables)?);
        }

        Ok(InitializedPetriNet::new(
            net,
            spec.initial.to_counter_valuation(&spec.variables)?,
            spec.target.to_counter_valuation(&spec.variables)?,
        ))
    }
}
