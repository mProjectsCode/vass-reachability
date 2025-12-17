use crate::automaton::{Automaton, InitializedAutomaton, Language, cfg::update::CFGCounterUpdate};

pub mod update;
pub mod vasscfg;

pub trait CFG:
    InitializedAutomaton + Automaton<E = CFGCounterUpdate> + Language<Letter = CFGCounterUpdate>
{
}
