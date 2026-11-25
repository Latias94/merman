//! Model-order associations over compact, owner-local slots.

use std::cmp::Ordering;
use std::collections::HashMap;

use super::model_order_relation_update_work_from_counts;
use crate::work::{WorkError, checked_add};

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum Side {
    Down,
    Up,
}
use Side::{Down, Up};

impl Side {
    fn opposite(self) -> Self {
        match self {
            Down => Up,
            Up => Down,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Word {
    index: usize,
    bits: u64,
}

impl Word {
    const WIDTH: usize = u64::BITS as usize;

    fn member(slot: usize) -> Self {
        Self {
            index: slot / Self::WIDTH,
            bits: 1 << (slot % Self::WIDTH),
        }
    }

    fn members(self) -> impl Iterator<Item = usize> {
        let mut bits = self.bits;
        std::iter::from_fn(move || {
            if bits == 0 {
                return None;
            }
            let offset = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            Some(self.index * Self::WIDTH + offset)
        })
    }
}

#[derive(Clone, Debug, Default)]
struct BitmapRow {
    len: usize,
    first: Word,
    // Keep only a pointer to the overflow table in every row. Allocate it on the second word.
    #[allow(clippy::box_collection)]
    additional: Option<Box<HashMap<usize, u64>>>,
}

impl BitmapRow {
    fn contains(&self, slot: usize) -> bool {
        let word = Word::member(slot);
        let bits = if self.first.index == word.index {
            self.first.bits
        } else {
            self.additional
                .as_ref()
                .and_then(|words| words.get(&word.index))
                .copied()
                .unwrap_or(0)
        };
        bits & word.bits != 0
    }

    fn words(&self) -> impl Iterator<Item = Word> + '_ {
        (self.first.bits != 0)
            .then_some(self.first)
            .into_iter()
            .chain(
                self.additional
                    .iter()
                    .flat_map(|words| words.iter().map(|(&index, &bits)| Word { index, bits })),
            )
    }

    fn insert(&mut self, word: Word) -> Word {
        let bits = if self.first.bits == 0 || self.first.index == word.index {
            self.first.index = word.index;
            &mut self.first.bits
        } else {
            self.additional
                .get_or_insert_with(Default::default)
                .entry(word.index)
                .or_default()
        };
        let new_bits = word.bits & !*bits;
        if new_bits != 0 {
            *bits |= new_bits;
            self.len += new_bits.count_ones() as usize;
        }
        Word {
            bits: new_bits,
            ..word
        }
    }

    fn snapshot_with(&self, slot: usize, snapshot: &mut Vec<Word>) -> usize {
        snapshot.clear();
        let mut endpoint = Word::member(slot);
        let mut count = self.len + 1;
        snapshot.extend(self.words().map(|mut word| {
            if word.index == endpoint.index {
                count -= usize::from(word.bits & endpoint.bits != 0);
                word.bits |= endpoint.bits;
                endpoint.bits = 0;
            }
            word
        }));
        if endpoint.bits != 0 {
            snapshot.push(endpoint);
        }
        count
    }
}

#[derive(Debug, Default)]
pub(super) struct ModelOrderRelations {
    // Node references can leave the sorted layer. Register only IDs that acquire facts, so
    // neither arena size nor the largest ID dictates the first allocation or row width.
    slots: HashMap<usize, usize>,
    rows: Vec<[BitmapRow; 2]>,
    smaller_snapshot: Vec<Word>,
    larger_snapshot: Vec<Word>,
    // Differential fixtures can inject facts in just one direction. Live owners start empty
    // and every update adds each pair to both directions, maintaining the transpose invariant.
    #[cfg(test)]
    unpaired_fixture: bool,
}

impl Clone for ModelOrderRelations {
    fn clone(&self) -> Self {
        Self {
            slots: self.slots.clone(),
            rows: self.rows.clone(),
            #[cfg(test)]
            unpaired_fixture: self.unpaired_fixture,
            ..Self::default()
        }
    }
}

impl ModelOrderRelations {
    pub(super) fn clear(&mut self) {
        self.slots.clear();
        self.rows.clear();
        self.smaller_snapshot.clear();
        self.larger_snapshot.clear();
        #[cfg(test)]
        {
            self.unpaired_fixture = false;
        }
    }

    pub(super) fn cached_ordering(&self, first: usize, second: usize) -> Option<Ordering> {
        self.ordering_at(self.lookup(first)?, self.lookup(second)?)
    }

    fn ordering_at(&self, first: usize, second: usize) -> Option<Ordering> {
        if self.rows[first][Down as usize].contains(second) {
            return Some(Ordering::Greater);
        }
        if self.rows[second][Down as usize].contains(first) {
            return Some(Ordering::Less);
        }
        if self.rows[first][Up as usize].contains(second) {
            return Some(Ordering::Less);
        }
        if self.rows[second][Up as usize].contains(first) {
            return Some(Ordering::Greater);
        }
        None
    }

    pub(super) fn barycenter_ordering(&self, first: usize, second: usize) -> Option<Ordering> {
        let (first, second) = (self.lookup(first)?, self.lookup(second)?);
        if self.rows[first][Down as usize].contains(second)
            || self.rows[second][Up as usize].contains(first)
        {
            Some(Ordering::Greater)
        } else if self.rows[second][Down as usize].contains(first)
            || self.rows[first][Up as usize].contains(second)
        {
            Some(Ordering::Less)
        } else {
            None
        }
    }

    // Port admission reads both possible directions before making the comparison. An admitted
    // cache hit reuses this ordering without a second lookup. Misses create no empty rows.
    pub(super) fn comparison_work(
        &self,
        first: usize,
        second: usize,
    ) -> Result<(usize, Option<Ordering>), WorkError> {
        let (first, second) = (self.lookup(first), self.lookup(second));
        if let (Some(first), Some(second)) = (first, second)
            && let Some(order) = self.ordering_at(first, second)
        {
            return Ok((1, Some(order)));
        }
        let forward = self.update_work_at(first, second)?;
        let reverse = self.update_work_at(second, first)?;
        Ok((checked_add(1, forward.max(reverse))?, None))
    }

    pub(super) fn update_work(&self, bigger: usize, smaller: usize) -> Result<usize, WorkError> {
        self.update_work_at(self.lookup(bigger), self.lookup(smaller))
    }

    fn update_work_at(
        &self,
        bigger: Option<usize>,
        smaller: Option<usize>,
    ) -> Result<usize, WorkError> {
        let smaller_count = smaller.map_or(0, |slot| self.rows[slot][Down as usize].len);
        let larger_count = bigger.map_or(0, |slot| self.rows[slot][Up as usize].len);
        model_order_relation_update_work_from_counts(smaller_count, larger_count)
    }

    fn lookup(&self, key: usize) -> Option<usize> {
        self.slots.get(&key).copied()
    }

    fn slot_for(&mut self, key: usize) -> usize {
        *self.slots.entry(key).or_insert_with(|| {
            let slot = self.rows.len();
            self.rows.push(Default::default());
            slot
        })
    }

    pub(super) fn update(&mut self, bigger: usize, smaller: usize) {
        let bigger = self.slot_for(bigger);
        let smaller = self.slot_for(smaller);
        let downs = &self.rows[smaller][Down as usize];
        let ups = &self.rows[bigger][Up as usize];
        if downs.len == 0 && ups.len == 0 {
            self.smaller_snapshot.clear();
            self.larger_snapshot.clear();
            let delta = self.rows[bigger][Down as usize].insert(Word::member(smaller));
            #[cfg(test)]
            let unpaired = self.unpaired_fixture;
            #[cfg(not(test))]
            let unpaired = false;
            if delta.bits != 0 || unpaired {
                self.rows[smaller][Up as usize].insert(Word::member(bigger));
            }
            return;
        }

        // One update adds just the rectangle between the two entry snapshots plus the
        // endpoints. Snapshot both sides before touching either plane, even for a self pair
        // or feedback reversal. Reaching a newly added bit never expands this rectangle.
        let smaller_count = downs.snapshot_with(smaller, &mut self.smaller_snapshot);
        let larger_count = ups.snapshot_with(bigger, &mut self.larger_snapshot);

        #[cfg(test)]
        if self.unpaired_fixture {
            Self::merge::<false>(
                &mut self.rows,
                &self.larger_snapshot,
                &self.smaller_snapshot,
                Down,
            );
            Self::merge::<false>(
                &mut self.rows,
                &self.smaller_snapshot,
                &self.larger_snapshot,
                Up,
            );
            return;
        }

        // With paired facts, a bit already set in the chosen plane is already set in its
        // transpose. Scan the cheaper orientation and mirror only newly set bits.
        if larger_count.saturating_mul(self.smaller_snapshot.len())
            <= smaller_count.saturating_mul(self.larger_snapshot.len())
        {
            Self::merge::<true>(
                &mut self.rows,
                &self.larger_snapshot,
                &self.smaller_snapshot,
                Down,
            );
        } else {
            Self::merge::<true>(
                &mut self.rows,
                &self.smaller_snapshot,
                &self.larger_snapshot,
                Up,
            );
        }
    }

    fn merge<const MIRROR: bool>(
        rows: &mut [[BitmapRow; 2]],
        targets: &[Word],
        source: &[Word],
        side: Side,
    ) {
        let reverse = side.opposite() as usize;
        for target in targets.iter().flat_map(|word| word.members()) {
            let reverse_word = Word::member(target);
            for &word in source {
                let delta = rows[target][side as usize].insert(word);
                if MIRROR {
                    for member in delta.members() {
                        rows[member][reverse].insert(reverse_word);
                    }
                }
            }
        }
    }
}
