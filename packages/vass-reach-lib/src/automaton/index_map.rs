use std::fmt::Debug;

use petgraph::{
    csr::IndexType,
    graph::{EdgeIndex, NodeIndex},
};

/// Trait for items that can be used as values in an IndexMap.
/// The type must have an "empty" value that represents the absence of a value
/// in the map.
pub trait IndexMapData: Clone + PartialEq {
    fn empty() -> Self;
}

impl<T: Clone + PartialEq> IndexMapData for Vec<T> {
    fn empty() -> Self {
        Vec::new()
    }
}

impl<T: Clone + PartialEq> IndexMapData for Box<[T]> {
    fn empty() -> Self {
        Box::new([])
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

impl<T: IndexType> IndexMapData for NodeIndex<T> {
    fn empty() -> Self {
        NodeIndex::end()
    }
}

impl<T: IndexType> IndexMapData for EdgeIndex<T> {
    fn empty() -> Self {
        EdgeIndex::end()
    }
}

/// Trait for keys that can be used in an IndexMap.
/// The key must be able to be constructed from a usize index and provide its
/// usize index.
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

/// A vector based map from keys of type K to values of type V.
/// The maximum key index must be known at map creation time.
/// Attempts to access keys out of range will in most cases panic.
#[derive(Debug, Clone)]
pub struct IndexMap<K: IndexMapKey, V: IndexMapData> {
    data: Box<[V]>,
    _marker: std::marker::PhantomData<K>,
}

impl<K: IndexMapKey, V: IndexMapData> IndexMap<K, V> {
    pub fn new(max_index: usize) -> Self {
        IndexMap {
            data: vec![V::empty(); max_index].into_boxed_slice(),
            _marker: std::marker::PhantomData,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn has_key(&self, key: K) -> bool {
        let index = key.index();

        index < self.data.len() && self.data[index] != V::empty()
    }

    /// Get the value associated with the key.
    /// Panics if the key is out of range.
    ///
    /// If the key is not present, returns the empty value for V.
    pub fn get(&self, key: K) -> &V {
        &self.data[key.index()]
    }

    /// Get the value associated with the key if present.
    /// Returns None if the key is not present or out of range.
    pub fn get_option(&self, key: K) -> Option<&V> {
        let index = key.index();

        if index < self.data.len() {
            let value = &self.data[index];
            if *value != V::empty() {
                return Some(value);
            }
        }

        None
    }

    /// Get a mutable reference to the value associated with the key.
    /// Panics if the key is out of range.
    pub fn get_mut(&mut self, key: K) -> &mut V {
        &mut self.data[key.index()]
    }

    /// Insert the value associated with the key.
    /// Overwrites any existing value.
    /// Panics if the key is out of range.
    pub fn insert(&mut self, key: K, value: V) {
        self.data[key.index()] = value;
    }

    /// Delete the value associated with the key.
    /// Panics if the key is out of range.
    pub fn delete(&mut self, key: K) {
        self.data[key.index()] = V::empty();
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.data
            .into_iter()
            .enumerate()
            .map(|(i, v)| (K::new(i), v))
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (K, &'a V)> + 'a {
        self.data.iter().enumerate().map(|(i, v)| (K::new(i), v))
    }

    /// Create a new IndexMap by mapping the filled values of this map using the
    /// provided function.
    pub fn map<F, V2: IndexMapData>(&self, f: F) -> IndexMap<K, V2>
    where
        F: Fn(&V) -> V2,
    {
        let empty = V::empty();

        let data = self
            .data
            .iter()
            .map(|v| if v != &empty { f(v) } else { V2::empty() })
            .collect();

        IndexMap {
            data,
            _marker: std::marker::PhantomData,
        }
    }

    /// Create a new OptionIndexMap by mapping the filled values of this map
    /// using the provided function.
    pub fn map_option<F, V2>(&self, f: F) -> OptionIndexMap<K, V2>
    where
        V2: Debug + Clone + PartialEq,
        F: Fn(&V) -> V2,
    {
        let empty = V::empty();

        let data = self
            .data
            .iter()
            .map(|v| if v != &empty { Some(f(v)) } else { None })
            .collect();

        OptionIndexMap {
            data,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<K: IndexMapKey, V: IndexMapData> std::ops::Index<K> for IndexMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index)
    }
}

impl<K: IndexMapKey, V: IndexMapData> std::ops::IndexMut<K> for IndexMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index)
    }
}

#[derive(Debug, Clone)]
pub struct OptionIndexMap<K: IndexMapKey, V: Debug + Clone + PartialEq> {
    data: Vec<Option<V>>,
    _marker: std::marker::PhantomData<K>,
}

impl<K: IndexMapKey, V: Debug + Clone + PartialEq> OptionIndexMap<K, V> {
    pub fn new(max_index: usize) -> Self {
        OptionIndexMap {
            data: vec![None; max_index],
            _marker: std::marker::PhantomData,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn has_key(&self, key: K) -> bool {
        let index = key.index();

        index < self.data.len() && self.data[index] != None
    }

    pub fn get(&self, key: K) -> Option<&V> {
        self.data[key.index()].as_ref()
    }

    pub fn get_mut(&mut self, key: K) -> Option<&mut V> {
        self.data[key.index()].as_mut()
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.data[key.index()] = Some(value);
    }

    pub fn into_iter(self) -> impl Iterator<Item = (K, V)> {
        self.data
            .into_iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|x| (K::new(i), x)))
    }

    pub fn iter<'a>(&'a self) -> impl Iterator<Item = (K, &'a V)> + 'a {
        self.data
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.as_ref().map(|x| (K::new(i), x)))
    }

    pub fn delete(&mut self, key: K) {
        self.data[key.index()] = None;
    }

    pub fn map<F, V2: IndexMapData>(&self, f: F) -> IndexMap<K, V2>
    where
        F: Fn(&V) -> V2,
    {
        let data = self
            .data
            .iter()
            .map(|v| if let Some(v) = v { f(v) } else { V2::empty() })
            .collect();

        IndexMap {
            data,
            _marker: std::marker::PhantomData,
        }
    }

    pub fn map_option<F, V2>(&self, f: F) -> OptionIndexMap<K, V2>
    where
        V2: Debug + Clone + PartialEq,
        F: Fn(&V) -> V2,
    {
        let data = self.data.iter().map(|v| v.as_ref().map(|v| f(v))).collect();

        OptionIndexMap {
            data,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<K: IndexMapKey, V: Debug + Clone + PartialEq> std::ops::Index<K> for OptionIndexMap<K, V> {
    type Output = V;

    fn index(&self, index: K) -> &Self::Output {
        self.get(index).expect("key not present in map")
    }
}

impl<K: IndexMapKey, V: Debug + Clone + PartialEq> std::ops::IndexMut<K> for OptionIndexMap<K, V> {
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        self.get_mut(index).expect("key not present in map")
    }
}

#[derive(Debug, Clone)]
pub struct IndexSet<K: IndexMapKey> {
    data: Vec<bool>,
    _marker: std::marker::PhantomData<K>,
}

impl<K: IndexMapKey> IndexSet<K> {
    pub fn new(max_index: usize) -> Self {
        IndexSet {
            data: vec![false; max_index],
            _marker: std::marker::PhantomData,
        }
    }

    pub fn size(&self) -> usize {
        self.data.len()
    }

    pub fn contains(&self, key: K) -> bool {
        self.data[key.index()]
    }

    /// Insert the key into the set.
    /// Returns true if the key was not already present.
    pub fn insert(&mut self, key: K) -> bool {
        let index = key.index();

        if self.data[index] {
            false
        } else {
            self.data[index] = true;
            true
        }
    }

    pub fn remove(&mut self, key: K) {
        self.data[key.index()] = false;
    }
}

impl<K: IndexMapKey> std::ops::Index<K> for IndexSet<K> {
    type Output = bool;

    fn index(&self, index: K) -> &Self::Output {
        &self.data[index.index()]
    }
}
