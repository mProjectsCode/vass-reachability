use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::automaton::{
    Deterministic, ExplicitEdgeAutomaton, InitializedAutomaton, Language, TransitionSystem,
    cfg::update::CFGCounterUpdate,
};

pub mod modulo;
pub mod update;
pub mod vasscfg;

pub trait CFG:
    InitializedAutomaton<Deterministic>
    + TransitionSystem<Deterministic>
    + Language<Letter = CFGCounterUpdate>
{
}

pub trait ExplicitEdgeCFG:
    CFG
    + ExplicitEdgeAutomaton<
        Deterministic,
        NIndex = NodeIndex,
        EIndex = EdgeIndex,
        E = CFGCounterUpdate,
    >
{
}

impl<T> ExplicitEdgeCFG for T where
    T: CFG
        + ExplicitEdgeAutomaton<
            Deterministic,
            NIndex = NodeIndex,
            EIndex = EdgeIndex,
            E = CFGCounterUpdate,
        >
{
}
