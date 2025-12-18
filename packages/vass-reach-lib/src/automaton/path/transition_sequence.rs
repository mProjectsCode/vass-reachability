use std::{fmt::Display, vec::IntoIter};

use itertools::Itertools;

use crate::automaton::{GIndex, Letter, path::Path};

/// A transition sequence is a list of transitions, where each transition is a
/// tuple of an edge and a node. The edge is the edge taken and the node is the
/// node reached by that edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionSequence<NIndex: GIndex, L: Letter>(Vec<(L, NIndex)>);

impl<NIndex: GIndex, L: Letter> TransitionSequence<NIndex, L> {
    pub fn new() -> Self {
        TransitionSequence(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn add(&mut self, letter: L, node: NIndex) {
        self.0.push((letter, node));
    }

    pub fn has_loop(&self) -> bool {
        let mut visited = vec![];

        for (_, node) in &self.0 {
            if visited.contains(node) {
                return true;
            }

            visited.push(*node);
        }

        false
    }

    pub fn end(&self) -> Option<NIndex> {
        self.0.last().map(|x| x.1)
    }

    pub fn to_fancy_string(&self) -> String {
        self.0
            .iter()
            .map(|x| format!("--({:?})-> {:?}", x.0, x.1.index()))
            .join(" ")
    }

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn append(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }

    pub fn iter(&self) -> impl Iterator<Item = &(L, NIndex)> {
        self.0.iter()
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &mut (L, NIndex)> {
        self.0.iter_mut()
    }

    pub fn iter_nodes(&self) -> impl Iterator<Item = NIndex> {
        self.iter().map(|x| x.1)
    }

    pub fn iter_letters(&self) -> impl Iterator<Item = &L> {
        self.iter().map(|x| &x.0)
    }

    pub fn contains_node(&self, node: NIndex) -> bool {
        self.iter_nodes().contains(&node)
    }

    pub fn first(&self) -> Option<&(L, NIndex)> {
        self.0.first()
    }

    pub fn last(&self) -> Option<&(L, NIndex)> {
        self.0.last()
    }

    pub fn split_off(&mut self, index: usize) -> Self {
        TransitionSequence(self.0.split_off(index))
    }

    pub fn slice(&self, index: usize) -> Self {
        TransitionSequence(self.0[..=index].to_vec())
    }

    pub fn slice_end(&self, index: usize) -> Self {
        TransitionSequence(self.0[index..].to_vec())
    }

    pub fn get(&self, index: usize) -> &(L, NIndex) {
        &self.0[index]
    }

    pub fn get_node(&self, index: usize) -> NIndex {
        self.0[index].1
    }

    pub fn get_letter(&self, index: usize) -> &L {
        &self.0[index].0
    }
}

impl<NIndex: GIndex, L: Letter> From<Vec<(L, NIndex)>> for TransitionSequence<NIndex, L> {
    fn from(vec: Vec<(L, NIndex)>) -> Self {
        TransitionSequence(vec)
    }
}

impl<NIndex: GIndex, L: Letter> From<Path<NIndex, L>> for TransitionSequence<NIndex, L> {
    fn from(path: Path<NIndex, L>) -> Self {
        path.transitions
    }
}

impl<NIndex: GIndex, L: Letter> IntoIterator for TransitionSequence<NIndex, L> {
    type Item = (L, NIndex);
    type IntoIter = IntoIter<(L, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, NIndex: GIndex, L: Letter> IntoIterator for &'a TransitionSequence<NIndex, L> {
    type Item = &'a (L, NIndex);
    type IntoIter = std::slice::Iter<'a, (L, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, NIndex: GIndex, L: Letter> IntoIterator for &'a mut TransitionSequence<NIndex, L> {
    type Item = &'a mut (L, NIndex);
    type IntoIter = std::slice::IterMut<'a, (L, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<NIndex: GIndex, L: Letter> Default for TransitionSequence<NIndex, L> {
    fn default() -> Self {
        Self::new()
    }
}

impl<NIndex: GIndex, L: Letter> Display for TransitionSequence<NIndex, L> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|x| format!("--({:?})-> {:?}", x.0, x.1.index()))
                .join(" ")
        )
    }
}
