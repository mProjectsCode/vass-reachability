use std::{fmt::Debug, hash::Hash};

pub mod dfa;
// pub mod dyck;
pub mod ltc;
// pub mod modulo;
pub mod nfa;
pub mod path;
pub mod petri_net;
pub mod utils;
pub mod vass;

pub trait AutNode: Debug + Clone + PartialEq + Eq + Hash {}
pub trait AutEdge: Debug + Clone + PartialEq + Eq + Hash + Ord {}

impl<T> AutNode for T where T: Debug + Clone + PartialEq + Eq + Hash {}
impl<T> AutEdge for T where T: Debug + Clone + PartialEq + Eq + Hash + Ord {}

pub trait AutBuild<NIndex, N: AutNode, E: AutEdge> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E);
}

pub trait Automaton<E: AutEdge> {
    fn accepts(&self, input: &[E]) -> bool;
    fn alphabet(&self) -> &Vec<E>;
}
