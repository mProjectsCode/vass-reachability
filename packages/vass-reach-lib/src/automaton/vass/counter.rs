use std::ops::{Index, IndexMut};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct VASSCounterIndex {
    index: u32,
}

impl VASSCounterIndex {
    pub fn new(index: u32) -> Self {
        VASSCounterIndex { index }
    }

    pub fn iter_counters(dimension: usize) -> impl Iterator<Item = VASSCounterIndex> {
        (0..dimension).map(|i| VASSCounterIndex::new(i as u32))
    }

    pub fn to_usize(&self) -> usize {
        self.index as usize
    }
}

impl std::fmt::Display for VASSCounterIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "c{}", self.index)
    }
}

impl From<u32> for VASSCounterIndex {
    fn from(index: u32) -> Self {
        VASSCounterIndex::new(index)
    }
}

impl From<VASSCounterIndex> for u32 {
    fn from(index: VASSCounterIndex) -> Self {
        index.index
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VASSCounterValuation {
    values: Box<[i32]>,
}

impl VASSCounterValuation {
    pub fn new(values: Box<[i32]>) -> Self {
        VASSCounterValuation { values }
    }

    pub fn zero(dimension: usize) -> Self {
        VASSCounterValuation {
            values: vec![0; dimension].into_boxed_slice(),
        }
    }

    pub fn dimension(&self) -> usize {
        self.values.len()
    }

    pub fn can_apply_update(&self, update: &VASSCounterUpdate) -> bool {
        debug_assert_eq!(
            self.dimension(),
            update.dimension(),
            "Valuation and update must have the same dimension"
        );
        for i in 0..self.dimension() {
            if self.values[i] + update.values[i] < 0 {
                return false;
            }
        }
        true
    }

    pub fn apply_update(&mut self, update: &VASSCounterUpdate) {
        debug_assert_eq!(
            self.dimension(),
            update.dimension(),
            "Valuation and update must have the same dimension"
        );
        for i in 0..self.dimension() {
            self.values[i] += update.values[i];
        }
    }

    pub fn apply_update_rev(&mut self, update: &VASSCounterUpdate) {
        debug_assert_eq!(
            self.dimension(),
            update.dimension(),
            "Valuation and update must have the same dimension"
        );
        for i in 0..self.dimension() {
            self.values[i] -= update.values[i];
        }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, i32> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, i32> {
        self.values.iter_mut()
    }

    pub fn to_update(self) -> VASSCounterUpdate {
        VASSCounterUpdate::new(self.values)
    }

    pub fn mod_euclid(&self, modulus: i32) -> Self {
        Self {
            values: self.values.iter().map(|x| x.rem_euclid(modulus)).collect(),
        }
    }

    pub fn mod_euclid_mut(&mut self, modulus: i32) {
        for x in self.values.iter_mut() {
            *x = x.rem_euclid(modulus);
        }
    }

    pub fn mod_euclid_slice(&self, modulus: &[i32]) -> Self {
        assert_eq!(self.dimension(), modulus.len());
        Self {
            values: self
                .values
                .iter()
                .zip(modulus.iter())
                .map(|(x, m)| x.rem_euclid(*m))
                .collect(),
        }
    }

    pub fn mod_euclid_slice_mut(&mut self, modulus: &[i32]) {
        assert_eq!(self.dimension(), modulus.len());
        for (x, m) in self.values.iter_mut().zip(modulus.iter()) {
            *x = x.rem_euclid(*m);
        }
    }

    pub fn find_negative_counter(&self) -> Option<VASSCounterIndex> {
        self.values
            .iter()
            .position(|&x| x < 0)
            .map(|i| VASSCounterIndex::new(i as u32))
    }
}

impl From<Box<[i32]>> for VASSCounterValuation {
    fn from(values: Box<[i32]>) -> Self {
        VASSCounterValuation::new(values)
    }
}

impl From<&[i32]> for VASSCounterValuation {
    fn from(values: &[i32]) -> Self {
        VASSCounterValuation::new(values.to_vec().into_boxed_slice())
    }
}

impl From<Vec<i32>> for VASSCounterValuation {
    fn from(values: Vec<i32>) -> Self {
        VASSCounterValuation::new(values.into_boxed_slice())
    }
}

impl From<VASSCounterValuation> for Box<[i32]> {
    fn from(valuation: VASSCounterValuation) -> Self {
        valuation.values
    }
}

impl FromIterator<i32> for VASSCounterValuation {
    fn from_iter<T: IntoIterator<Item = i32>>(iter: T) -> Self {
        let values: Vec<i32> = iter.into_iter().collect();
        VASSCounterValuation::new(values.into_boxed_slice())
    }
}

impl Index<VASSCounterIndex> for VASSCounterValuation {
    type Output = i32;

    fn index(&self, index: VASSCounterIndex) -> &Self::Output {
        &self.values[index.index as usize]
    }
}

impl IndexMut<VASSCounterIndex> for VASSCounterValuation {
    fn index_mut(&mut self, index: VASSCounterIndex) -> &mut Self::Output {
        &mut self.values[index.index as usize]
    }
}

impl Index<usize> for VASSCounterValuation {
    type Output = i32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl IndexMut<usize> for VASSCounterValuation {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.values[index]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VASSCounterUpdate {
    values: Box<[i32]>,
}

impl VASSCounterUpdate {
    pub fn new(values: Box<[i32]>) -> Self {
        VASSCounterUpdate { values }
    }

    pub fn dimension(&self) -> usize {
        self.values.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, i32> {
        self.values.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<'_, i32> {
        self.values.iter_mut()
    }

    pub fn to_valuation(self) -> VASSCounterValuation {
        VASSCounterValuation::new(self.values)
    }
}

impl From<Box<[i32]>> for VASSCounterUpdate {
    fn from(values: Box<[i32]>) -> Self {
        VASSCounterUpdate::new(values)
    }
}

impl From<&[i32]> for VASSCounterUpdate {
    fn from(values: &[i32]) -> Self {
        VASSCounterUpdate::new(values.to_vec().into_boxed_slice())
    }
}

impl From<Vec<i32>> for VASSCounterUpdate {
    fn from(values: Vec<i32>) -> Self {
        VASSCounterUpdate::new(values.into_boxed_slice())
    }
}

impl From<VASSCounterUpdate> for Box<[i32]> {
    fn from(valuation: VASSCounterUpdate) -> Self {
        valuation.values
    }
}

impl FromIterator<i32> for VASSCounterUpdate {
    fn from_iter<T: IntoIterator<Item = i32>>(iter: T) -> Self {
        let values: Vec<i32> = iter.into_iter().collect();
        VASSCounterUpdate::new(values.into_boxed_slice())
    }
}

impl Index<VASSCounterIndex> for VASSCounterUpdate {
    type Output = i32;

    fn index(&self, index: VASSCounterIndex) -> &Self::Output {
        &self.values[index.index as usize]
    }
}

impl IndexMut<VASSCounterIndex> for VASSCounterUpdate {
    fn index_mut(&mut self, index: VASSCounterIndex) -> &mut Self::Output {
        &mut self.values[index.index as usize]
    }
}

impl Index<usize> for VASSCounterUpdate {
    type Output = i32;

    fn index(&self, index: usize) -> &Self::Output {
        &self.values[index]
    }
}

impl IndexMut<usize> for VASSCounterUpdate {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.values[index]
    }
}

impl IntoIterator for VASSCounterUpdate {
    type Item = i32;
    type IntoIter = std::vec::IntoIter<i32>;

    fn into_iter(self) -> Self::IntoIter {
        self.values.into_iter()
    }
}
