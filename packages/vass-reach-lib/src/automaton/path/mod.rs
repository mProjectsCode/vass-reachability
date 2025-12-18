use std::{fmt::Display, vec::IntoIter};

use crate::automaton::{
    GIndex, Letter, TransitionSystem, path::transition_sequence::TransitionSequence,
};

pub mod parikh_image;
pub mod transition_sequence;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Path<NIndex: GIndex, L: Letter> {
    transitions: TransitionSequence<NIndex, L>,
    start: NIndex,
}

impl<NIndex: GIndex, L: Letter> Path<NIndex, L> {
    pub fn new(start_index: NIndex) -> Self {
        Path {
            transitions: TransitionSequence::new(),
            start: start_index,
        }
    }

    pub fn new_from_sequence(
        start_index: NIndex,
        transitions: TransitionSequence<NIndex, L>,
    ) -> Self {
        Path {
            transitions,
            start: start_index,
        }
    }

    pub fn from_word<'a>(
        start_index: NIndex,
        word: impl IntoIterator<Item = &'a L>,
        graph: &impl TransitionSystem<NIndex = NIndex, Letter = L>,
    ) -> anyhow::Result<Self>
    where
        L: 'a,
    {
        let mut path = Path::new(start_index);

        for letter in word {
            path.take_edge(letter.clone(), graph)?;
        }

        Ok(path)
    }

    pub fn add(&mut self, letter: L, node: NIndex) {
        self.transitions.add(letter, node);
    }

    pub fn take_edge(
        &mut self,
        letter: L,
        graph: &impl TransitionSystem<NIndex = NIndex, Letter = L>,
    ) -> anyhow::Result<()> {
        let successor = graph.successor(self.end(), &letter).ok_or_else(|| {
            anyhow::anyhow!(format!(
                "path failed to take letter {:?}, no suitable successor found for end node {:?}",
                letter,
                self.end()
            ))
        })?;
        self.add(letter, successor);
        Ok(())
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

        for (letter, target) in self.transitions.iter() {
            current_part.add(letter.clone(), *target);

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

        for (letter, target) in self.transitions.iter() {
            current_part.add(letter.clone(), *target);

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

    pub fn to_fancy_string(&self) -> String {
        format!(
            "{:?} {}",
            self.start.index(),
            self.transitions.to_fancy_string()
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

    pub fn iter_letters(&self) -> impl Iterator<Item = &L> {
        self.transitions.iter_letters()
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = NIndex> {
        vec![self.start]
            .into_iter()
            .chain(self.transitions.iter_nodes())
    }

    pub fn has_node(&self, node: NIndex) -> bool {
        self.start == node || self.transitions.contains_node(node)
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a (L, NIndex)>
    where
        L: 'a,
        NIndex: 'a,
    {
        self.transitions.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut (L, NIndex)>
    where
        L: 'a,
        NIndex: 'a,
    {
        self.transitions.iter_mut()
    }

    pub fn first(&self) -> Option<&(L, NIndex)> {
        self.transitions.first()
    }

    pub fn last(&self) -> Option<&(L, NIndex)> {
        self.transitions.last()
    }

    pub fn split_off(&mut self, index: usize) -> Self {
        Path {
            transitions: self.transitions.split_off(index),
            start: self.start,
        }
    }

    pub fn slice(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice(index),
            start: self.start,
        }
    }

    pub fn slice_end(&self, index: usize) -> Self {
        Path {
            transitions: self.transitions.slice_end(index),
            start: self.start,
        }
    }

    pub fn len(&self) -> usize {
        self.transitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }

    pub fn contains_node(&self, node: NIndex) -> bool {
        self.start == node || self.transitions.contains_node(node)
    }

    pub fn get(&self, index: usize) -> &(L, NIndex) {
        self.transitions.get(index)
    }

    pub fn get_node(&self, index: usize) -> NIndex {
        self.transitions.get_node(index)
    }

    pub fn get_letter(&self, index: usize) -> &L {
        self.transitions.get_letter(index)
    }
}

impl<NIndex: GIndex, L: Letter> Display for Path<NIndex, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?} {}", self.start.index(), self.transitions)
    }
}

impl<NIndex: GIndex, L: Letter> IntoIterator for Path<NIndex, L> {
    type Item = (L, NIndex);
    type IntoIter = IntoIter<(L, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.transitions.into_iter()
    }
}
