use std::{fmt::Debug, hash::Hash, ops::Deref};

use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};

use crate::automaton::{cfg::update::CFGCounterUpdate, nfa::NFAEdge, vass::VASSEdge};

pub mod algorithms;
pub mod cfg;
pub mod dfa;
pub mod implicit_cfg_product;
pub mod index_map;
pub mod lsg;
pub mod ltc;
pub mod nfa;
pub mod path;
pub mod petri_net;
pub mod utils;
pub mod vass;

/// This trait represents types that can be used as node data in an automaton.
pub trait AutomatonNode: Debug + Clone + PartialEq + Eq + Hash {}
impl<T> AutomatonNode for T where T: Debug + Clone + PartialEq + Eq + Hash {}

/// This trait represents types that can be used as edge data in an automaton.
pub trait AutomatonEdge: Debug + Clone + PartialEq + Eq {
    /// A letter type that is used as the letters along the edges.
    type Letter: Debug + Clone + PartialEq + Eq + Hash + Ord;

    /// Checks whether an instance of self matches a given letter.
    fn matches(&self, letter: &Self::Letter) -> bool;
}

/// This trait allows creation of an [AutomatonEdge] from a letter.
pub trait FromLetter: AutomatonEdge {
    fn from_letter(letter: &Self::Letter) -> Self;
}

/// Implements [AutomatonEdge] and [FromLetter] for basic types that implement
/// [Copy] and [Eq].
macro_rules! impl_automaton_edge {
    ($t:ty) => {
        impl AutomatonEdge for $t {
            type Letter = $t;

            fn matches(&self, letter: &Self::Letter) -> bool {
                self == letter
            }
        }
        impl FromLetter for $t {
            fn from_letter(letter: &Self::Letter) -> Self {
                *letter
            }
        }
    };
}

impl_automaton_edge!(());
impl_automaton_edge!(u32);
impl_automaton_edge!(i32);
impl_automaton_edge!(u64);
impl_automaton_edge!(i64);
impl_automaton_edge!(usize);
impl_automaton_edge!(char);

impl_automaton_edge!(CFGCounterUpdate);

impl AutomatonEdge for String {
    type Letter = String;

    fn matches(&self, letter: &Self::Letter) -> bool {
        self == letter
    }
}
impl FromLetter for String {
    fn from_letter(letter: &Self::Letter) -> Self {
        letter.clone()
    }
}

impl<T: AutomatonEdge> AutomatonEdge for NFAEdge<T> {
    type Letter = T::Letter;

    fn matches(&self, letter: &Self::Letter) -> bool {
        match self {
            NFAEdge::Symbol(s) => s.matches(letter),
            NFAEdge::Epsilon => false,
        }
    }
}

impl<T: AutomatonEdge + FromLetter> AutomatonEdge for VASSEdge<T> {
    type Letter = T::Letter;

    fn matches(&self, letter: &Self::Letter) -> bool {
        self.data.matches(letter)
    }
}

/// This trait represents node or edge indices in an automaton.
/// The index space must be compact, so usually implementers of this trait are
/// just some wrapper type around some integer type. It must be possible to
/// construct an index from a [usize], to turn an index into a [usize], and
/// there must be a representation of an empty index.
pub trait GIndex: Debug + Copy + Clone + PartialEq + Eq + Hash + Ord {
    /// Create a new index from a [usize].
    fn new(index: usize) -> Self;
    /// Turn this index into a [usize] to e.g. index into a [Vec].
    fn index(self) -> usize;
    /// Create an empty index. This equates to [None].
    fn empty() -> Self;
}

impl GIndex for NodeIndex {
    fn new(index: usize) -> Self {
        NodeIndex::new(index)
    }

    fn index(self) -> usize {
        NodeIndex::index(self)
    }

    fn empty() -> Self {
        NodeIndex::end()
    }
}

impl GIndex for EdgeIndex {
    fn new(index: usize) -> Self {
        EdgeIndex::new(index)
    }

    fn index(self) -> usize {
        EdgeIndex::index(self)
    }

    fn empty() -> Self {
        EdgeIndex::end()
    }
}

impl GIndex for usize {
    fn new(index: usize) -> Self {
        index
    }

    fn index(self) -> usize {
        self
    }

    fn empty() -> Self {
        usize::MAX
    }
}

impl GIndex for u32 {
    fn new(index: usize) -> Self {
        index as u32
    }

    fn index(self) -> usize {
        self as usize
    }

    fn empty() -> Self {
        u32::MAX
    }
}

/// An iterator over a compact index space.
pub struct GIndexIterator<G: GIndex> {
    current: usize,
    end: usize,
    __marker: std::marker::PhantomData<G>,
}

impl<G: GIndex> GIndexIterator<G> {
    /// Create a new iterator over a compact index interval.
    pub fn new(start: usize, end: usize) -> Self {
        GIndexIterator {
            current: start,
            end,
            __marker: std::marker::PhantomData,
        }
    }
}

impl<G: GIndex> Iterator for GIndexIterator<G> {
    type Item = G;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            let index = G::new(self.current);
            self.current += 1;
            Some(index)
        } else {
            None
        }
    }
}

impl<G: GIndex> ExactSizeIterator for GIndexIterator<G> {
    fn len(&self) -> usize {
        self.end - self.current
    }
}

impl<G: GIndex> DoubleEndedIterator for GIndexIterator<G> {
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.current < self.end {
            self.end -= 1;
            let index = G::new(self.end);
            Some(index)
        } else {
            None
        }
    }
}

/// Helper struct for passing out immutable access to mutable references without
/// loosing the mutable reference.
pub struct Frozen<'a, G: 'a>(&'a mut G);

impl<G> Deref for Frozen<'_, G> {
    type Target = G;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl<'a, G> From<&'a mut G> for Frozen<'a, G> {
    fn from(value: &'a mut G) -> Self {
        Frozen(value)
    }
}

pub trait Letter: Debug + Clone + PartialEq + Eq + Hash + Ord {}

impl<T: Debug + Clone + PartialEq + Eq + Hash + Ord> Letter for T {}

pub trait Alphabet {
    type Letter: Letter;

    fn alphabet(&self) -> &[Self::Letter];
}

pub trait Automaton: Sized + Alphabet {
    /// The index type used to identify nodes.
    type NIndex: GIndex;
    /// The data type associated with nodes.
    type N: AutomatonNode;

    /// Returns the number of nodes in the automaton.
    /// It should be valid to index nodes from 0 to node_count() - 1
    fn node_count(&self) -> usize;

    /// Returns the node corresponding to the given index, or None if the index
    /// is invalid.
    fn get_node(&self, index: Self::NIndex) -> Option<&Self::N>;

    /// Returns the node corresponding to the given index, panicking if the
    /// index is invalid.
    fn get_node_unchecked(&self, index: Self::NIndex) -> &Self::N;

    /// Returns an iterator over all node indices in the automaton.
    fn iter_node_indices(&self) -> GIndexIterator<Self::NIndex> {
        GIndexIterator::new(0, self.node_count())
    }

    /// Returns a combined iterator over all nodes in the automaton, yielding
    /// (node_index, node_reference) pairs.
    fn iter_nodes<'a>(&'a self) -> impl Iterator<Item = (Self::NIndex, &'a Self::N)>
    where
        Self::N: 'a,
    {
        self.iter_node_indices()
            .filter_map(move |idx| self.get_node(idx).map(|n| (idx, n)))
    }
}

pub trait TransitionSystem: Automaton {
    fn successor(&self, node: Self::NIndex, letter: &Self::Letter) -> Option<Self::NIndex>;

    fn successors(&self, node: Self::NIndex) -> impl Iterator<Item = Self::NIndex>;
    fn predecessors(&self, node: Self::NIndex) -> impl Iterator<Item = Self::NIndex>;

    fn undirected_neighbors(&self, node: Self::NIndex) -> Vec<Self::NIndex> {
        let mut neighbors = self
            .successors(node)
            .chain(self.predecessors(node))
            .collect_vec();
        neighbors.sort();
        neighbors.dedup();
        neighbors
    }
}

impl<T: ExplicitEdgeAutomaton> TransitionSystem for T {
    fn successor(&self, node: Self::NIndex, letter: &Self::Letter) -> Option<Self::NIndex> {
        self.outgoing_edge_indices(node)
            .find(|e| self.get_edge_unchecked(*e).matches(letter))
            .map(|e| self.edge_target_unchecked(e))
    }

    fn successors(&self, node: Self::NIndex) -> impl Iterator<Item = Self::NIndex> {
        self.outgoing_edge_indices(node)
            .map(|e| self.edge_target_unchecked(e))
    }

    fn predecessors(&self, node: Self::NIndex) -> impl Iterator<Item = Self::NIndex> {
        self.incoming_edge_indices(node)
            .map(|e| self.edge_source_unchecked(e))
    }
}

/// The base trait for automata.
/// An automaton consists of nodes and edges, each identified by an index type.
/// We assert that the index types are continuous from 0 to n-1, where n is the
/// number of nodes/edges.
pub trait ExplicitEdgeAutomaton:
    Automaton<Letter = <<Self as ExplicitEdgeAutomaton>::E as AutomatonEdge>::Letter>
{
    /// The index type used to identify edges.
    type EIndex: GIndex;
    /// The data type associated with edges.
    type E: AutomatonEdge;

    /// Returns the number of edges in the automaton.
    /// It should be valid to index edges from 0 to edge_count() - 1.
    fn edge_count(&self) -> usize;

    /// Returns the edge corresponding to the given index, or None if the index
    /// is invalid.
    fn get_edge(&self, index: Self::EIndex) -> Option<&Self::E>;

    /// Returns the edge corresponding to the given index, panicking if the
    /// index is invalid.
    fn get_edge_unchecked(&self, index: Self::EIndex) -> &Self::E;

    /// Returns the source and target nodes of the given edge, or None if the
    /// edge index is invalid.
    fn edge_endpoints(&self, edge: Self::EIndex) -> Option<(Self::NIndex, Self::NIndex)>;
    /// Returns the source and target nodes of the given edge, panicking if the
    /// edge index is invalid.
    fn edge_endpoints_unchecked(&self, edge: Self::EIndex) -> (Self::NIndex, Self::NIndex);

    /// Returns the source node of the given edge, or None if the edge index is
    /// invalid.
    fn edge_source(&self, edge: Self::EIndex) -> Option<Self::NIndex> {
        self.edge_endpoints(edge).map(|(src, _)| src)
    }
    /// Returns the target node of the given edge, or None if the edge index is
    /// invalid.
    fn edge_target(&self, edge: Self::EIndex) -> Option<Self::NIndex> {
        self.edge_endpoints(edge).map(|(_, tgt)| tgt)
    }

    /// Returns the source node of the given edge, panicking if the edge index
    /// is invalid.
    fn edge_source_unchecked(&self, edge: Self::EIndex) -> Self::NIndex {
        self.edge_endpoints_unchecked(edge).0
    }
    /// Returns the target node of the given edge, panicking if the edge index
    /// is invalid.
    fn edge_target_unchecked(&self, edge: Self::EIndex) -> Self::NIndex {
        self.edge_endpoints_unchecked(edge).1
    }

    /// Returns an iterator over all edge indices in the automaton.
    fn iter_edge_indices(&self) -> GIndexIterator<Self::EIndex> {
        GIndexIterator::new(0, self.edge_count())
    }

    /// Returns a combined iterator over all edges in the automaton, yielding
    /// (edge_index, edge_reference) pairs.
    fn iter_edges<'a>(&'a self) -> impl Iterator<Item = (Self::EIndex, &'a Self::E)>
    where
        Self::E: 'a,
    {
        self.iter_edge_indices()
            .filter_map(move |idx| self.get_edge(idx).map(|e| (idx, e)))
    }

    /// Returns an iterator over the outgoing edge indices of the given node.
    fn outgoing_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex>;
    /// Returns an iterator over the incoming edge indices of the given node.
    fn incoming_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex>;
    /// Returns an iterator over the undirected edge indices of the given node.
    fn undirected_edge_indices(&self, node: Self::NIndex) -> impl Iterator<Item = Self::EIndex> {
        self.outgoing_edge_indices(node)
            .chain(self.incoming_edge_indices(node))
    }

    /// Returns an iterator over the edge indices directly connecting the given
    /// from and to nodes.
    fn connecting_edge_indices(
        &self,
        from: Self::NIndex,
        to: Self::NIndex,
    ) -> impl Iterator<Item = Self::EIndex>;
}

pub trait ModifiableAutomaton: ExplicitEdgeAutomaton {
    /// Adds a new node with the given data to the automaton.
    /// Returns the index of the newly added node.
    fn add_node(&mut self, data: Self::N) -> Self::NIndex;
    /// Adds a new edge from the given from node to the given to node with the
    /// given label. Returns the index of the newly added edge.
    fn add_edge(&mut self, from: Self::NIndex, to: Self::NIndex, label: Self::E) -> Self::EIndex;

    /// Removes the given node from the automaton.
    /// Also removes all edges connected to the node.
    /// For removing multiple nodes, consider using `retain_nodes` instead.
    fn remove_node(&mut self, node: Self::NIndex);
    /// Removes the given edge from the automaton.
    fn remove_edge(&mut self, edge: Self::EIndex);

    /// Retains only the nodes for which the given predicate returns true.
    fn retain_nodes<F>(&mut self, f: F)
    where
        F: Fn(Frozen<Self>, Self::NIndex) -> bool;
}

pub trait InitializedAutomaton: TransitionSystem {
    /// Returns the start node of the automaton, panicking if no start node is
    /// set.
    fn get_initial(&self) -> Self::NIndex;
    /// Sets the start node of the automaton.
    fn set_initial(&mut self, node: Self::NIndex);

    /// Returns true if the passed in node is accepting / a final node. Returns
    /// false otherwise.
    fn is_accepting(&self, node: Self::NIndex) -> bool;
}

pub trait SingleFinalStateAutomaton: InitializedAutomaton {
    /// Returns the final node of the automaton, panicking if no final node is
    /// set.
    fn get_final(&self) -> Self::NIndex;
    /// Sets the final node of the automaton.
    fn set_final(&mut self, node: Self::NIndex);

    fn is_accepting(&self, node: Self::NIndex) -> bool {
        node == self.get_final()
    }
}

/// The basic trait for anything that defines a language over a set alphabet.
pub trait Language: Alphabet {
    fn accepts<'a>(&self, input: impl IntoIterator<Item = &'a Self::Letter>) -> bool
    where
        Self::Letter: 'a;
}
