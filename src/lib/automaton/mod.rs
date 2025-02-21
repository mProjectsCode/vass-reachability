use std::{fmt::Debug, hash::Hash};

pub mod dfa;
// pub mod dyck;
pub mod ltc;
// pub mod modulo;
pub mod cfg;
pub mod nfa;
pub mod parikh_image;
pub mod path;
pub mod petri_net;
pub mod utils;
pub mod vass;

pub trait AutomatonNode: Debug + Clone + PartialEq + Eq + Hash {}
pub trait AutomatonEdge: Debug + Clone + PartialEq + Eq + Hash + Ord {}

impl<T> AutomatonNode for T where T: Debug + Clone + PartialEq + Eq + Hash {}
impl<T> AutomatonEdge for T where T: Debug + Clone + PartialEq + Eq + Hash + Ord {}

pub trait AutBuild<NIndex, EIndex, N: AutomatonNode, E: AutomatonEdge> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E) -> EIndex;
}

pub trait Automaton<E: AutomatonEdge> {
    fn accepts(&self, input: &[E]) -> bool;
    fn alphabet(&self) -> &Vec<E>;
}
