use itertools::Itertools;

use crate::automaton::{Automaton, GIndex};

pub trait EdgeIndexList<NIndex: GIndex, EIndex: GIndex> {
    fn iter_edges(&self) -> impl Iterator<Item = EIndex>;
    fn has_edge(&self, edge: EIndex) -> bool;
}

pub trait IndexPath<NIndex: GIndex, EIndex: GIndex>: EdgeIndexList<NIndex, EIndex> {
    fn iter_nodes(&self) -> impl Iterator<Item = NIndex>;
    fn has_node(&self, node: NIndex) -> bool;
    fn iter<'a>(&'a self) -> impl Iterator<Item = &'a (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a;
    fn iter_mut<'a>(&'a mut self) -> impl Iterator<Item = &'a mut (EIndex, NIndex)>
    where
        EIndex: 'a,
        NIndex: 'a;
    fn first(&self) -> Option<&(EIndex, NIndex)>;
    fn last(&self) -> Option<&(EIndex, NIndex)>;
    fn split_off(&mut self, index: usize) -> Self;
    fn slice(&self, index: usize) -> Self;
    fn slice_end(&self, index: usize) -> Self;
    fn add_pair(&mut self, edge: (EIndex, NIndex));
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn get(&self, index: usize) -> (EIndex, NIndex);
    fn get_node(&self, index: usize) -> NIndex;
    fn get_edge(&self, index: usize) -> EIndex;

    fn add(&mut self, edge: EIndex, node: NIndex) {
        self.add_pair((edge, node));
    }

    /// A safer alternative to `add` that checks that the edge's source matches
    /// the current path end.
    fn take_edge(
        &mut self,
        edge: EIndex,
        graph: &impl Automaton<NIndex = NIndex, EIndex = EIndex>,
    ) {
        let endpoints = graph.edge_endpoints(edge).expect("Edge index must exist");
        if let Some(last) = self.last() {
            assert_eq!(last.1, endpoints.0, "Edge source must match path end");
        }
        self.add(edge, endpoints.1);
    }

    fn take_edges<'a>(
        &mut self,
        edges: impl IntoIterator<Item = &'a EIndex>,
        graph: &impl Automaton<NIndex = NIndex, EIndex = EIndex>,
    ) where
        EIndex: 'a,
    {
        for edge in edges {
            self.take_edge(*edge, graph);
        }
    }

    fn to_word<T>(&self, get_edge_weight: impl Fn(EIndex) -> T) -> Vec<T> {
        self.iter().map(|x| get_edge_weight(x.0)).collect_vec()
    }

    fn contains_node(&self, node: NIndex) -> bool {
        self.iter().any(|x| x.1 == node)
    }

    fn contains_edge(&self, edge: EIndex) -> bool {
        self.iter().any(|x| x.0 == edge)
    }
}
