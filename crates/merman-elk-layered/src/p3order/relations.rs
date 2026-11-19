//! Sparse model-order associations with owner-local propagation scratch.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

#[cfg(test)]
mod tests;

#[derive(Debug, Default)]
pub(super) struct ModelOrderRelations {
    pub(super) bigger_than: HashMap<usize, HashSet<usize>>,
    pub(super) smaller_than: HashMap<usize, HashSet<usize>>,
    smaller_snapshot: Vec<usize>,
    larger_snapshot: Vec<usize>,
}

impl Clone for ModelOrderRelations {
    fn clone(&self) -> Self {
        Self {
            bigger_than: self.bigger_than.clone(),
            smaller_than: self.smaller_than.clone(),
            ..Self::default()
        }
    }
}

impl ModelOrderRelations {
    pub(super) fn clear(&mut self) {
        self.bigger_than.clear();
        self.smaller_than.clear();
        self.smaller_snapshot.clear();
        self.larger_snapshot.clear();
    }

    pub(super) fn cached_ordering(&mut self, first: usize, second: usize) -> Option<Ordering> {
        self.ensure_key(first);
        self.ensure_key(second);
        if self.bigger_than[&first].contains(&second) {
            return Some(Ordering::Greater);
        }
        if self.bigger_than[&second].contains(&first) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&first].contains(&second) {
            return Some(Ordering::Less);
        }
        if self.smaller_than[&second].contains(&first) {
            return Some(Ordering::Greater);
        }
        None
    }

    fn ensure_key(&mut self, key: usize) {
        self.bigger_than.entry(key).or_default();
        self.smaller_than.entry(key).or_default();
    }

    pub(super) fn update(&mut self, bigger: usize, smaller: usize) {
        let Self {
            bigger_than,
            smaller_than,
            smaller_snapshot,
            larger_snapshot,
        } = self;
        bigger_than.entry(bigger).or_default();
        bigger_than.entry(smaller).or_default();
        smaller_than.entry(bigger).or_default();
        smaller_than.entry(smaller).or_default();

        smaller_snapshot.clear();
        smaller_snapshot.extend(bigger_than[&smaller].iter().copied());
        larger_snapshot.clear();
        larger_snapshot.extend(smaller_than[&bigger].iter().copied());

        bigger_than.entry(bigger).or_default().insert(smaller);
        smaller_than.entry(smaller).or_default().insert(bigger);

        for &very_small in smaller_snapshot.iter() {
            bigger_than.entry(bigger).or_default().insert(very_small);
            let row = smaller_than.entry(very_small).or_default();
            row.insert(bigger);
            row.extend(larger_snapshot.iter().copied());
        }

        for &very_big in larger_snapshot.iter() {
            smaller_than.entry(smaller).or_default().insert(very_big);
            let row = bigger_than.entry(very_big).or_default();
            row.insert(smaller);
            row.extend(smaller_snapshot.iter().copied());
        }
    }
}
