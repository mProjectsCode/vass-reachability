use std::fmt::Debug;

pub mod dfa;
pub mod vass;
pub mod modulo;

pub trait AutNode : Debug + Clone + PartialEq {}
pub trait AutEdge : Debug + Clone + PartialEq {}

impl<T> AutNode for T where T:Debug + Clone + PartialEq {}
impl<T> AutEdge for T where T: Debug + Clone + PartialEq {}

pub trait AutBuild<NIndex, N: AutNode, E: AutEdge> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E);   
}

pub trait Automaton<NIndex, E: AutEdge> {
    fn accepts(&self, input: &[E]) -> bool;
    fn start(&self) -> NIndex;
}