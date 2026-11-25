use super::*;
use std::collections::HashSet;

use crate::graph::{LGraph, LNode, LNodeKind, LPoint, LayeredEdge, PortSide, PortType};
use crate::options::{LayeredOptions, OrderingStrategy};
use crate::p3order::{ModelOrderNodeComparator, ModelOrderPortComparator};
use crate::work::{NoopWorkControl, WorkControl, WorkError};

type Facts = HashMap<usize, HashSet<usize>>;

impl ModelOrderRelations {
    pub(in crate::p3order) fn from_test_rows(bigger_than: &Facts, smaller_than: &Facts) -> Self {
        let mut relations = Self::default();
        for (side, rows) in [(Down, bigger_than), (Up, smaller_than)] {
            for (&key, members) in rows {
                for &member in members {
                    let slot = relations.slot_for(key);
                    let member = relations.slot_for(member);
                    relations.rows[slot][side as usize].insert(Word::member(member));
                }
            }
        }
        relations.unpaired_fixture = relations.rows.iter().enumerate().any(|(slot, pair)| {
            [Down, Up].into_iter().any(|side| {
                pair[side as usize]
                    .words()
                    .flat_map(Word::members)
                    .any(|member| !relations.rows[member][side.opposite() as usize].contains(slot))
            })
        });
        relations
    }

    pub(in crate::p3order) fn is_clear(&self) -> bool {
        self.slots.is_empty() && self.rows.is_empty()
    }

    pub(in crate::p3order) fn bigger_members(&self, key: usize) -> HashSet<usize> {
        self.members(key, Down)
    }

    pub(in crate::p3order) fn smaller_members(&self, key: usize) -> HashSet<usize> {
        self.members(key, Up)
    }

    fn members(&self, key: usize, side: Side) -> HashSet<usize> {
        let Some(slot) = self.lookup(key) else {
            return HashSet::new();
        };
        self.slots
            .iter()
            .filter_map(|(&member, &index)| {
                self.rows[slot][side as usize]
                    .contains(index)
                    .then_some(member)
            })
            .collect()
    }

    pub(in crate::p3order) fn facts(&self) -> (Facts, Facts) {
        let mut ids = vec![0; self.rows.len()];
        for (&id, &slot) in &self.slots {
            ids[slot] = id;
        }
        let collect = |side: Side| {
            self.rows
                .iter()
                .enumerate()
                .filter_map(|(slot, pair)| {
                    let row = &pair[side as usize];
                    (row.len != 0).then(|| {
                        let members = row
                            .words()
                            .flat_map(Word::members)
                            .map(|i| ids[i])
                            .collect();
                        (ids[slot], members)
                    })
                })
                .collect::<Facts>()
        };
        (collect(Down), collect(Up))
    }

    fn scratch_buffers(&self) -> [(*const Word, usize); 2] {
        [
            (
                self.smaller_snapshot.as_ptr(),
                self.smaller_snapshot.capacity(),
            ),
            (
                self.larger_snapshot.as_ptr(),
                self.larger_snapshot.capacity(),
            ),
        ]
    }

    fn snapshot_members(&self, snapshot: &[Word]) -> HashSet<usize> {
        let mut ids = vec![0; self.rows.len()];
        for (&id, &slot) in &self.slots {
            ids[slot] = id;
        }
        snapshot
            .iter()
            .copied()
            .flat_map(Word::members)
            .map(|i| ids[i])
            .collect()
    }
}

#[derive(Clone, Default)]
struct LegacyRelations {
    bigger_than: Facts,
    smaller_than: Facts,
}

impl LegacyRelations {
    fn update(&mut self, bigger: usize, smaller: usize) {
        let bigger_than = &mut self.bigger_than;
        let smaller_than = &mut self.smaller_than;
        bigger_than.entry(bigger).or_default();
        bigger_than.entry(smaller).or_default();
        smaller_than.entry(bigger).or_default();
        smaller_than.entry(smaller).or_default();

        let smaller_bigger_than = bigger_than[&smaller].clone();
        let bigger_smaller_than = smaller_than[&bigger].clone();

        bigger_than.get_mut(&bigger).unwrap().insert(smaller);
        smaller_than.get_mut(&smaller).unwrap().insert(bigger);

        for very_small in smaller_bigger_than {
            bigger_than.get_mut(&bigger).unwrap().insert(very_small);
            smaller_than.entry(very_small).or_default().insert(bigger);
            let bigger_smaller_than = smaller_than[&bigger].clone();
            smaller_than
                .get_mut(&very_small)
                .unwrap()
                .extend(bigger_smaller_than);
        }

        for very_big in bigger_smaller_than {
            smaller_than.get_mut(&smaller).unwrap().insert(very_big);
            bigger_than.entry(very_big).or_default().insert(smaller);
            let smaller_bigger_than = bigger_than[&smaller].clone();
            bigger_than
                .get_mut(&very_big)
                .unwrap()
                .extend(smaller_bigger_than);
        }
    }

    fn cached_ordering(&mut self, first: usize, second: usize) -> Option<Ordering> {
        for key in [first, second] {
            self.bigger_than.entry(key).or_default();
            self.smaller_than.entry(key).or_default();
        }
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

    fn candidate(&self) -> ModelOrderRelations {
        ModelOrderRelations::from_test_rows(&self.bigger_than, &self.smaller_than)
    }
}

fn assert_state(candidate: &ModelOrderRelations, legacy: &LegacyRelations) {
    // Legacy cache probes materialize empty rows. They have zero cardinality and no facts.
    let nonempty = |rows: &Facts| {
        rows.iter()
            .filter(|(_, members)| !members.is_empty())
            .map(|(&key, members)| (key, members.clone()))
            .collect::<Facts>()
    };
    assert_eq!(
        candidate.facts(),
        (
            nonempty(&legacy.bigger_than),
            nonempty(&legacy.smaller_than)
        )
    );
}

fn assert_observations(candidate: &ModelOrderRelations, legacy: &LegacyRelations, keys: &[usize]) {
    assert_state(candidate, legacy);
    let mut reference = legacy.clone();
    for &first in keys {
        for &second in keys {
            let order = reference.cached_ordering(first, second);
            let (priced, priced_order) = candidate
                .comparison_work(first, second, &mut NoopWorkControl)
                .unwrap();
            assert_eq!(priced == 1, order.is_some());
            assert_eq!(priced_order, order);
            assert_eq!(candidate.cached_ordering(first, second), order);
        }
    }
}

fn update_both(
    candidate: &mut ModelOrderRelations,
    legacy: &mut LegacyRelations,
    bigger: usize,
    smaller: usize,
) {
    let mut downs = legacy
        .bigger_than
        .get(&smaller)
        .cloned()
        .unwrap_or_default();
    let mut ups = legacy
        .smaller_than
        .get(&bigger)
        .cloned()
        .unwrap_or_default();
    candidate.update(bigger, smaller);
    legacy.update(bigger, smaller);
    assert_state(candidate, legacy);
    if !downs.is_empty() || !ups.is_empty() {
        downs.insert(smaller);
        ups.insert(bigger);
    }
    assert_eq!(
        candidate.snapshot_members(&candidate.smaller_snapshot),
        downs
    );
    assert_eq!(candidate.snapshot_members(&candidate.larger_snapshot), ups);
}

#[test]
fn reached_multiword_rows_match_the_entry_rectangles_through_cycles() {
    let ids = (0..130).map(|i| usize::MAX - i * 1009).collect::<Vec<_>>();
    let mut legacy = LegacyRelations::default();
    let mut relations = ModelOrderRelations::default();
    for index in (0..64).chain(65..129) {
        update_both(&mut relations, &mut legacy, ids[index], ids[index + 1]);
    }
    for (a, b) in [(64, 65), (64, 65), (129, 0), (63, 63), (65, 64)] {
        assert!(!relations.unpaired_fixture);
        update_both(&mut relations, &mut legacy, ids[a], ids[b]);
    }
    assert_eq!(relations.bigger_members(ids[63]).len(), ids.len());
    assert_eq!(relations.smaller_members(ids[64]).len(), ids.len());
    assert_eq!(
        relations.cached_ordering(ids[63], ids[65]),
        Some(Ordering::Greater)
    );
    assert_eq!(
        relations.cached_ordering(ids[65], ids[63]),
        Some(Ordering::Greater)
    );
}

#[test]
fn false_transpose_and_full_closure_assumptions_change_seeded_facts() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([
            (1, HashSet::from([2])),
            (2, HashSet::from([3])),
            (4, HashSet::from([5])),
        ]),
        smaller_than: HashMap::new(),
    };
    let mut relations = legacy.candidate();
    update_both(&mut relations, &mut legacy, 4, 5);
    assert_eq!(relations.smaller_members(5), HashSet::from([4]));
    assert!(!relations.bigger_members(1).contains(&3));
}

#[test]
fn high_external_ids_and_cache_misses_allocate_only_observed_slots() {
    let mut relations = ModelOrderRelations::default();
    for first in 0..500 {
        assert_eq!(relations.cached_ordering(first, usize::MAX), None);
        assert_eq!(
            relations.comparison_work(first, usize::MAX, &mut NoopWorkControl),
            Ok((3, None))
        );
    }
    assert_eq!(
        (relations.slots.capacity(), relations.rows.capacity()),
        (0, 0)
    );

    let ids = (0..500).map(|i| (i * 997, usize::MAX - i * 1009));
    for (first, second) in ids {
        relations.update(first, second);
    }
    assert_eq!(relations.rows.len(), 1_000);
    assert!(
        relations
            .rows
            .iter()
            .flatten()
            .all(|row| row.additional.is_none())
    );
    assert_eq!(
        (
            relations.smaller_snapshot.capacity(),
            relations.larger_snapshot.capacity()
        ),
        (0, 0)
    );
    let capacities = (relations.slots.capacity(), relations.rows.capacity());
    relations.clear();
    assert!(relations.is_clear());
    relations.update(1, usize::MAX);
    assert_eq!(relations.cached_ordering(0, usize::MAX), None);
    assert_eq!(
        relations.cached_ordering(1, usize::MAX),
        Some(Ordering::Greater)
    );
    assert_eq!(
        (relations.slots.capacity(), relations.rows.capacity()),
        capacities
    );
}

#[test]
fn all_two_key_states_match_legacy_through_two_updates() {
    let keys = [7, usize::MAX];
    let pairs = [
        (keys[0], keys[0]),
        (keys[0], keys[1]),
        (keys[1], keys[0]),
        (keys[1], keys[1]),
    ];
    for state in 0..5usize.pow(4) {
        let mut seed = LegacyRelations::default();
        let mut digits = state;
        for rows in [&mut seed.bigger_than, &mut seed.smaller_than] {
            for &key in &keys {
                let row = digits % 5;
                digits /= 5;
                if row != 0 {
                    let members = keys
                        .iter()
                        .enumerate()
                        .filter_map(|(bit, &member)| {
                            ((row - 1) & (1 << bit) != 0).then_some(member)
                        })
                        .collect();
                    rows.insert(key, members);
                }
            }
        }
        assert_observations(&seed.candidate(), &seed, &keys);
        for &first in &pairs {
            for &second in &pairs {
                let mut candidate = seed.candidate();
                let mut legacy = seed.clone();
                for (bigger, smaller) in [first, second] {
                    update_both(&mut candidate, &mut legacy, bigger, smaller);
                    assert_observations(&candidate, &legacy, &keys);
                }
            }
        }
    }
}

#[test]
fn cached_ordering_retains_all_four_priorities() {
    use Ordering::{Greater, Less};
    let expected = [
        None,
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Greater),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
        Some(Less),
        Some(Greater),
    ];
    for (mask, expected) in expected.into_iter().enumerate() {
        let mut legacy = LegacyRelations::default();
        for (bit, key, member) in [(0, 11, 23), (1, 23, 11)] {
            if mask & (1 << bit) != 0 {
                legacy.bigger_than.entry(key).or_default().insert(member);
            }
        }
        for (bit, key, member) in [(2, 11, 23), (3, 23, 11)] {
            if mask & (1 << bit) != 0 {
                legacy.smaller_than.entry(key).or_default().insert(member);
            }
        }
        let candidate = legacy.candidate();
        assert_eq!(candidate.cached_ordering(11, 23), expected, "mask {mask}");
        assert_eq!(legacy.cached_ordering(11, 23), expected, "mask {mask}");
        assert_state(&candidate, &legacy);
    }
}

#[test]
fn mixed_alias_updates_and_intermediate_queries_match_legacy() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([
            (9, HashSet::from([9, 11, usize::MAX])),
            (11, HashSet::from([27])),
            (5, HashSet::new()),
        ]),
        smaller_than: HashMap::from([
            (11, HashSet::from([9, 11, 27])),
            (9, HashSet::from([usize::MAX])),
            (99, HashSet::new()),
        ]),
    };
    let keys = [5, 9, 11, 27, 99, usize::MAX];
    let mut candidate = legacy.candidate();
    for (bigger, smaller) in [
        (11, 9),
        (9, 11),
        (11, 11),
        (9, 9),
        (27, usize::MAX),
        (11, 9),
        (usize::MAX, 11),
        (99, 5),
        (5, 99),
    ] {
        assert_observations(&candidate, &legacy, &keys);
        update_both(&mut candidate, &mut legacy, bigger, smaller);
        for (first, second) in [(smaller, bigger), (bigger, bigger), (5, 99)] {
            assert_eq!(
                candidate.cached_ordering(first, second),
                legacy.cached_ordering(first, second),
            );
            assert_state(&candidate, &legacy);
        }
    }
    assert_observations(&candidate, &legacy, &keys);
}

#[test]
fn entry_snapshots_reuse_capacity_without_leaking_previous_members() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, (0..8).collect())]),
        smaller_than: HashMap::from([(40, (8..16).collect())]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    let buffers = candidate.scratch_buffers();
    update_both(&mut candidate, &mut legacy, 1_000, 2_000);
    assert_eq!(candidate.bigger_members(1_000), HashSet::from([2_000]));
    assert_eq!(candidate.smaller_members(2_000), HashSet::from([1_000]));
    update_both(&mut candidate, &mut legacy, 4_000, 50);
    assert_eq!(candidate.scratch_buffers(), buffers);
    assert_observations(&candidate, &legacy, &[0, 8, 40, 50, 1_000, 2_000, 4_000]);
}

#[test]
fn clear_drops_facts_and_reuses_scratch_without_carrying_members_forward() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, (0..8).collect())]),
        smaller_than: HashMap::from([(40, (8..16).collect())]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    let buffers = candidate.scratch_buffers();
    candidate.clear();
    let mut legacy = LegacyRelations::default();
    assert_state(&candidate, &legacy);
    assert!(candidate.smaller_snapshot.is_empty());
    assert!(candidate.larger_snapshot.is_empty());
    update_both(&mut candidate, &mut legacy, 1_000, 2_000);
    assert_eq!(candidate.scratch_buffers(), buffers);
    assert_observations(&candidate, &legacy, &[40, 50, 1_000, 2_000]);
}

#[test]
fn clone_copies_facts_without_copying_propagation_scratch() {
    let mut legacy = LegacyRelations {
        bigger_than: HashMap::from([(50, HashSet::from([40, 50, 60]))]),
        smaller_than: HashMap::from([(40, HashSet::from([40, 50, 70]))]),
    };
    let mut candidate = legacy.candidate();
    update_both(&mut candidate, &mut legacy, 40, 50);
    assert!(!candidate.smaller_snapshot.is_empty());
    assert!(!candidate.larger_snapshot.is_empty());
    let mut cloned = candidate.clone();
    assert_observations(&cloned, &legacy, &[40, 50, 60, 70]);
    assert_eq!(cloned.smaller_snapshot.capacity(), 0);
    assert_eq!(cloned.larger_snapshot.capacity(), 0);
    let mut cloned_legacy = legacy.clone();
    update_both(&mut cloned, &mut cloned_legacy, 70, 60);
    assert_observations(&cloned, &cloned_legacy, &[40, 50, 60, 70]);
    assert_state(&candidate, &legacy);
}

#[derive(Debug, PartialEq, Eq)]
enum WorkEvent {
    Check(usize),
    Charge(usize),
}

struct TraceWork {
    remaining: usize,
    events: Vec<WorkEvent>,
    reject_charge: bool,
    cancel_on_poll: bool,
}

impl TraceWork {
    fn new(remaining: usize) -> Self {
        Self {
            remaining,
            events: Vec::new(),
            reject_charge: false,
            cancel_on_poll: false,
        }
    }
}

impl WorkControl for TraceWork {
    fn check(&mut self, units: usize) -> Result<(), WorkError> {
        self.events.push(WorkEvent::Check(units));
        if units > self.remaining || (self.cancel_on_poll && units == 0) {
            Err(WorkError::Interrupted)
        } else {
            Ok(())
        }
    }

    fn charge(&mut self, units: usize) -> Result<(), WorkError> {
        self.events.push(WorkEvent::Charge(units));
        if self.reject_charge {
            return Err(WorkError::Interrupted);
        }
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(WorkError::Interrupted)?;
        Ok(())
    }
}

#[test]
fn rectangle_pricing_checks_before_scanning_and_polls_without_publishing() {
    use WorkEvent::{Charge, Check};
    let graph = LGraph::new("multiword-pricing", LayeredOptions::default());
    let mut comparator =
        ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::NodesAndEdges);
    for index in (0..64).chain(65..129) {
        comparator.relations.update(index, index + 1);
    }
    let before = comparator.relations.facts();
    let buffers = comparator.relations.scratch_buffers();

    // Two words in each snapshot and 65 target rows. Each scan selects 65 rows and
    // visits 130 words. Pricing polls twice and the bridge reflects 65 * 65 new bits.
    const FLOOR: usize = 4 + 2 * (65 + 130) + 2;
    const PRICE: usize = FLOOR + 65 * 65;
    for (budget, cancel, reject_charge, events) in [
        (FLOOR - 1, false, false, vec![Check(FLOOR)]),
        (PRICE, true, false, vec![Check(FLOOR), Check(0)]),
        (
            PRICE - 1,
            false,
            false,
            vec![Check(FLOOR), Check(0), Check(0), Check(PRICE)],
        ),
        (
            PRICE,
            false,
            true,
            vec![
                Check(FLOOR),
                Check(0),
                Check(0),
                Check(PRICE),
                Charge(PRICE),
            ],
        ),
    ] {
        let mut work = TraceWork::new(budget);
        work.cancel_on_poll = cancel;
        work.reject_charge = reject_charge;
        assert_eq!(
            comparator.update_bigger_smaller(64, 65, &mut work),
            Err(WorkError::Interrupted)
        );
        assert_eq!(work.events, events);
        assert_eq!(work.remaining, budget);
        assert_eq!(comparator.relations.facts(), before);
        assert_eq!(comparator.relations.scratch_buffers(), buffers);
    }
    let mut work = TraceWork::new(PRICE);
    comparator.update_bigger_smaller(64, 65, &mut work).unwrap();
    assert_eq!(work.remaining, 0);
    assert_eq!(
        comparator.relations.cached_ordering(0, 129),
        Some(Ordering::Greater)
    );
    assert_eq!(
        comparator
            .relations
            .update_work(64, 65, &mut NoopWorkControl),
        Ok(FLOOR)
    );
}

#[test]
fn sparse_word_pricing_does_not_assume_sixty_four_members_per_word() {
    let mut relations = ModelOrderRelations::default();
    for slot in 0..=256 {
        // Register empty intermediate slots without changing the bridge shape.
        relations.slot_for(slot);
    }
    for (bigger, smaller) in [(64, 0), (128, 0), (1, 192), (1, 256)] {
        relations.update(bigger, smaller);
    }
    // The 3-by-3 rectangle uses 3 occupied words on each side. Price both scans in full.
    let floor = 6 + 2 * (3 + 3 * 3);
    let price = floor + 9;
    let mut work = TraceWork::new(price);
    assert_eq!(relations.update_work(0, 1, &mut work), Ok(price));
    assert_eq!(work.events, [WorkEvent::Check(floor)]);
    relations.update(0, 1);
    assert_eq!(relations.update_work(0, 1, &mut NoopWorkControl), Ok(floor));
}

#[test]
fn pre_cancelled_relation_cannot_register_its_first_slot() {
    struct Cancelled;
    impl WorkControl for Cancelled {
        fn check(&mut self, _units: usize) -> Result<(), WorkError> {
            Err(WorkError::Interrupted)
        }
        fn charge(&mut self, _units: usize) -> Result<(), WorkError> {
            panic!("a pre-cancelled relation charged work")
        }
    }

    let graph = LGraph::new("pre-cancelled-relation", LayeredOptions::default());
    let mut comparator =
        ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::NodesAndEdges);
    assert_eq!(
        comparator.update_bigger_smaller(usize::MAX, 0, &mut Cancelled),
        Err(WorkError::Interrupted)
    );
    assert!(comparator.relations.is_clear());
    assert_eq!(comparator.relations.rows.capacity(), 0);
    assert!(comparator.relations.smaller_snapshot.is_empty());
    let mut fresh = TraceWork::new(2);
    comparator
        .update_bigger_smaller(usize::MAX, 0, &mut fresh)
        .unwrap();
    assert_eq!(
        comparator.relations.cached_ordering(usize::MAX, 0),
        Some(Ordering::Greater)
    );
}

#[test]
fn port_price_pays_for_both_word_scans_before_reserving_one_application() {
    use WorkEvent::Check;
    let mut relations = ModelOrderRelations::default();
    for (bigger, smaller) in [(0, 2), (3, 0), (1, 4), (5, 1)] {
        relations.update(bigger, smaller);
    }
    let mut work = TraceWork::new(19);
    // Each direction scans 2 rows and 2 words. The application adds 2 snapshot copies
    // and 4 reflected bits to that scan. The comparison itself costs one more unit.
    assert_eq!(relations.comparison_work(0, 1, &mut work), Ok((19, None)));
    assert_eq!(work.events, [Check(11), Check(15)]);
    assert_eq!(work.remaining, 19);
}

#[test]
fn bitmap_relation_work_arithmetic_fails_closed() {
    assert_eq!(
        row_union_work(usize::MAX, 2),
        Err(WorkError::ArithmeticOverflow)
    );
    assert_eq!(
        row_union_work(usize::MAX, 1),
        Err(WorkError::ArithmeticOverflow)
    );
    assert_eq!(
        pricing_scan_work(usize::MAX, 2),
        Err(WorkError::ArithmeticOverflow)
    );
    assert_eq!(
        UpdateWork {
            pricing: usize::MAX,
            apply: 1
        }
        .total(),
        Err(WorkError::ArithmeticOverflow)
    );
}

#[test]
fn snapshot_updates_admit_exact_work_and_preserve_state_on_check_or_charge_failure() {
    use WorkEvent::{Charge, Check};
    let crossed = LegacyRelations {
        bigger_than: HashMap::from([(40, HashSet::from([10, 20])), (50, HashSet::from([10, 20]))]),
        smaller_than: HashMap::from([(40, HashSet::from([10, 20])), (50, HashSet::from([10, 20]))]),
    };
    let reverse = LegacyRelations {
        bigger_than: HashMap::from([(40, HashSet::from([50]))]),
        smaller_than: HashMap::from([(50, HashSet::from([40]))]),
    };
    let graph = LGraph::new("snapshot-update-work", LayeredOptions::default());
    for (name, seed, large, small, required, scan_floor) in [
        ("empty", LegacyRelations::default(), 40, 50, 2, None),
        (
            "one-sided",
            LegacyRelations {
                bigger_than: HashMap::from([(50, HashSet::from([10, 20]))]),
                ..LegacyRelations::default()
            },
            40,
            50,
            10,
            None,
        ),
        ("crossed-overlap", crossed.clone(), 40, 50, 14, None),
        ("self", crossed, 40, 40, 14, None),
        ("reverse", reverse.clone(), 50, 40, 13, Some(10)),
        ("already-learned", reverse, 40, 50, 2, None),
    ] {
        let mut smaller_members = seed.bigger_than.get(&small).cloned().unwrap_or_default();
        let mut larger_members = seed.smaller_than.get(&large).cloned().unwrap_or_default();
        if !smaller_members.is_empty() || !larger_members.is_empty() {
            smaller_members.insert(small);
            larger_members.insert(large);
        }
        for (budget, reject_charge) in [(required - 1, false), (required, true), (required, false)]
        {
            let mut comparator =
                ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::NodesAndEdges);
            comparator.relations = seed.candidate();
            let smaller_scratch = [Word::member(91), Word::member(92)];
            let larger_scratch = [Word::member(93)];
            comparator.relations.smaller_snapshot = smaller_scratch.to_vec();
            comparator.relations.larger_snapshot = larger_scratch.to_vec();
            let buffers = comparator.relations.scratch_buffers();
            let capacities = (
                comparator.relations.slots.capacity(),
                comparator.relations.rows.capacity(),
            );
            assert_eq!(
                comparator
                    .relations
                    .update_work(large, small, &mut NoopWorkControl),
                Ok(required),
                "{name}",
            );
            let mut work = TraceWork::new(budget);
            work.reject_charge = reject_charge;
            let result = comparator.update_bigger_smaller(large, small, &mut work);
            let mut expected_events: Vec<_> = scan_floor.into_iter().map(Check).collect();
            expected_events.push(Check(required));
            if budget >= required {
                expected_events.push(Charge(required));
            }
            assert_eq!(work.events, expected_events);
            if budget < required || reject_charge {
                assert_eq!(result, Err(WorkError::Interrupted), "{name}");
                assert_eq!(work.remaining, budget);
                assert_state(&comparator.relations, &seed);
                assert_eq!(comparator.relations.smaller_snapshot, smaller_scratch);
                assert_eq!(comparator.relations.larger_snapshot, larger_scratch);
                assert_eq!(comparator.relations.scratch_buffers(), buffers);
                assert_eq!(
                    (
                        comparator.relations.slots.capacity(),
                        comparator.relations.rows.capacity(),
                    ),
                    capacities,
                );
            } else {
                assert_eq!(result, Ok(()), "{name}");
                assert_eq!(work.remaining, 0);
                let mut expected = seed.clone();
                expected.update(large, small);
                assert_state(&comparator.relations, &expected);
                assert_eq!(
                    comparator
                        .relations
                        .snapshot_members(&comparator.relations.smaller_snapshot),
                    smaller_members,
                );
                assert_eq!(
                    comparator
                        .relations
                        .snapshot_members(&comparator.relations.larger_snapshot),
                    larger_members,
                );
            }
        }
    }
}

#[test]
fn port_precharge_failure_preserves_facts_keys_and_scratch_before_graph_access() {
    use WorkEvent::{Charge, Check};
    let graph = LGraph::new("snapshot-port-rejection", LayeredOptions::default());
    let orders = HashMap::new();
    let seed = LegacyRelations {
        bigger_than: HashMap::from([(50, HashSet::from([10, 20]))]),
        smaller_than: HashMap::from([(40, HashSet::from([60, 70]))]),
    };
    for (budget, reject_charge) in [(14, false), (15, true)] {
        let mut comparator = ModelOrderPortComparator::new(
            &graph,
            usize::MAX,
            &[],
            OrderingStrategy::NodesAndEdges,
            &orders,
            false,
        );
        comparator.relations = seed.candidate();
        let smaller_scratch = [Word::member(91), Word::member(92)];
        let larger_scratch = [Word::member(93)];
        comparator.relations.smaller_snapshot = smaller_scratch.to_vec();
        comparator.relations.larger_snapshot = larger_scratch.to_vec();
        let buffers = comparator.relations.scratch_buffers();
        let capacities = (
            comparator.relations.slots.capacity(),
            comparator.relations.rows.capacity(),
        );
        let mut work = TraceWork::new(budget);
        work.reject_charge = reject_charge;
        assert_eq!(
            comparator.compare_with_work(40, 50, &mut work),
            Err(WorkError::Interrupted),
        );
        assert_eq!(work.remaining, budget);
        assert_state(&comparator.relations, &seed);
        assert_eq!(comparator.relations.smaller_snapshot, smaller_scratch);
        assert_eq!(comparator.relations.larger_snapshot, larger_scratch);
        assert_eq!(comparator.relations.scratch_buffers(), buffers);
        assert_eq!(
            (
                comparator.relations.slots.capacity(),
                comparator.relations.rows.capacity(),
            ),
            capacities,
        );
        let mut expected_events = vec![Check(15)];
        if reject_charge {
            expected_events.push(Charge(15));
        }
        assert_eq!(work.events, expected_events);
    }
}

fn connect(graph: &mut LGraph, source: usize, target: usize) {
    let source_port = graph
        .add_port(source, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();
    let target_port = graph
        .add_port(target, PortType::Input, PortSide::West, LPoint::default())
        .unwrap();
    graph
        .add_edge(LayeredEdge {
            id: format!("{source}-{target}"),
            source: source_port,
            target: target_port,
            source_node_id: graph.layerless_nodes[source].id.clone(),
            target_node_id: graph.layerless_nodes[target].id.clone(),
            labels: Vec::new(),
            minlen: 1,
            reversed: false,
            bend_points: Vec::new(),
            model_order: None,
            priority_direction: 0,
            priority_shortness: 0,
            priority_straightness: 0,
            thickness: 0.0,
            original_opposite_port: None,
            compound_segment: None,
        })
        .unwrap();
}

#[test]
fn feedback_admission_preserves_each_snapshot_prefix_and_legacy_query_result() {
    use WorkEvent::{Charge, Check};
    let mut graph = LGraph::new("relation-feedback", LayeredOptions::default());
    for (index, layer, kind) in [
        (0, 0, LNodeKind::Normal),
        (1, 1, LNodeKind::Normal),
        (2, 2, LNodeKind::Normal),
        (3, 1, LNodeKind::LongEdge),
        (4, 1, LNodeKind::LongEdge),
    ] {
        let mut node = LNode::new(format!("node-{index}"), 10.0, 10.0, None);
        node.kind = kind;
        graph.layerless_nodes.push(node);
        graph.set_node_layer(index, layer);
    }
    for (source, target) in [(0, 4), (4, 1), (1, 3), (3, 2)] {
        connect(&mut graph, source, target);
    }
    for budget in [10, 11, 20, 23, 24] {
        let mut comparator = ModelOrderNodeComparator::new(
            &graph,
            &graph.layers[0].nodes,
            OrderingStrategy::NodesAndEdges,
        );
        comparator.before_ports = true;
        let mut work = TraceWork::new(budget);
        let result = comparator.compare_with_work(3, 4, &mut work);
        let mut legacy = LegacyRelations::default();
        assert_eq!(legacy.cached_ordering(3, 4), None);
        let mut expected_events = vec![Check(9), Charge(9), Check(2)];
        if budget >= 11 {
            legacy.update(4, 3);
            expected_events.extend([Charge(2), Check(10)]);
        }
        if budget >= 21 {
            expected_events.push(Check(13));
        }
        if budget == 24 {
            legacy.update(3, 4);
            expected_events.push(Charge(13));
            assert_eq!(result, Ok(Ordering::Greater));
        } else {
            assert_eq!(result, Err(WorkError::Interrupted));
        }
        assert_eq!(work.events, expected_events);
        assert_observations(&comparator.relations, &legacy, &[3, 4]);
        if budget == 24 {
            assert_eq!(legacy.cached_ordering(3, 4), Some(Ordering::Greater));
            assert_eq!(legacy.cached_ordering(4, 3), Some(Ordering::Greater));
            assert!(comparator.relations.bigger_members(3).contains(&3));
            assert!(comparator.relations.smaller_members(4).contains(&4));
        }
    }
}

#[test]
fn helper_continuation_records_a_node_outside_the_sorted_layer() {
    let mut graph = LGraph::new("relation-domain", LayeredOptions::default());
    for (index, layer, order) in [
        (0, 0, Some(5)),
        (1, 1, Some(7)),
        (2, 1, Some(9)),
        (3, 1, None),
    ] {
        graph
            .layerless_nodes
            .push(LNode::new(format!("node-{index}"), 10.0, 10.0, order));
        graph.set_node_layer(index, layer);
    }
    graph.layerless_nodes[3].kind = LNodeKind::LongEdge;
    connect(&mut graph, 0, 3);
    connect(&mut graph, 3, 1);
    assert!(!graph.layers[1].nodes.contains(&0));
    let mut comparator = ModelOrderNodeComparator::new(
        &graph,
        &graph.layers[0].nodes,
        OrderingStrategy::NodesAndEdges,
    );
    let mut work = TraceWork::new(13);
    assert_eq!(
        comparator.compare_with_work(3, 2, &mut work),
        Ok(Ordering::Less)
    );
    assert_eq!(work.remaining, 0);
    let mut legacy = LegacyRelations::default();
    legacy.cached_ordering(3, 2);
    legacy.cached_ordering(0, 2);
    legacy.update(2, 0);
    legacy.update(2, 3);
    assert_observations(&comparator.relations, &legacy, &[0, 1, 2, 3]);
    assert!(comparator.relations.bigger_members(2).contains(&0));
}

#[test]
fn equal_node_model_orders_learn_the_first_comparison_direction() {
    let mut graph = LGraph::new("relation-ties", LayeredOptions::default());
    for index in 0..2 {
        graph
            .layerless_nodes
            .push(LNode::new(format!("node-{index}"), 10.0, 10.0, Some(7)));
    }
    for (first, second) in [(0, 1), (1, 0)] {
        let mut comparator =
            ModelOrderNodeComparator::new(&graph, &[], OrderingStrategy::PreferNodes);
        let mut work = TraceWork::new(4);
        assert_eq!(
            comparator.compare_with_work(first, second, &mut work),
            Ok(Ordering::Less),
        );
        let mut legacy = LegacyRelations::default();
        legacy.update(second, first);
        assert_observations(&comparator.relations, &legacy, &[0, 1]);
        assert_eq!(
            comparator.compare_with_work(second, first, &mut work),
            Ok(Ordering::Greater),
        );
        assert_eq!(work.remaining, 0);
        assert_state(&comparator.relations, &legacy);
    }
}

#[test]
fn port_self_comparison_keeps_legacy_admission_and_non_equal_results() {
    use WorkEvent::{Charge, Check};
    let mut graph = LGraph::new("relation-port-self", LayeredOptions::default());
    graph
        .layerless_nodes
        .push(LNode::new("owner", 10.0, 10.0, None));
    graph
        .add_port(0, PortType::Output, PortSide::East, LPoint::default())
        .unwrap();
    let target_orders = HashMap::new();
    let mut comparator = ModelOrderPortComparator::new(
        &graph,
        0,
        &[],
        OrderingStrategy::NodesAndEdges,
        &target_orders,
        false,
    );
    let mut rejected = TraceWork::new(2);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut rejected),
        Err(WorkError::Interrupted),
    );
    assert_eq!(rejected.events, [Check(3)]);
    assert_state(&comparator.relations, &LegacyRelations::default());

    let mut work = TraceWork::new(4);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut work),
        Ok(Ordering::Less),
    );
    let mut legacy = LegacyRelations::default();
    legacy.update(0, 0);
    assert_observations(&comparator.relations, &legacy, &[0]);
    assert_eq!(
        comparator.compare_with_work(0, 0, &mut work),
        Ok(Ordering::Greater),
    );
    assert_eq!(work.events, [Check(3), Charge(3), Check(1), Charge(1)]);
    assert_eq!(work.remaining, 0);
    assert_state(&comparator.relations, &legacy);
}
