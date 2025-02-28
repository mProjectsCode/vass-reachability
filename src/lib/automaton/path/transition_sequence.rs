use std::{fmt::Display, vec::IntoIter};

use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};

/// A transition sequence is a list of transitions, where each transition is a
/// tuple of an edge and a node The edge is the edge taken and the node is the
/// node reached by that edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionSequence(Vec<(EdgeIndex<u32>, NodeIndex<u32>)>);

impl TransitionSequence {
    pub fn new() -> Self {
        TransitionSequence(Vec::new())
    }

    pub fn add(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.0.push((edge, node));
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn slice(&self, index: usize) -> Self {
        TransitionSequence(self.0[..=index].to_vec())
    }

    pub fn to_word<T>(&self, get_edge_weight: impl Fn(EdgeIndex<u32>) -> T) -> Vec<T> {
        self.0.iter().map(|x| get_edge_weight(x.0)).collect_vec()
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

    pub fn end(&self) -> Option<NodeIndex<u32>> {
        self.0.last().map(|x| x.1)
    }

    pub fn last(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.0.last()
    }

    pub fn split_off(&mut self, index: usize) -> Self {
        TransitionSequence(self.0.split_off(index))
    }

    pub fn iter(&self) -> std::slice::Iter<(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.0.iter()
    }

    pub fn to_fancy_string(&self, get_edge_string: impl Fn(EdgeIndex) -> String) -> String {
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
}

impl From<Vec<(EdgeIndex<u32>, NodeIndex<u32>)>> for TransitionSequence {
    fn from(vec: Vec<(EdgeIndex<u32>, NodeIndex<u32>)>) -> Self {
        TransitionSequence(vec)
    }
}

impl IntoIterator for TransitionSequence {
    type Item = (EdgeIndex<u32>, NodeIndex<u32>);
    type IntoIter = IntoIter<(EdgeIndex<u32>, NodeIndex<u32>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a TransitionSequence {
    type Item = &'a (EdgeIndex<u32>, NodeIndex<u32>);
    type IntoIter = std::slice::Iter<'a, (EdgeIndex<u32>, NodeIndex<u32>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl<'a> IntoIterator for &'a mut TransitionSequence {
    type Item = &'a mut (EdgeIndex<u32>, NodeIndex<u32>);
    type IntoIter = std::slice::IterMut<'a, (EdgeIndex<u32>, NodeIndex<u32>)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter_mut()
    }
}

impl Default for TransitionSequence {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for TransitionSequence {
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
