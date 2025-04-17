use itertools::Itertools;
use petgraph::graph::{EdgeIndex, NodeIndex};

pub trait EdgeListLike {
    fn iter_edges(&self) -> impl Iterator<Item = EdgeIndex<u32>>;
    fn has_edge(&self, edge: EdgeIndex<u32>) -> bool;
    fn get_edge_label(&self, edge: EdgeIndex<u32>) -> String;
}

pub trait PathLike: EdgeListLike {
    fn iter_nodes(&self) -> impl Iterator<Item = NodeIndex<u32>>;
    fn has_node(&self, node: NodeIndex<u32>) -> bool;
    fn get_node_label(&self, node: NodeIndex<u32>) -> String;
    fn iter(&self) -> impl Iterator<Item = &(EdgeIndex<u32>, NodeIndex<u32>)>;
    fn iter_mut(&mut self) -> impl Iterator<Item = &mut (EdgeIndex<u32>, NodeIndex<u32>)>;
    fn first(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)>;
    fn last(&self) -> Option<&(EdgeIndex<u32>, NodeIndex<u32>)>;
    fn split_off(&mut self, index: usize) -> Self;
    fn slice(&self, index: usize) -> Self;
    fn slice_end(&self, index: usize) -> Self;
    fn add_pair(&mut self, edge: (EdgeIndex<u32>, NodeIndex<u32>));
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
    fn get(&self, index: usize) -> (EdgeIndex<u32>, NodeIndex<u32>);
    fn get_node(&self, index: usize) -> NodeIndex<u32>;
    fn get_edge(&self, index: usize) -> EdgeIndex<u32>;

    fn add(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.add_pair((edge, node));
    }

    fn to_word<T>(&self, get_edge_weight: impl Fn(EdgeIndex<u32>) -> T) -> Vec<T> {
        self.iter().map(|x| get_edge_weight(x.0)).collect_vec()
    }

    fn contains_node(&self, node: NodeIndex<u32>) -> bool {
        self.iter().any(|x| x.1 == node)
    }

    fn contains_edge(&self, edge: EdgeIndex<u32>) -> bool {
        self.iter().any(|x| x.0 == edge)
    }
}
