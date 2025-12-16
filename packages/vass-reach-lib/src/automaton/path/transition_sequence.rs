use std::{fmt::Display, vec::IntoIter};

use itertools::Itertools;

use crate::automaton::{
    GIndex,
    path::{
        Path,
        path_like::{EdgeIndexList, IndexPath},
    },
};

/// A transition sequence is a list of transitions, where each transition is a
/// tuple of an edge and a node. The edge is the edge taken and the node is the
/// node reached by that edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionSequence<NIndex: GIndex, EIndex: GIndex>(Vec<(EIndex, NIndex)>);

impl<NIndex: GIndex, EIndex: GIndex> TransitionSequence<NIndex, EIndex> {
    pub fn new() -> Self {
        TransitionSequence(Vec::new())
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

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EIndex) -> String) -> String {
        self.0
            .iter()
            .map(|x| {
                format!(
                    "--({:?} | {})-> {:?}",
                    x.0.index(),
                    get_edge_string(x.0),
                    x.1.index()
                )
            })
            .join(" ")
    }

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn append(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }
}

impl<NIndex: GIndex, EIndex: GIndex> EdgeIndexList<NIndex, EIndex>
    for TransitionSequence<NIndex, EIndex>
{
    fn iter_edges(&self) -> impl Iterator<Item = EIndex> {
        self.iter().map(|x| x.0)
    }

    fn has_edge(&self, edge: EIndex) -> bool {
        self.0.iter().any(|x| x.0 == edge)
    }
}

impl<NIndex: GIndex, EIndex: GIndex> IndexPath<NIndex, EIndex>
    for TransitionSequence<NIndex, EIndex>
{
    fn iter_nodes(&self) -> impl Iterator<Item = NIndex> {
        self.iter().map(|x| x.1)
    }

    fn has_node(&self, node: NIndex) -> bool {
        self.0.iter().any(|x| x.1 == node)
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a,
    {
        self.0.iter()
    }

    fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a,
    {
        self.0.iter_mut()
    }

    fn first(&self) -> Option<&(EIndex, NIndex)> {
        self.0.first()
    }

    fn last(&self) -> Option<&(EIndex, NIndex)> {
        self.0.last()
    }

    fn split_off(&mut self, index: usize) -> Self {
        TransitionSequence(self.0.split_off(index))
    }

    fn slice(&self, index: usize) -> Self {
        TransitionSequence(self.0[..=index].to_vec())
    }

    fn slice_end(&self, index: usize) -> Self {
        TransitionSequence(self.0[index..].to_vec())
    }

    fn add_pair(&mut self, edge: (EIndex, NIndex)) {
        self.0.push(edge);
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn get(&self, index: usize) -> (EIndex, NIndex) {
        self.0[index]
    }

    fn get_node(&self, index: usize) -> NIndex {
        self.0[index].1
    }

    fn get_edge(&self, index: usize) -> EIndex {
        self.0[index].0
    }
}

impl<NIndex: GIndex, EIndex: GIndex> From<Vec<(EIndex, NIndex)>>
    for TransitionSequence<NIndex, EIndex>
{
    fn from(vec: Vec<(EIndex, NIndex)>) -> Self {
        TransitionSequence(vec)
    }
}

impl<NIndex: GIndex, EIndex: GIndex> From<Path<NIndex, EIndex>>
    for TransitionSequence<NIndex, EIndex>
{
    fn from(path: Path<NIndex, EIndex>) -> Self {
        path.transitions
    }
}

impl<NIndex: GIndex, EIndex: GIndex> IntoIterator for TransitionSequence<NIndex, EIndex> {
    type Item = (EIndex, NIndex);
    type IntoIter = IntoIter<(EIndex, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a, NIndex: GIndex, EIndex: GIndex> IntoIterator for &'a TransitionSequence<NIndex, EIndex> {
    type Item = &'a (EIndex, NIndex);
    type IntoIter = std::slice::Iter<'a, (EIndex, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a, NIndex: GIndex, EIndex: GIndex> IntoIterator
    for &'a mut TransitionSequence<NIndex, EIndex>
{
    type Item = &'a mut (EIndex, NIndex);
    type IntoIter = std::slice::IterMut<'a, (EIndex, NIndex)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl<NIndex: GIndex, EIndex: GIndex> Default for TransitionSequence<NIndex, EIndex> {
    fn default() -> Self {
        Self::new()
    }
}

impl<NIndex: GIndex, EIndex: GIndex> Display for TransitionSequence<NIndex, EIndex> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.0
                .iter()
                .map(|x| format!("--({:?})-> {:?}", x.0.index(), x.1.index()))
                .join(" ")
        )
    }
}
