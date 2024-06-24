use std::fmt::Debug;

pub mod dfa;
pub mod dyck;
pub mod modulo;
pub mod vass;

pub trait AutNode: Debug + Clone + PartialEq {}
pub trait AutEdge: Debug + Clone + PartialEq {}

impl<T> AutNode for T where T: Debug + Clone + PartialEq {}
impl<T> AutEdge for T where T: Debug + Clone + PartialEq {}

pub trait AutBuild<NIndex, N: AutNode, E: AutEdge> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E);
}

pub trait Automaton<E: AutEdge> {
    fn accepts(&self, input: &[E]) -> bool;
}
