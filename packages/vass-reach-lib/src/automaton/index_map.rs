use petgraph::{csr::IndexType, graph::{EdgeIndex, NodeIndex}};

pub trait IndexMapData: Clone + PartialEq {
    fn empty() -> Self;
}

impl<T: Clone + PartialEq> IndexMapData for Option<T> {
    fn empty() -> Self {
        None
    }
}

impl<T: Clone + PartialEq> IndexMapData for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }
}

impl IndexMapData for usize {
    fn empty() -> Self {
        0
    }
}

impl IndexMapData for u32 {
    fn empty() -> Self {
        0
    }
}

pub trait IndexMapKey {
    fn new(index: usize) -> Self;
    fn index(self) -> usize;
}

impl<T: IndexType> IndexMapKey for NodeIndex<T> {
    fn new(index: usize) -> Self {
        NodeIndex::new(index)
    }

    fn index(self) -> usize {
        NodeIndex::index(self)
    }
}

impl<T: IndexType> IndexMapKey for EdgeIndex<T> {
    fn new(index: usize) -> Self {
        EdgeIndex::new(index)
    }

    fn index(self) -> usize {
        EdgeIndex::index(self)
    }
}

pub struct IndexMap<K: IndexMapKey, V: IndexMapData> {
    data: Vec<V>,
    _marker: std::marker::PhantomData<K>,
}

impl<K: IndexMapKey, V: IndexMapData> IndexMap<K, V> {
    pub fn new(max_index: usize) -> Self {
        IndexMap {
            data: vec![V::empty(); max_index],
            _marker: std::marker::PhantomData,
        }
    }

    pub fn has_key(&self, key: K) -> bool {
        let index = key.index();

        index < self.data.len() && self.data[index] != V::empty()
    }

    pub fn get(&self, key: K) -> &V {
        &self.data[key.index()]
    }

    pub fn get_mut(&mut self, key: K) -> &mut V {
        &mut self.data[key.index()]
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.data[key.index()] = value;
    }
}