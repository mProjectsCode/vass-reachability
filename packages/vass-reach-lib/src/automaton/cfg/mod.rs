use crate::automaton::{InitializedAutomaton, Language, cfg::update::CFGCounterUpdate};

pub mod update;
pub mod vasscfg;

pub trait CFG:
    InitializedAutomaton<E = CFGCounterUpdate> + Language<Letter = CFGCounterUpdate>
{
}
