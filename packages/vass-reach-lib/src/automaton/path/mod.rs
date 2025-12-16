use std::{fmt::Display, vec::IntoIter};

use crate::automaton::{
    Automaton, GIndex, InitializedAutomaton,
    cfg::{CFG, update::CFGCounterUpdate},
    path::{
        path_like::{EdgeIndexList, IndexPath},
        transition_sequence::TransitionSequence,
    },
};

pub mod parikh_image;
pub mod path_like;
pub mod transition_sequence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<NIndex: GIndex, EIndex: GIndex> {
    transitions: TransitionSequence<NIndex, EIndex>,
    start: NIndex,
}

impl<NIndex: GIndex, EIndex: GIndex> Path<NIndex, EIndex> {
    pub fn new(start_index: NIndex) -> Self {
        Path {
            transitions: TransitionSequence::new(),
            start: start_index,
        }
    }

    pub fn new_from_sequence(
        start_index: NIndex,
        transitions: TransitionSequence<NIndex, EIndex>,
    ) -> Self {
        Path {
            transitions,
            start: start_index,
        }
    }

    pub fn from_edges<'a>(
        start_index: NIndex,
        edges: impl IntoIterator<Item = &'a EIndex>,
        graph: &impl Automaton<NIndex = NIndex, EIndex = EIndex>,
    ) -> Self
    where
        EIndex: 'a,
    {
        let mut path = Path::new(start_index);

        path.take_edges(edges, graph);

        path
    }

    pub fn from_word<'a, A: InitializedAutomaton<NIndex = NIndex, EIndex = EIndex>>(
        word: impl IntoIterator<Item = &'a A::E>,
        graph: &A,
    ) -> anyhow::Result<Self>
    where
        A::E: 'a,
    {
        let mut path = Path::new(graph.get_initial());

        for letter in word {
            let edge = graph
                .outgoing_edge_indices(path.end())
                .find(|e| graph.get_edge_unchecked(*e) == letter);
            let Some(edge) = edge else {
                anyhow::bail!(
                    "Found no edge with letter {:?} from node {:?}",
                    letter,
                    path.end()
                )
            };

            path.add(edge, graph.edge_target_unchecked(edge));
        }

        Ok(path)
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        self.transitions.has_loop()
    }

    pub fn start(&self) -> NIndex {
        self.start
    }

    pub fn end(&self) -> NIndex {
        self.transitions.end().unwrap_or(self.start)
    }

    /// Whether the path contains a specific node.
    /// This does **not** check the start node.
    pub fn transitions_contain_node(&self, node: NIndex) -> bool {
        self.transitions.contains_node(node)
    }

    pub fn split_at_node(self, node: NIndex) -> Vec<Self> {
        if self.transitions.is_empty() || !self.contains_node(node) {
            return vec![self];
        }

        let mut parts = vec![];
        let mut current_part = Path::new(self.start);

        for (edge, target) in self.transitions.iter() {
            current_part.add_pair((*edge, *target));

            if *target == node {
                parts.push(current_part);
                current_part = Path::new(node);
            }
        }

        if !current_part.is_empty() {
            parts.push(current_part);
        }

        parts
    }

    pub fn split_at_nodes(self, nodes: &[NIndex]) -> Vec<Self> {
        // for splitting to have an effect, the path needs to be non-empty and contain
        // at least one of the nodes
        if self.transitions.is_empty() || nodes.iter().all(|n| !self.contains_node(*n)) {
            return vec![self];
        }

        let mut parts = vec![];
        let mut current_part = Path::new(self.start);

        for (edge, target) in self.transitions.iter() {
            current_part.add_pair((*edge, *target));

            for node in nodes {
                if *node == *target {
                    parts.push(current_part);
                    current_part = Path::new(*node);
                    break;
                }
            }
        }

        // if !current_part.is_empty() {
        parts.push(current_part);
        // }

        parts
    }

    pub fn iter_cfg_updates<'a>(
        &'a self,
        cfg: &'a impl CFG<NIndex = NIndex, EIndex = EIndex>,
    ) -> impl Iterator<Item = CFGCounterUpdate> + 'a {
        self.transitions
            .iter()
            .map(move |(edge, _)| *cfg.get_edge_unchecked(*edge))
    }

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EIndex) -> String) -> String {
        format!(
            "{:?} {}",
            self.start.index(),
            self.transitions.to_fancy_string(get_edge_string)
        )
    }

    pub fn concatenate(&mut self, other: Self) {
        assert_eq!(
            self.end(),
            other.start,
            "Paths can only be concatenated if the end of the first matches the start of the second"
        );
        self.transitions.append(other.transitions);
    }
}

impl<NIndex: GIndex, EIndex: GIndex> EdgeIndexList<NIndex, EIndex> for Path<NIndex, EIndex> {
    fn iter_edges(&self) -> impl Iterator<Item = EIndex> {
        self.transitions.iter_edges()
    }

    fn has_edge(&self, edge: EIndex) -> bool {
        self.transitions.has_edge(edge)
    }
}

impl<NIndex: GIndex, EIndex: GIndex> IndexPath<NIndex, EIndex> for Path<NIndex, EIndex> {
    fn iter_nodes(&self) -> impl Iterator<Item = NIndex> {
        vec![self.start]
            .into_iter()
            .chain(self.transitions.iter_nodes())
    }

    fn has_node(&self, node: NIndex) -> bool {
        self.start == node || self.transitions.has_node(node)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a,
    {
        self.transitions.iter()
    }

    fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a,
    {
        self.transitions.iter_mut()
    }

    fn first(&self) -> Option<&(EIndex, NIndex)> {
        self.transitions.first()
    }

    fn last(&self) -> Option<&(EIndex, NIndex)> {
        self.transitions.last()
    }

    fn split_off(&mut self, index: usize) -> Self {
        Path {
            transitions: self.transitions.split_off(index),
            start: self.start,
        }
    }

    fn slice(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice(index),
            start: self.start,
        }
    }

    fn slice_end(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice_end(index),
            start: self.start,
        }
    }

    fn add_pair(&mut self, edge: (EIndex, NIndex)) {
        self.transitions.add_pair(edge);
    }

    fn len(&self) -> usize {
        self.transitions.len()
    }

    fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    fn contains_node(&self, node: NIndex) -> bool {
        self.start == node || self.transitions.contains_node(node)
    }

    fn get(&self, index: usize) -> (EIndex, NIndex) {
        self.transitions.get(index)
    }

    fn get_node(&self, index: usize) -> NIndex {
        self.transitions.get_node(index)
    }

    fn get_edge(&self, index: usize) -> EIndex {
        self.transitions.get_edge(index)
    }
}

impl<NIndex: GIndex, EIndex: GIndex> Display for Path<NIndex, EIndex> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}", self.start.index(), self.transitions)
    }
}

impl<NIndex: GIndex, EIndex: GIndex> IntoIterator for Path<NIndex, EIndex> {
    type Item = (EIndex, NIndex);
    type IntoIter = IntoIter<(EIndex, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.transitions.into_iter()
    }
}
