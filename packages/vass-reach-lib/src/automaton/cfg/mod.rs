use crate::automaton::{
    ExplicitEdgeAutomaton, InitializedAutomaton, Language, cfg::update::CFGCounterUpdate,
};

pub mod update;
pub mod vasscfg;

pub trait CFG:
    InitializedAutomaton
    + ExplicitEdgeAutomaton<E = CFGCounterUpdate>
    + Language<Letter = CFGCounterUpdate>
{
}
