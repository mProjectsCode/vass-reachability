use std::fmt::Debug;

pub mod dfa;
pub mod dyck;
pub mod ltc;
pub mod modulo;
pub mod nfa;
pub mod path;
pub mod vass;

pub trait AutNode: Debug + Clone + PartialEq {}
pub trait AutEdge: Debug + Clone + PartialEq + Ord {}

impl<T> AutNode for T where T: Debug + Clone + PartialEq {}
impl<T> AutEdge for T where T: Debug + Clone + PartialEq + Ord {}

pub trait AutBuild<NIndex, N: AutNode, E: AutEdge> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E);
}

pub trait Automaton<E: AutEdge> {
    fn accepts(&self, input: &[E]) -> bool;
    fn alphabet(&self) -> &Vec<E>;
}
