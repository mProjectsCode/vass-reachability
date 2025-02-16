use hashbrown::HashMap;
use petgraph::graph::EdgeIndex;

use crate::logger::{LogLevel, Logger};

#[derive(Debug, Clone)]
pub struct ParikhImage {
    pub image: HashMap<EdgeIndex, u32>,
}

impl ParikhImage {
    pub fn new(image: HashMap<EdgeIndex, u32>) -> Self {
        ParikhImage { image }
    }

    pub fn print(&self, logger: &Logger, level: LogLevel) {
        for (edge, count) in &self.image {
            logger.log(level.clone(), &format!("Edge: {}: {}", edge.index(), count));
        }
    }
}
