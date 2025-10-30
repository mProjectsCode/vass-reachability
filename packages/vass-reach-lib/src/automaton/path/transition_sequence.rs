use std::{fmt::Display, vec::IntoIter};

use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::automaton::path::{Path, path_like::{EdgeListLike, PathLike}};


/// A transition sequence is a list of transitions, where each transition is a
/// tuple of an edge and a node. The edge is the edge taken and the node is the
/// node reached by that edge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransitionSequence(Vec<(EdgeIndex<u32>, NodeIndex<u32>)>);

impl TransitionSequence {
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

    pub fn end(&self) -> Option<NodeIndex<u32>> {
        self.0.last().map(|x| x.1)
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

    pub fn reverse(&mut self) {
        self.0.reverse();
    }

    pub fn append(&mut self, mut other: Self) {
        self.0.append(&mut other.0);
    }
}

impl EdgeListLike for TransitionSequence {
    fn iter_edges(&self) -> impl Iterator<Item = EdgeIndex<u32>> {
        self.iter().map(|x| x.0)
    }

    fn has_edge(&self, edge: EdgeIndex<u32>) -> bool {
        self.0.iter().any(|x| x.0 == edge)
    }

    fn get_edge_label(&self, edge: EdgeIndex<u32>) -> String {
        edge.index().to_string()
    }
}

impl PathLike for TransitionSequence {
    fn iter_nodes(&self) -> impl Iterator<Item = NodeIndex<u32>> {
        self.iter().map(|x| x.1)
    }

    fn has_node(&self, node: NodeIndex<u32>) -> bool {
        self.0.iter().any(|x| x.1 == node)
    }

    fn get_node_label(&self, node: NodeIndex<u32>) -> String {
        node.index().to_string()
    }

    fn iter(&self) -> impl Iterator<Item = &(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.0.iter()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (EdgeIndex<u32>, NodeIndex<u32>)> {
        self.0.iter_mut()
    }

    fn first(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
        self.0.first()
    }

    fn last(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)> {
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

    fn add_pair(&mut self, edge: (EdgeIndex<u32>, NodeIndex<u32>)) {
        self.0.push(edge);
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    fn get(&self, index: usize) -> (EdgeIndex<u32>, NodeIndex<u32>) {
        self.0[index]
    }

    fn get_node(&self, index: usize) -> NodeIndex<u32> {
        self.0[index].1
    }

    fn get_edge(&self, index: usize) -> EdgeIndex<u32> {
        self.0[index].0
    }
}

impl From<Vec<(EdgeIndex<u32>, NodeIndex<u32>)>> for TransitionSequence {
    fn from(vec: Vec<(EdgeIndex<u32>, NodeIndex<u32>)>) -> Self {
        TransitionSequence(vec)
    }
}

impl From<Path> for TransitionSequence {
    fn from(path: Path) -> Self {
        path.transitions
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
