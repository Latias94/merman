//! Model-order associations over compact, owner-local slots.

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::work::{WorkControl, WorkError, checked_add, checked_mul, checked_sum};

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
        self.bits_at(word.index) & word.bits != 0
    }

    fn bits_at(&self, index: usize) -> u64 {
        if self.first.index == index {
            self.first.bits
        } else {
            self.additional
                .as_ref()
                .and_then(|words| words.get(&index))
                .copied()
                .unwrap_or(0)
        }
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
}

// A read-only view of an entry snapshot. A proposed slot can be outside the registered rows.
// Pricing can scan the view without allocating or touching the owner's reusable snapshots.
#[derive(Clone, Copy)]
struct EntryRow<'a> {
    row: &'a BitmapRow,
    endpoint: Word,
    members: usize,
    words: usize,
    append_endpoint: bool,
}

impl<'a> EntryRow<'a> {
    fn new(row: &'a BitmapRow, slot: usize) -> Self {
        let endpoint = Word::member(slot);
        let bits = row.bits_at(endpoint.index);
        let append_endpoint = bits == 0;
        Self {
            row,
            endpoint,
            members: row.len + usize::from(bits & endpoint.bits == 0),
            words: usize::from(row.first.bits != 0)
                + row.additional.as_ref().map_or(0, |words| words.len())
                + usize::from(append_endpoint),
            append_endpoint,
        }
    }

    fn masks(self) -> impl Iterator<Item = Word> + 'a {
        self.row
            .words()
            .map(move |mut word| {
                if word.index == self.endpoint.index {
                    word.bits |= self.endpoint.bits;
                }
                word
            })
            .chain(self.append_endpoint.then_some(self.endpoint))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct UpdateWork {
    pricing: usize,
    apply: usize,
}

impl UpdateWork {
    fn total(&self) -> Result<usize, WorkError> {
        checked_add(self.pricing, self.apply)
    }
}

fn row_union_work(targets: usize, source_words: usize) -> Result<usize, WorkError> {
    // One row selection and one visit for every source word, even on a zero delta.
    checked_add(targets, checked_mul(targets, source_words)?)
}

fn pricing_scan_work(targets: usize, source_words: usize) -> Result<usize, WorkError> {
    let visits = checked_mul(targets, source_words)?;
    checked_sum([targets, visits, visits / Word::WIDTH])
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
        work: &mut dyn WorkControl,
    ) -> Result<(usize, Option<Ordering>), WorkError> {
        let (first_slot, second_slot) = (self.lookup(first), self.lookup(second));
        if let (Some(first), Some(second)) = (first_slot, second_slot)
            && let Some(order) = self.ordering_at(first, second)
        {
            return Ok((1, Some(order)));
        }
        let same = first == second;
        let forward = self.price_update(first_slot, second_slot, same, 1, work)?;
        if same {
            return Ok((checked_add(1, forward.total()?)?, None));
        }
        // The port comparator has not touched the graph yet. Pay for both pricing scans
        // and retain the larger application reservation until it chooses a direction.
        let prefix = checked_add(1, forward.pricing)?;
        let reverse = self.price_update(second_slot, first_slot, false, prefix, work)?;
        Ok((
            checked_sum([prefix, reverse.pricing, forward.apply.max(reverse.apply)])?,
            None,
        ))
    }

    pub(super) fn update_work(
        &self,
        bigger: usize,
        smaller: usize,
        work: &mut dyn WorkControl,
    ) -> Result<usize, WorkError> {
        self.price_update(
            self.lookup(bigger),
            self.lookup(smaller),
            bigger == smaller,
            0,
            work,
        )?
        .total()
    }

    fn price_update(
        &self,
        bigger: Option<usize>,
        smaller: Option<usize>,
        same: bool,
        prefix: usize,
        work: &mut dyn WorkControl,
    ) -> Result<UpdateWork, WorkError> {
        // Project the same first-seen slots that update() will register after admission.
        let next = self.rows.len();
        let bigger = bigger.unwrap_or(next);
        let smaller = if same {
            bigger
        } else if let Some(smaller) = smaller {
            smaller
        } else {
            checked_add(next, usize::from(bigger == next))?
        };
        let empty = BitmapRow::default();
        let smaller_row = self.row(smaller, Down).unwrap_or(&empty);
        let larger_row = self.row(bigger, Up).unwrap_or(&empty);
        if smaller_row.len == 0 && larger_row.len == 0 {
            // A constant-size direct update has one primary and at most one reverse write.
            return Ok(UpdateWork {
                pricing: 0,
                apply: 2,
            });
        }

        let downs = EntryRow::new(smaller_row, smaller);
        let ups = EntryRow::new(larger_row, bigger);
        let copies = checked_add(downs.words, ups.words)?;
        #[cfg(test)]
        if self.unpaired_fixture {
            return Ok(UpdateWork {
                pricing: 0,
                apply: checked_sum([
                    copies,
                    row_union_work(ups.members, downs.words)?,
                    row_union_work(downs.members, ups.words)?,
                ])?,
            });
        }
        let (targets, source, side) = Self::propagation_rows(downs, ups);
        let apply_scan = row_union_work(targets.members, source.words)?;
        let pricing = pricing_scan_work(targets.members, source.words)?;
        // Both scans and snapshot copies must fit before examining the rectangle. The extra
        // scan intersects words and popcounts; only the admitted application walks delta bits.
        let apply = checked_add(copies, apply_scan)?;
        work.check(checked_sum([prefix, pricing, apply])?)?;

        let mut reflected = 0usize;
        let mut words_until_poll = Word::WIDTH;
        for target in targets.masks().flat_map(Word::members) {
            let row = self.row(target, side).unwrap_or(&empty);
            for word in source.masks() {
                reflected = checked_add(
                    reflected,
                    (word.bits & !row.bits_at(word.index)).count_ones() as usize,
                )?;
                words_until_poll -= 1;
                if words_until_poll == 0 {
                    // Hash iteration may permute the progress within the rectangle. These zero
                    // unit polls observe cancellation without making the ceiling order-dependent.
                    work.check(0)?;
                    words_until_poll = Word::WIDTH;
                }
            }
        }
        Ok(UpdateWork {
            pricing,
            apply: checked_add(apply, reflected)?,
        })
    }

    fn row(&self, slot: usize, side: Side) -> Option<&BitmapRow> {
        self.rows.get(slot).map(|row| &row[side as usize])
    }

    fn propagation_rows<'a>(
        downs: EntryRow<'a>,
        ups: EntryRow<'a>,
    ) -> (EntryRow<'a>, EntryRow<'a>, Side) {
        if ups.members.saturating_mul(downs.words) <= downs.members.saturating_mul(ups.words) {
            (ups, downs, Down)
        } else {
            (downs, ups, Up)
        }
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
        let downs = EntryRow::new(downs, smaller);
        let ups = EntryRow::new(ups, bigger);
        self.smaller_snapshot.clear();
        self.smaller_snapshot.extend(downs.masks());
        self.larger_snapshot.clear();
        self.larger_snapshot.extend(ups.masks());
        let (_, _, side) = Self::propagation_rows(downs, ups);

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
        if matches!(side, Down) {
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
