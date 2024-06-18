use std::fmt::Debug;

pub mod dfa;

pub trait Automaton<NIndex, N: Debug + Clone + PartialEq, E: Debug + Clone + PartialEq> {
    fn add_state(&mut self, data: N) -> NIndex;
    fn add_transition(&mut self, from: NIndex, to: NIndex, label: E);
    fn accepts(&self, input: &[E]) -> bool;
}
