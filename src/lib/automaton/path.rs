use petgraph::{graph::EdgeIndex, graph::NodeIndex};

use super::ltc::LTC;

#[derive(Debug, Clone, PartialEq)]
pub struct Path {
    pub edges: Vec<EdgeIndex<u32>>,
    pub start: NodeIndex<u32>,
    pub end: NodeIndex<u32>,
}

impl Path {
    pub fn new(start_index: NodeIndex<u32>) -> Self {
        Path {
            edges: Vec::new(),
            start: start_index,
            end: start_index,
        }
    }

    /// Take an edge to a new node
    pub fn add_edge(&mut self, edge: EdgeIndex<u32>, node: NodeIndex<u32>) {
        self.edges.push(edge);
        self.end = node;
    }

    /// Checks if a path has a loop by checking if an edge in taken twice
    pub fn has_loop(&self) -> bool {
        let mut visited = vec![];

        for edge in &self.edges {
            if visited.contains(&edge) {
                return true;
            }

            visited.push(edge);
        }

        false
    }

    pub fn to_ltc(&self) -> LTC {
        unimplemented!()
    }
}
