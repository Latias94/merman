//! Exact whole-primitive Bentley-Ottmann `y`-sweep for strictly `y`-monotone centerlines.
//!
//! Large compound fans (such as a star node connecting across nested namespaces) place many
//! wide, non-crossing S-curves in the same horizontal band so AABB band queries test
//! `Omega(E^2)` pairs, while chord-by-chord sweeping pays `O(E * CHORDS * log E)` heap
//! events. Treating each strictly `y`-monotone primitive as a single chain in a flat-arena
//! AVL tree reduces heap traffic to `2 * N + 2 * K` events (`EV_START`, `EV_END`, `EV_CROSS`)
//! and only runs a fast two-pointer chord merge when two adjacent chains actually overlap in
//! bounding box.

use super::{
    Centerlines, Chord, Endpoint, LayoutOutput, LayoutResult, Point, Primitive, Work,
    bounds_in_shared_fan, shared_fan,
};
use crate::work::{filled, push, reserved};
use crate::{LayoutError, WorkError};
use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::BinaryHeap;

/// Compact deterministic open-addressed set for canonical `(a, b)` (`a < b`) edge pairs.
///
/// Only membership (`contains`), insertion (`insert`), and cardinality (`len`) are exposed;
/// the table is never iterated, so layout decisions remain 100% deterministic.
pub(super) struct PairSet {
    slots: Vec<u64>,
    len: usize,
}

impl PairSet {
    pub(super) fn new() -> Self {
        Self {
            slots: Vec::new(),
            len: 0,
        }
    }

    #[inline]
    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[inline]
    fn encode((a, b): (u32, u32)) -> u64 {
        debug_assert!(a < b);
        (u64::from(a) << 32) | u64::from(b)
    }

    #[inline]
    fn hash(mut key: u64) -> usize {
        key ^= key >> 30;
        key = key.wrapping_mul(0xbf58_476d_1ce4_e5b9);
        key ^= key >> 27;
        key = key.wrapping_mul(0x94d0_49bb_1331_11eb);
        key ^= key >> 31;
        key as usize
    }

    #[inline]
    pub(super) fn contains(&self, pair: (u32, u32)) -> bool {
        let cap = self.slots.len();
        if cap == 0 {
            return false;
        }
        let key = Self::encode(pair);
        let mask = cap - 1;
        let mut idx = Self::hash(key) & mask;
        loop {
            let slot = self.slots[idx];
            if slot == 0 {
                return false;
            }
            if slot == key {
                return true;
            }
            idx = (idx + 1) & mask;
        }
    }

    pub(super) fn insert(&mut self, pair: (u32, u32), work: &mut Work<'_>) -> LayoutResult<bool> {
        let key = Self::encode(pair);
        if self.slots.is_empty() || (self.len + 1) * 10 >= self.slots.len() * 7 {
            self.grow(work)?;
        }
        let mask = self.slots.len() - 1;
        let mut idx = Self::hash(key) & mask;
        loop {
            let slot = self.slots[idx];
            if slot == 0 {
                self.slots[idx] = key;
                self.len += 1;
                return Ok(true);
            }
            if slot == key {
                return Ok(false);
            }
            idx = (idx + 1) & mask;
        }
    }

    fn grow(&mut self, work: &mut Work<'_>) -> LayoutResult<()> {
        let new_cap = if self.slots.is_empty() {
            64
        } else {
            self.slots
                .len()
                .checked_mul(2)
                .ok_or(WorkError::ArithmeticOverflow)?
        };
        work.charge(new_cap)?;
        let mut next = filled(new_cap, 0u64)?;
        let mask = new_cap - 1;
        for &key in &self.slots {
            if key != 0 {
                let mut idx = Self::hash(key) & mask;
                while next[idx] != 0 {
                    idx = (idx + 1) & mask;
                }
                next[idx] = key;
            }
        }
        self.slots = next;
        Ok(())
    }
}

/// Deterministic open-addressed schedule cache keyed by ordered active chain pair `(a, b)`.
///
/// Stores the `y` coordinate of the pending `EV_CROSS(a, b)` event, or `f64::INFINITY` when
/// `(a, b)` has been proven not to cross for the remainder of their vertical overlap.
/// Because the min-heap pops events in non-decreasing `y` order, any query with `cur_y < y_sched`
/// is guaranteed either to have no future crossing (`INFINITY`) or to still have its scheduled
/// `EV_CROSS` event pending in the heap.
struct PairSchedule {
    slots: Vec<(u64, f64)>,
    len: usize,
}

impl PairSchedule {
    fn new() -> Self {
        Self {
            slots: Vec::new(),
            len: 0,
        }
    }

    #[inline]
    fn encode(a: usize, b: usize) -> u64 {
        // Adding 1 guarantees non-zero keys even when `a == 0 && b == 0`.
        ((a as u64) << 32) | ((b as u64) + 1)
    }

    #[inline]
    fn is_covered(&self, a: usize, b: usize, cur_y: f64) -> bool {
        let cap = self.slots.len();
        if cap == 0 {
            return false;
        }
        let key = Self::encode(a, b);
        let mask = cap - 1;
        let mut idx = PairSet::hash(key) & mask;
        loop {
            let (slot_key, y_sched) = self.slots[idx];
            if slot_key == 0 {
                return false;
            }
            if slot_key == key {
                return cur_y < y_sched;
            }
            idx = (idx + 1) & mask;
        }
    }

    fn record(
        &mut self,
        a: usize,
        b: usize,
        y_sched: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let key = Self::encode(a, b);
        if self.slots.is_empty() || (self.len + 1) * 10 >= self.slots.len() * 7 {
            self.grow(work)?;
        }
        let mask = self.slots.len() - 1;
        let mut idx = PairSet::hash(key) & mask;
        loop {
            let (slot_key, slot_y) = &mut self.slots[idx];
            if *slot_key == 0 {
                *slot_key = key;
                *slot_y = y_sched;
                self.len += 1;
                return Ok(());
            }
            if *slot_key == key {
                *slot_y = y_sched;
                return Ok(());
            }
            idx = (idx + 1) & mask;
        }
    }

    fn grow(&mut self, work: &mut Work<'_>) -> LayoutResult<()> {
        let new_cap = if self.slots.is_empty() {
            256
        } else {
            self.slots
                .len()
                .checked_mul(2)
                .ok_or(WorkError::ArithmeticOverflow)?
        };
        work.charge(new_cap)?;
        let mut next = filled(new_cap, (0u64, 0.0))?;
        let mask = new_cap - 1;
        for &(key, y_sched) in &self.slots {
            if key != 0 {
                let mut idx = PairSet::hash(key) & mask;
                while next[idx].0 != 0 {
                    idx = (idx + 1) & mask;
                }
                next[idx] = (key, y_sched);
            }
        }
        self.slots = next;
        Ok(())
    }
}

/// Minimum number of overlapping strictly `y`-monotone primitives required to amortize sweep setup.
pub(super) const MIN_SWEEP_PRIMITIVES: usize = 32;

/// Abort the sweep and fall back to band queries if pathological all-pairs crossings blow up
/// event volume.
const CROSS_EVENT_MULTIPLIER: usize = 8;
const COINCIDENT_EPS: f64 = 1e-9;
const NONE: u32 = u32::MAX;

/// Marks every primitive whose `x` AND `y` intervals overlap at least one primitive from a
/// different edge.
///
/// Any primitive marked `false` cannot intersect any primitive of another edge and can be skipped
/// before curve expansion or band insertion.
pub(super) fn x_overlap_mask(
    primitives: &[Primitive],
    work: &mut Work<'_>,
) -> LayoutResult<(Vec<bool>, usize)> {
    let n = primitives.len();
    work.charge(n.saturating_mul(2))?;
    work.charge_sort(n)?;
    let mut order: Vec<usize> = reserved(n)?;
    order.extend(0..n);

    let mark_axis = |order: &mut Vec<usize>,
                     axis_bounds: fn(&Primitive) -> [f64; 2],
                     mask: &mut [bool],
                     work: &mut Work<'_>|
     -> LayoutResult<()> {
        crate::work::sort_indices(order, &|&a, &b| {
            let aa = axis_bounds(&primitives[a]);
            let bb = axis_bounds(&primitives[b]);
            aa[0]
                .total_cmp(&bb[0])
                .then_with(|| aa[1].total_cmp(&bb[1]))
                .then_with(|| a.cmp(&b))
        });
        work.check()?;

        // Forward pass: check whether any earlier primitive from a different edge reaches `lo`.
        let mut max1 = f64::NEG_INFINITY;
        let mut max1_edge = usize::MAX;
        let mut max2 = f64::NEG_INFINITY;
        for &idx in order.iter() {
            work.visit(1)?;
            let p = &primitives[idx];
            let e = p.edge;
            let [lo, hi] = axis_bounds(p);
            let earlier_other_max = if max1_edge == e { max2 } else { max1 };
            if earlier_other_max >= lo {
                mask[idx] = true;
            }
            if e == max1_edge {
                if hi > max1 {
                    max1 = hi;
                }
            } else if hi > max1 {
                max2 = max1;
                max1 = hi;
                max1_edge = e;
            } else if hi > max2 {
                max2 = hi;
            }
        }

        // Backward pass: check whether any later primitive from a different edge starts at or before `hi`.
        let mut min1 = f64::INFINITY;
        let mut min1_edge = usize::MAX;
        let mut min2 = f64::INFINITY;
        for &idx in order.iter().rev() {
            work.visit(1)?;
            let p = &primitives[idx];
            let e = p.edge;
            let [lo, hi] = axis_bounds(p);
            let later_other_min = if min1_edge == e { min2 } else { min1 };
            if later_other_min <= hi {
                mask[idx] = true;
            }
            if e == min1_edge {
                if lo < min1 {
                    min1 = lo;
                }
            } else if lo < min1 {
                min2 = min1;
                min1 = lo;
                min1_edge = e;
            } else if lo < min2 {
                min2 = lo;
            }
        }
        Ok(())
    };

    let mut x_hit = filled(n, false)?;
    mark_axis(&mut order, |p| p.bounds.x, &mut x_hit, work)?;
    let mut y_candidates: Vec<usize> = reserved(n)?;
    for (idx, &hit) in x_hit.iter().enumerate() {
        if hit {
            y_candidates.push(idx);
        }
    }
    if y_candidates.len() < 2 {
        return Ok((filled(n, false)?, 0));
    }
    work.charge_sort(y_candidates.len())?;
    let mut y_hit = filled(n, false)?;
    mark_axis(&mut y_candidates, |p| p.bounds.y, &mut y_hit, work)?;
    let count = y_hit.iter().filter(|&&hit| hit).count();
    Ok((y_hit, count))
}

struct SweepChain {
    primitive: usize,
    edge: usize,
    rev: bool,
    x_monotone: bool,
    chord_count: usize,
    y_top: f64,
    y_bot: f64,
    x_min: f64,
    x_max: f64,
    x_top: f64,
    x_bot: f64,
    slope_top: f64,
    /// Monotonically advancing chord cursor cached as the sweep line `cur_y` descends.
    cur_chord: Cell<u16>,
}

#[derive(Clone, Copy)]
struct SweepHoriz {
    primitive: usize,
    edge: usize,
    y: f64,
    x_min: f64,
    x_max: f64,
}

#[derive(Clone, Copy)]
struct AvlNode {
    left: u32,
    right: u32,
    parent: u32,
    prev: u32,
    next: u32,
    chain: u32,
    height: u8,
}

/// Zero-allocation flat-arena AVL tree maintaining active chains in left-to-right `x(y)` order
/// with an intrusive doubly-linked in-order list (`prev`/`next`).
///
/// `insert`, `remove`, and `lower_bound_x` are `O(log N)` worst-case; `prev_chain`, `next_chain`,
/// `is_adjacent`, and `swap_adjacent` are `O(1)`.
struct ActiveTree {
    nodes: Vec<AvlNode>,
    node_of: Vec<u32>,
    next_slot: u32,
    root: u32,
}

impl ActiveTree {
    fn new(n: usize, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(n)?;
        let empty = AvlNode {
            left: NONE,
            right: NONE,
            parent: NONE,
            prev: NONE,
            next: NONE,
            chain: NONE,
            height: 0,
        };
        Ok(Self {
            nodes: filled(n, empty)?,
            node_of: filled(n, NONE)?,
            next_slot: 0,
            root: NONE,
        })
    }

    #[inline]
    fn is_active(&self, chain_id: usize) -> bool {
        self.node_of[chain_id] != NONE
    }

    #[inline]
    fn height(&self, u: u32) -> i8 {
        if u == NONE {
            0
        } else {
            self.nodes[u as usize].height as i8
        }
    }

    #[inline]
    fn update_height(&mut self, u: u32) {
        let l = self.nodes[u as usize].left;
        let r = self.nodes[u as usize].right;
        self.nodes[u as usize].height = (1 + self.height(l).max(self.height(r))) as u8;
    }

    fn rotate_left(&mut self, x: u32) {
        let y = self.nodes[x as usize].right;
        let b = self.nodes[y as usize].left;
        let p = self.nodes[x as usize].parent;
        self.nodes[y as usize].left = x;
        self.nodes[y as usize].parent = p;
        self.nodes[x as usize].parent = y;
        self.nodes[x as usize].right = b;
        if b != NONE {
            self.nodes[b as usize].parent = x;
        }
        if p == NONE {
            self.root = y;
        } else if self.nodes[p as usize].left == x {
            self.nodes[p as usize].left = y;
        } else {
            self.nodes[p as usize].right = y;
        }
        self.update_height(x);
        self.update_height(y);
    }

    fn rotate_right(&mut self, y: u32) {
        let x = self.nodes[y as usize].left;
        let b = self.nodes[x as usize].right;
        let p = self.nodes[y as usize].parent;
        self.nodes[x as usize].right = y;
        self.nodes[x as usize].parent = p;
        self.nodes[y as usize].parent = x;
        self.nodes[y as usize].left = b;
        if b != NONE {
            self.nodes[b as usize].parent = y;
        }
        if p == NONE {
            self.root = x;
        } else if self.nodes[p as usize].left == y {
            self.nodes[p as usize].left = x;
        } else {
            self.nodes[p as usize].right = x;
        }
        self.update_height(y);
        self.update_height(x);
    }

    fn rebalance_up(&mut self, mut u: u32, work: &mut Work<'_>) -> LayoutResult<()> {
        while u != NONE {
            work.visit(1)?;
            self.update_height(u);
            let l = self.nodes[u as usize].left;
            let r = self.nodes[u as usize].right;
            let bf = self.height(l) - self.height(r);
            if bf > 1 {
                if self.height(self.nodes[l as usize].left)
                    < self.height(self.nodes[l as usize].right)
                {
                    self.rotate_left(l);
                }
                self.rotate_right(u);
                u = self.nodes[u as usize].parent;
            } else if bf < -1 {
                if self.height(self.nodes[r as usize].right)
                    < self.height(self.nodes[r as usize].left)
                {
                    self.rotate_right(r);
                }
                self.rotate_left(u);
                u = self.nodes[u as usize].parent;
            }
            u = self.nodes[u as usize].parent;
        }
        Ok(())
    }

    fn insert(
        &mut self,
        chain_id: usize,
        cmp: impl Fn(usize, usize) -> Ordering,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let u = self.next_slot;
        self.next_slot += 1;
        self.nodes[u as usize] = AvlNode {
            left: NONE,
            right: NONE,
            parent: NONE,
            prev: NONE,
            next: NONE,
            chain: chain_id as u32,
            height: 1,
        };
        self.node_of[chain_id] = u;
        if self.root == NONE {
            self.root = u;
            return Ok(());
        }
        let mut cur = self.root;
        let mut parent = NONE;
        let mut go_left = false;
        while cur != NONE {
            work.visit(1)?;
            parent = cur;
            let other = self.nodes[cur as usize].chain as usize;
            if cmp(chain_id, other) == Ordering::Less {
                cur = self.nodes[cur as usize].left;
                go_left = true;
            } else {
                cur = self.nodes[cur as usize].right;
                go_left = false;
            }
        }
        self.nodes[u as usize].parent = parent;
        if go_left {
            self.nodes[parent as usize].left = u;
            // Insert `u` immediately before `parent` in the in-order list.
            let prev = self.nodes[parent as usize].prev;
            self.nodes[u as usize].prev = prev;
            self.nodes[u as usize].next = parent;
            self.nodes[parent as usize].prev = u;
            if prev != NONE {
                self.nodes[prev as usize].next = u;
            }
        } else {
            self.nodes[parent as usize].right = u;
            // Insert `u` immediately after `parent` in the in-order list.
            let next = self.nodes[parent as usize].next;
            self.nodes[u as usize].prev = parent;
            self.nodes[u as usize].next = next;
            self.nodes[parent as usize].next = u;
            if next != NONE {
                self.nodes[next as usize].prev = u;
            }
        }
        self.rebalance_up(parent, work)
    }

    fn remove(&mut self, chain_id: usize, work: &mut Work<'_>) -> LayoutResult<()> {
        let mut u = self.node_of[chain_id];
        if u == NONE {
            return Ok(());
        }
        if self.nodes[u as usize].left != NONE && self.nodes[u as usize].right != NONE {
            // The in-order successor is `self.nodes[u].next` directly in O(1)!
            let s = self.nodes[u as usize].next;
            let s_chain = self.nodes[s as usize].chain as usize;
            self.nodes[u as usize].chain = s_chain as u32;
            self.node_of[s_chain] = u;
            u = s;
        }
        self.node_of[chain_id] = NONE;
        // Unlink physical node `u` from the in-order doubly-linked list.
        let prev = self.nodes[u as usize].prev;
        let next = self.nodes[u as usize].next;
        if prev != NONE {
            self.nodes[prev as usize].next = next;
        }
        if next != NONE {
            self.nodes[next as usize].prev = prev;
        }
        let child = if self.nodes[u as usize].left != NONE {
            self.nodes[u as usize].left
        } else {
            self.nodes[u as usize].right
        };
        let p = self.nodes[u as usize].parent;
        if child != NONE {
            self.nodes[child as usize].parent = p;
        }
        if p == NONE {
            self.root = child;
        } else if self.nodes[p as usize].left == u {
            self.nodes[p as usize].left = child;
        } else {
            self.nodes[p as usize].right = child;
        }
        self.nodes[u as usize] = AvlNode {
            left: NONE,
            right: NONE,
            parent: NONE,
            prev: NONE,
            next: NONE,
            chain: NONE,
            height: 0,
        };
        self.rebalance_up(p, work)
    }

    #[inline]
    fn prev_chain(&self, chain_id: usize) -> Option<usize> {
        let u = self.node_of[chain_id];
        if u == NONE {
            return None;
        }
        let p = self.nodes[u as usize].prev;
        (p != NONE).then(|| self.nodes[p as usize].chain as usize)
    }

    #[inline]
    fn next_chain(&self, chain_id: usize) -> Option<usize> {
        let u = self.node_of[chain_id];
        if u == NONE {
            return None;
        }
        let n = self.nodes[u as usize].next;
        (n != NONE).then(|| self.nodes[n as usize].chain as usize)
    }

    #[inline]
    fn is_adjacent(&self, left_chain: usize, right_chain: usize) -> bool {
        let na = self.node_of[left_chain];
        let nb = self.node_of[right_chain];
        na != NONE && nb != NONE && self.nodes[na as usize].next == nb
    }

    #[inline]
    fn swap_adjacent(&mut self, a: usize, b: usize) {
        let na = self.node_of[a];
        let nb = self.node_of[b];
        self.nodes[na as usize].chain = b as u32;
        self.nodes[nb as usize].chain = a as u32;
        self.node_of[a] = nb;
        self.node_of[b] = na;
    }

    fn lower_bound_x(
        &self,
        x_lo: f64,
        cur_y: f64,
        centerlines: &Centerlines,
        chains: &[SweepChain],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<usize>> {
        let mut cur = self.root;
        let mut best = None;
        while cur != NONE {
            work.visit(1)?;
            let cid = self.nodes[cur as usize].chain as usize;
            let xc = chain_x_at_y(centerlines, &chains[cid], cur_y);
            if xc >= x_lo {
                best = Some(cid);
                cur = self.nodes[cur as usize].left;
            } else {
                cur = self.nodes[cur as usize].right;
            }
        }
        Ok(best)
    }
}

#[derive(Clone, Copy, PartialEq)]
struct OrdF64(f64);

impl Eq for OrdF64 {}

impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compact 16-byte min-heap event ordered deterministically by `(y, kind, a, b)`.
///
/// Packs `kind` (`0..=3`) in the top 2 bits of `kind_and_a` and `a` (`< 2^30`) in the low 30 bits.
#[derive(Clone, Copy, Eq, PartialEq)]
struct Event {
    y: OrdF64,
    kind_and_a: u32,
    b: u32,
}

impl Event {
    #[inline]
    fn new(y: f64, kind: u8, a: u32, b: u32) -> Self {
        debug_assert!(kind < 4 && a < (1 << 30));
        Self {
            y: OrdF64(y),
            kind_and_a: (u32::from(kind) << 30) | a,
            b,
        }
    }

    #[inline]
    fn kind(self) -> u8 {
        (self.kind_and_a >> 30) as u8
    }

    #[inline]
    fn a(self) -> usize {
        (self.kind_and_a & 0x3fff_ffff) as usize
    }
}

impl Ord for Event {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for BinaryHeap (max-heap -> min-heap).
        other
            .y
            .cmp(&self.y)
            .then_with(|| other.kind_and_a.cmp(&self.kind_and_a))
            .then_with(|| other.b.cmp(&self.b))
    }
}

impl PartialOrd for Event {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

const EV_CROSS: u8 = 0;
const EV_START: u8 = 1;
const EV_HORIZ: u8 = 2;
const EV_END: u8 = 3;

pub(super) struct MonotonePartition {
    pub(super) in_sweep: Vec<bool>,
    chains: Vec<SweepChain>,
    horizontals: Vec<SweepHoriz>,
    pub(super) other: Vec<usize>,
    total_chords: usize,
}

impl MonotonePartition {
    pub(super) fn build(
        centerlines: &mut Centerlines,
        x_overlaps: &[bool],
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let count = centerlines.primitives.len();
        let mut candidate_count = 0usize;
        for (index, primitive) in centerlines.primitives.iter().enumerate() {
            work.visit(1)?;
            if !x_overlaps[index] {
                continue;
            }
            if primitive.bounds.y[0] < primitive.bounds.y[1]
                && primitive.cubic.as_ref().is_none_or(|cubic| {
                    let ys = [
                        primitive.ends[0].y,
                        cubic.handles[0].y,
                        cubic.handles[1].y,
                        primitive.ends[1].y,
                    ];
                    (ys[0] <= ys[1] && ys[1] <= ys[2] && ys[2] <= ys[3] && ys[0] < ys[3])
                        || (ys[0] >= ys[1] && ys[1] >= ys[2] && ys[2] >= ys[3] && ys[0] > ys[3])
                })
            {
                candidate_count += 1;
            }
        }
        if candidate_count < MIN_SWEEP_PRIMITIVES {
            return Ok(None);
        }
        work.charge(count)?;
        let mut in_sweep = filled(count, false)?;
        let mut chains = reserved(candidate_count)?;
        let mut horizontals = Vec::new();
        let mut other = Vec::new();
        let mut total_chords = 0usize;
        for index in 0..count {
            work.visit(1)?;
            if !x_overlaps[index] {
                continue;
            }
            let primitive = &centerlines.primitives[index];
            let edge = primitive.edge;
            let bounds = primitive.bounds;
            if primitive.cubic.is_none() && bounds.y[0] == bounds.y[1] && bounds.x[0] < bounds.x[1]
            {
                in_sweep[index] = true;
                push(
                    &mut horizontals,
                    SweepHoriz {
                        primitive: index,
                        edge,
                        y: bounds.y[0],
                        x_min: bounds.x[0],
                        x_max: bounds.x[1],
                    },
                )?;
                continue;
            }
            let may_be_monotone = bounds.y[0] < bounds.y[1]
                && primitive.cubic.as_ref().is_none_or(|cubic| {
                    let ys = [
                        primitive.ends[0].y,
                        cubic.handles[0].y,
                        cubic.handles[1].y,
                        primitive.ends[1].y,
                    ];
                    (ys[0] <= ys[1] && ys[1] <= ys[2] && ys[2] <= ys[3] && ys[0] < ys[3])
                        || (ys[0] >= ys[1] && ys[1] >= ys[2] && ys[2] >= ys[3] && ys[0] > ys[3])
                });
            if !may_be_monotone {
                push(&mut other, index)?;
                continue;
            }
            if centerlines.primitives[index].cubic.is_some() {
                centerlines.expand(index, work)?;
            }
            let points = centerlines.points(index);
            if points.len() < 2 {
                push(&mut other, index)?;
                continue;
            }
            let rev = points[0].y > points[points.len() - 1].y;
            let strict = if rev {
                points.windows(2).all(|w| w[0].y > w[1].y)
            } else {
                points.windows(2).all(|w| w[0].y < w[1].y)
            };
            if !strict {
                push(&mut other, index)?;
                continue;
            }
            let chord_count = points.len() - 1;
            let x_monotone = if let Some(cubic) = &centerlines.primitives[index].cubic {
                cubic.ordered[0]
            } else {
                true
            };
            let (top_pt, second_pt, bot_pt) = if rev {
                (
                    points[points.len() - 1],
                    points[points.len() - 2],
                    points[0],
                )
            } else {
                (points[0], points[1], points[points.len() - 1])
            };
            let y_top = top_pt.y;
            let y_bot = bot_pt.y;
            let x_top = top_pt.x;
            let x_bot = bot_pt.x;
            let slope_top = (second_pt.x - top_pt.x) / (second_pt.y - top_pt.y);
            total_chords = total_chords.saturating_add(chord_count);
            in_sweep[index] = true;
            chains.push(SweepChain {
                primitive: index,
                edge,
                rev,
                x_monotone,
                chord_count,
                y_top,
                y_bot,
                x_min: bounds.x[0],
                x_max: bounds.x[1],
                x_top,
                x_bot,
                slope_top,
                cur_chord: Cell::new(0),
            });
        }
        if chains.len() < MIN_SWEEP_PRIMITIVES {
            return Ok(None);
        }
        Ok(Some(Self {
            in_sweep,
            chains,
            horizontals,
            other,
            total_chords,
        }))
    }
}

#[inline]
fn ordered_chord_in(points: &[Point], chain: &SweepChain, k: usize) -> (Chord, [Point; 2]) {
    let raw_index = if chain.rev {
        chain.chord_count - 1 - k
    } else {
        k
    };
    let raw_ends = [points[raw_index], points[raw_index + 1]];
    let ordered = if chain.rev {
        [raw_ends[1], raw_ends[0]]
    } else {
        raw_ends
    };
    (
        Chord {
            primitive: chain.primitive,
            index: raw_index,
            ends: raw_ends,
        },
        ordered,
    )
}

#[inline]
fn ordered_ends_in(points: &[Point], chain: &SweepChain, k: usize) -> [Point; 2] {
    if chain.rev {
        let i = chain.chord_count - 1 - k;
        [points[i + 1], points[i]]
    } else {
        [points[k], points[k + 1]]
    }
}

/// Binary searches for the chord `k in 0..chord_count` whose vertical interval covers `y`
/// with right-continuous tie breaking (`top.y <= y < bot.y` except clamped at the ends).
#[inline]
fn chord_index_in(points: &[Point], chain: &SweepChain, y: f64) -> usize {
    if chain.chord_count <= 1 || y <= chain.y_top {
        return 0;
    }
    if y >= chain.y_bot {
        return chain.chord_count - 1;
    }
    let mut lo = 0usize;
    let mut hi = chain.chord_count - 1;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        let bot_y = if chain.rev {
            points[chain.chord_count - 1 - mid].y
        } else {
            points[mid + 1].y
        };
        if bot_y <= y {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Monotonic chord lookup leveraging the sweep line's non-decreasing `cur_y`.
///
/// Advances `chain.cur_chord` in `O(1)` when `y` stays in the cached chord or moves to the next
/// chord, falling back to binary search over the remaining tail only on larger jumps.
#[inline]
fn chord_index_monotonic(points: &[Point], chain: &SweepChain, y: f64) -> usize {
    if chain.chord_count <= 1 || y <= chain.y_top {
        return 0;
    }
    if y >= chain.y_bot {
        return chain.chord_count - 1;
    }
    let k = (chain.cur_chord.get() as usize).min(chain.chord_count - 1);
    let top_y = |i: usize| {
        if chain.rev {
            points[chain.chord_count - i].y
        } else {
            points[i].y
        }
    };
    let bot_y = |i: usize| {
        if chain.rev {
            points[chain.chord_count - 1 - i].y
        } else {
            points[i + 1].y
        }
    };
    if top_y(k) <= y {
        if y < bot_y(k) {
            return k;
        }
        let next = k + 1;
        if next < chain.chord_count && y < bot_y(next) {
            if next <= u16::MAX as usize {
                chain.cur_chord.set(next as u16);
            }
            return next;
        }
        let mut lo = next.min(chain.chord_count - 1);
        let mut hi = chain.chord_count - 1;
        while lo < hi {
            let mid = lo + (hi - lo) / 2;
            if bot_y(mid) <= y {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo <= u16::MAX as usize {
            chain.cur_chord.set(lo as u16);
        }
        return lo;
    }
    chord_index_in(points, chain, y)
}

#[inline]
fn x_at_y([top, bot]: [Point; 2], y: f64) -> f64 {
    if y <= top.y {
        top.x
    } else if y >= bot.y {
        bot.x
    } else {
        let dy = bot.y - top.y;
        let t = (y - top.y) / dy;
        top.x + (bot.x - top.x) * t
    }
}

#[inline]
fn slope_dx_dy([top, bot]: [Point; 2]) -> f64 {
    (bot.x - top.x) / (bot.y - top.y)
}

/// Evaluates `(x(y), outgoing_dx_dy(y))` on a whole `y`-monotone chain at the current sweep `y`.
#[inline]
fn chain_x_and_slope(centerlines: &Centerlines, chain: &SweepChain, y: f64) -> (f64, f64) {
    if y <= chain.y_top {
        return (chain.x_top, chain.slope_top);
    }
    if chain.chord_count == 1 {
        let x = if y >= chain.y_bot {
            chain.x_bot
        } else if chain.slope_top == 0.0 {
            chain.x_top
        } else {
            chain.x_top + (y - chain.y_top) * chain.slope_top
        };
        return (x, chain.slope_top);
    }
    let points = centerlines.points(chain.primitive);
    let k = chord_index_monotonic(points, chain, y);
    let ord = ordered_ends_in(points, chain, k);
    (x_at_y(ord, y), slope_dx_dy(ord))
}

#[inline]
fn chain_x_at_y(centerlines: &Centerlines, chain: &SweepChain, y: f64) -> f64 {
    if y <= chain.y_top {
        return chain.x_top;
    }
    if y >= chain.y_bot {
        return chain.x_bot;
    }
    if chain.chord_count == 1 {
        return if chain.slope_top == 0.0 {
            chain.x_top
        } else {
            chain.x_top + (y - chain.y_top) * chain.slope_top
        };
    }
    let points = centerlines.points(chain.primitive);
    let k = chord_index_monotonic(points, chain, y);
    let ord = ordered_ends_in(points, chain, k);
    x_at_y(ord, y)
}

/// Computes the exact `y` coordinate where `a` (currently left of `b`) moves strictly right of
/// `b` within `[max(cur_y, a.top.y, b.top.y), min(a.bot.y, b.bot.y)]`.
#[inline]
fn geometric_crossing_y(a: [Point; 2], b: [Point; 2], cur_y: f64) -> Option<f64> {
    let y_lo = cur_y.max(a[0].y).max(b[0].y);
    let y_hi = a[1].y.min(b[1].y);
    if y_hi < y_lo {
        return None;
    }
    let ax_lo = x_at_y(a, y_lo);
    let bx_lo = x_at_y(b, y_lo);
    let ax_hi = x_at_y(a, y_hi);
    let bx_hi = x_at_y(b, y_hi);
    let d_lo = ax_lo - bx_lo;
    let d_hi = ax_hi - bx_hi;
    if d_lo == 0.0 && d_hi == 0.0 {
        return None;
    }
    if d_lo <= 0.0 && d_hi <= 0.0 {
        return None;
    }
    if d_lo >= 0.0 {
        if d_lo > 0.0 || d_hi > 0.0 {
            return Some(y_lo);
        }
        return None;
    }
    let denom = d_lo - d_hi;
    let frac = (d_lo / denom).clamp(0.0, 1.0);
    let y_cross = (y_lo + (y_hi - y_lo) * frac).clamp(y_lo, y_hi);
    y_cross.is_finite().then_some(y_cross)
}

#[inline]
fn edge_pair(a: usize, b: usize) -> (u32, u32) {
    if a < b {
        (a as u32, b as u32)
    } else {
        (b as u32, a as u32)
    }
}

/// Walks the overlapping vertical range `[y_lo, y_hi]` of two whole `y`-monotone chains `ca`
/// (currently left) and `cb` (currently right) in `O(chords_a + chords_b)` time.
///
/// Updates `crossed` if an unrecorded non-fan crossing occurs, and returns the earliest
/// `y_cross >= cur_y` (or `> cur_y` when `strict_after` is true) where `ca` moves strictly
/// right of `cb`.
#[allow(clippy::too_many_arguments)]
fn inspect_chain_pair(
    centerlines: &Centerlines,
    ca: &SweepChain,
    cb: &SweepChain,
    cur_y: f64,
    strict_after: bool,
    ends: &[[Endpoint; 2]],
    out: &LayoutOutput,
    limit: u64,
    crossed: &mut PairSet,
    work: &mut Work<'_>,
) -> LayoutResult<Option<f64>> {
    let y_lo = cur_y.max(ca.y_top).max(cb.y_top);
    let y_hi = ca.y_bot.min(cb.y_bot);
    if y_hi < y_lo {
        return Ok(None);
    }
    let pair = edge_pair(ca.edge, cb.edge);
    let mut need_cross_check =
        ca.edge != cb.edge && (crossed.len() as u64) < limit && !crossed.contains(pair);

    // Fast-reject when `ca` lies wholly left of `cb`. If `ca` lies wholly right of `cb`, the
    // chains are already inverted in `ActiveTree` and must swap immediately at `y_lo`.
    if ca.x_max < cb.x_min {
        return Ok(None);
    }
    if cb.x_max < ca.x_min {
        if need_cross_check
            && !bounds_in_shared_fan(
                centerlines.primitives[ca.primitive].bounds,
                centerlines.primitives[cb.primitive].bounds,
                ends[ca.edge],
                ends[cb.edge],
                out,
            )
            && centerlines.crosses_expanded(
                ca.primitive,
                cb.primitive,
                &|at| shared_fan(at, ends[ca.edge], ends[cb.edge], out),
                work,
            )?
        {
            crossed.insert(pair, work)?;
        }
        return Ok((!strict_after || y_lo > cur_y).then_some(y_lo));
    }

    let points_a = centerlines.points(ca.primitive);
    let points_b = centerlines.points(cb.primitive);

    // Fast path for two single-chord straight segments.
    if ca.chord_count == 1 && cb.chord_count == 1 {
        let (chord_a, ord_a) = ordered_chord_in(points_a, ca, 0);
        let (chord_b, ord_b) = ordered_chord_in(points_b, cb, 0);
        if need_cross_check
            && chord_a.bounds().overlaps(chord_b.bounds())
            && !bounds_in_shared_fan(
                chord_a.bounds(),
                chord_b.bounds(),
                ends[ca.edge],
                ends[cb.edge],
                out,
            )
            && centerlines.crossing(
                chord_a,
                chord_b,
                &|at| shared_fan(at, ends[ca.edge], ends[cb.edge], out),
                work,
            )?
        {
            crossed.insert(pair, work)?;
        }
        return Ok(geometric_crossing_y(ord_a, ord_b, y_lo)
            .filter(|&y_cross| !strict_after || y_cross > cur_y));
    }

    // Compute overlapping chord ranges ONCE and reuse them for endpoint evaluation and the merge.
    let mut ia = chord_index_monotonic(points_a, ca, y_lo);
    let mut ib = chord_index_monotonic(points_b, cb, y_lo);
    let ia_last = chord_index_in(points_a, ca, y_hi);
    let ib_last = chord_index_in(points_b, cb, y_hi);
    let mut ord_a = ordered_ends_in(points_a, ca, ia);
    let mut ord_b = ordered_ends_in(points_b, cb, ib);
    let a_lo = x_at_y(ord_a, y_lo);
    let b_lo = x_at_y(ord_b, y_lo);

    // Exact sub-interval `[y_lo, y_hi]` separation cull for `x`-monotone chains:
    // if `x` is monotone along a chain, its exact `x` range on `[y_lo, y_hi]` is bounded by
    // its endpoints `x(y_lo)` and `x(y_hi)`.
    let (ax0, ax1) = if ca.x_monotone {
        let a_hi = if ia_last == ia {
            x_at_y(ord_a, y_hi)
        } else {
            x_at_y(ordered_ends_in(points_a, ca, ia_last), y_hi)
        };
        (a_lo.min(a_hi), a_lo.max(a_hi))
    } else {
        (ca.x_min, ca.x_max)
    };
    let (bx0, bx1) = if cb.x_monotone {
        let b_hi = if ib_last == ib {
            x_at_y(ord_b, y_hi)
        } else {
            x_at_y(ordered_ends_in(points_b, cb, ib_last), y_hi)
        };
        (b_lo.min(b_hi), b_lo.max(b_hi))
    } else {
        (cb.x_min, cb.x_max)
    };
    if ax1 < bx0 {
        return Ok(None);
    }
    if bx1 < ax0 {
        if need_cross_check
            && !bounds_in_shared_fan(
                centerlines.primitives[ca.primitive].bounds,
                centerlines.primitives[cb.primitive].bounds,
                ends[ca.edge],
                ends[cb.edge],
                out,
            )
            && centerlines.crosses_expanded(
                ca.primitive,
                cb.primitive,
                &|at| shared_fan(at, ends[ca.edge], ends[cb.edge], out),
                work,
            )?
        {
            crossed.insert(pair, work)?;
        }
        return Ok((!strict_after || y_lo > cur_y).then_some(y_lo));
    }

    // If `ca` is already strictly right of `cb` at `y_lo` (e.g. a tight triple intersection where
    // an intervening chain moved out slightly after `ca` and `cb` crossed), test the full
    // expanded pair before swapping at `y_lo` so an upstream chord crossing is never missed.
    if need_cross_check && a_lo > b_lo {
        if !bounds_in_shared_fan(
            centerlines.primitives[ca.primitive].bounds,
            centerlines.primitives[cb.primitive].bounds,
            ends[ca.edge],
            ends[cb.edge],
            out,
        ) && centerlines.crosses_expanded(
            ca.primitive,
            cb.primitive,
            &|at| shared_fan(at, ends[ca.edge], ends[cb.edge], out),
            work,
        )? {
            crossed.insert(pair, work)?;
        }
        need_cross_check = false;
    }

    // Whole-primitive `shared_fan` pre-cull avoids per-chord fan checks when both chains lie
    // completely inside a shared endpoint disk.
    if need_cross_check
        && bounds_in_shared_fan(
            centerlines.primitives[ca.primitive].bounds,
            centerlines.primitives[cb.primitive].bounds,
            ends[ca.edge],
            ends[cb.edge],
            out,
        )
    {
        need_cross_check = false;
    }

    let ia_end = ia_last + 1;
    let ib_end = ib_last + 1;
    let both_x_monotone = ca.x_monotone && cb.x_monotone;
    let pt_a = |k: usize| -> Point {
        if ca.rev {
            points_a[ca.chord_count - k]
        } else {
            points_a[k]
        }
    };
    let pt_b = |k: usize| -> Point {
        if cb.rev {
            points_b[cb.chord_count - k]
        } else {
            points_b[k]
        }
    };

    let mut slope_a = slope_dx_dy(ord_a);
    let mut slope_b = slope_dx_dy(ord_b);

    while ia < ia_end && ib < ib_end {
        // Coarse 8-chord block skip when both chains are `x`-monotone and separated in `x`.
        if both_x_monotone && ia + 8 <= ia_end && ib + 8 <= ib_end {
            let a0 = pt_a(ia);
            let a8 = pt_a(ia + 8);
            let b0 = pt_b(ib);
            let b8 = pt_b(ib + 8);
            if a0.x.max(a8.x) < b0.x.min(b8.x) {
                work.visit(1)?;
                if a8.y <= b8.y {
                    ia += 8;
                    if a8.y >= y_hi || ia >= ia_end {
                        break;
                    }
                    ord_a = ordered_ends_in(points_a, ca, ia);
                    slope_a = slope_dx_dy(ord_a);
                    continue;
                } else {
                    ib += 8;
                    if b8.y >= y_hi || ib >= ib_end {
                        break;
                    }
                    ord_b = ordered_ends_in(points_b, cb, ib);
                    slope_b = slope_dx_dy(ord_b);
                    continue;
                }
            }
        }
        work.visit(1)?;

        // Fast linear slab separation reject: since both chords are straight line segments,
        // if `x_a(y) < x_b(y) - tol` at both endpoints of their closed vertical overlap
        // `[ov_lo, ov_hi]`, they are strictly disjoint in R^2 and cannot cross.
        let ax_max = ord_a[0].x.max(ord_a[1].x);
        let bx_min = ord_b[0].x.min(ord_b[1].x);
        if ax_max >= bx_min {
            let ov_lo = ord_a[0].y.max(ord_b[0].y);
            let ov_hi = ord_a[1].y.min(ord_b[1].y);
            let separated = if ov_lo < ov_hi {
                let ax_ov_lo = if ov_lo == ord_a[0].y {
                    ord_a[0].x
                } else {
                    ord_a[0].x + (ov_lo - ord_a[0].y) * slope_a
                };
                let bx_ov_lo = if ov_lo == ord_b[0].y {
                    ord_b[0].x
                } else {
                    ord_b[0].x + (ov_lo - ord_b[0].y) * slope_b
                };
                let ax_ov_hi = if ov_hi == ord_a[1].y {
                    ord_a[1].x
                } else {
                    ord_a[0].x + (ov_hi - ord_a[0].y) * slope_a
                };
                let bx_ov_hi = if ov_hi == ord_b[1].y {
                    ord_b[1].x
                } else {
                    ord_b[0].x + (ov_hi - ord_b[0].y) * slope_b
                };
                let scale = ax_ov_lo
                    .abs()
                    .max(bx_ov_lo.abs())
                    .max(ax_ov_hi.abs())
                    .max(bx_ov_hi.abs())
                    .max(1.0);
                let tol = 1e-9 * scale;
                ax_ov_lo + tol < bx_ov_lo && ax_ov_hi + tol < bx_ov_hi
            } else {
                false
            };

            if !separated {
                let ax_min = ord_a[0].x.min(ord_a[1].x);
                let bx_max = ord_b[0].x.max(ord_b[1].x);
                if need_cross_check && ax_min <= bx_max {
                    let (chord_a, _) = ordered_chord_in(points_a, ca, ia);
                    let (chord_b, _) = ordered_chord_in(points_b, cb, ib);
                    if !bounds_in_shared_fan(
                        chord_a.bounds(),
                        chord_b.bounds(),
                        ends[ca.edge],
                        ends[cb.edge],
                        out,
                    ) && centerlines.crossing(
                        chord_a,
                        chord_b,
                        &|at| shared_fan(at, ends[ca.edge], ends[cb.edge], out),
                        work,
                    )? {
                        crossed.insert(pair, work)?;
                        need_cross_check = false;
                    }
                }
                if let Some(y_cross) = geometric_crossing_y(ord_a, ord_b, y_lo)
                    && (!strict_after || y_cross > cur_y)
                {
                    return Ok(Some(y_cross));
                }
            }
        }

        if ord_a[1].y <= ord_b[1].y {
            ia += 1;
            if ord_a[1].y >= y_hi || ia >= ia_end {
                break;
            }
            ord_a = ordered_ends_in(points_a, ca, ia);
            slope_a = slope_dx_dy(ord_a);
        } else {
            ib += 1;
            if ord_b[1].y >= y_hi || ib >= ib_end {
                break;
            }
            ord_b = ordered_ends_in(points_b, cb, ib);
            slope_b = slope_dx_dy(ord_b);
        }
    }

    Ok(None)
}

/// Runs the exact whole-primitive `y`-monotone sweep over `partition.chains` and
/// `partition.horizontals`.
///
/// Returns `Ok(Some(crossed_pairs))` on success, or `Ok(None)` if the graph exceeds the
/// cross-event cap so the caller can fall back cleanly to band queries.
pub(super) fn run_monotone_sweep(
    centerlines: &Centerlines,
    partition: &mut MonotonePartition,
    ends: &[[Endpoint; 2]],
    out: &LayoutOutput,
    limit: u64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<PairSet>> {
    let chains = &partition.chains;
    let horizontals = &partition.horizontals;
    let n = chains.len();
    if n >= (1 << 30) || horizontals.len() >= (1 << 30) {
        return Ok(None);
    }
    work.charge(n.saturating_add(horizontals.len()))?;
    let mut heap = BinaryHeap::new();
    heap.try_reserve(n.saturating_mul(2).saturating_add(horizontals.len()))
        .map_err(|_| LayoutError::AllocationLimit)?;
    for (id, chain) in chains.iter().enumerate() {
        work.visit(1)?;
        heap.push(Event::new(chain.y_top, EV_START, id as u32, 0));
    }
    for (hid, horiz) in horizontals.iter().enumerate() {
        work.visit(1)?;
        heap.push(Event::new(horiz.y, EV_HORIZ, hid as u32, 0));
    }

    let mut active = ActiveTree::new(n, work)?;
    let mut crossed = PairSet::new();
    let mut schedule = PairSchedule::new();
    let cross_cap = partition
        .total_chords
        .saturating_mul(CROSS_EVENT_MULTIPLIER)
        .max(256);
    let mut cross_events = 0usize;

    let check_pair = |a_id: usize,
                      b_id: usize,
                      cur_y: f64,
                      strict_after: bool,
                      crossed: &mut PairSet,
                      schedule: &mut PairSchedule,
                      heap: &mut BinaryHeap<Event>,
                      cross_events: &mut usize,
                      work: &mut Work<'_>|
     -> LayoutResult<bool> {
        if !strict_after && schedule.is_covered(a_id, b_id, cur_y) {
            return Ok(true);
        }
        if let Some(y_cross) = inspect_chain_pair(
            centerlines,
            &chains[a_id],
            &chains[b_id],
            cur_y,
            strict_after,
            ends,
            out,
            limit,
            crossed,
            work,
        )? {
            *cross_events = cross_events.saturating_add(1);
            if *cross_events > cross_cap {
                return Ok(false);
            }
            schedule.record(a_id, b_id, y_cross, work)?;
            if heap.len() == heap.capacity() {
                heap.try_reserve(1)
                    .map_err(|_| LayoutError::AllocationLimit)?;
            }
            heap.push(Event::new(y_cross, EV_CROSS, a_id as u32, b_id as u32));
        } else {
            schedule.record(a_id, b_id, f64::INFINITY, work)?;
        }
        Ok(true)
    };

    // Always test immediate left and right neighbors; only walk further across a coincident-`x`
    // bundle when the contact lies outside `shared_fan` so star fans stay `O(1)` per insertion.
    let check_neighbors = |target_id: usize,
                           cur_y: f64,
                           active: &ActiveTree,
                           crossed: &mut PairSet,
                           schedule: &mut PairSchedule,
                           heap: &mut BinaryHeap<Event>,
                           cross_events: &mut usize,
                           work: &mut Work<'_>|
     -> LayoutResult<bool> {
        let xt = chain_x_at_y(centerlines, &chains[target_id], cur_y);
        let at = Point { x: xt, y: cur_y };
        let target_edge = chains[target_id].edge;

        let mut left = active.prev_chain(target_id);
        while let Some(left_id) = left {
            work.visit(1)?;
            if !check_pair(
                left_id,
                target_id,
                cur_y,
                false,
                crossed,
                schedule,
                heap,
                cross_events,
                work,
            )? {
                return Ok(false);
            }
            let xl = chain_x_at_y(centerlines, &chains[left_id], cur_y);
            if (xl - xt).abs() > COINCIDENT_EPS
                || shared_fan(at, ends[chains[left_id].edge], ends[target_edge], out)
            {
                break;
            }
            left = active.prev_chain(left_id);
        }

        let mut right = active.next_chain(target_id);
        while let Some(right_id) = right {
            work.visit(1)?;
            if !check_pair(
                target_id,
                right_id,
                cur_y,
                false,
                crossed,
                schedule,
                heap,
                cross_events,
                work,
            )? {
                return Ok(false);
            }
            let xr = chain_x_at_y(centerlines, &chains[right_id], cur_y);
            if (xr - xt).abs() > COINCIDENT_EPS
                || shared_fan(at, ends[target_edge], ends[chains[right_id].edge], out)
            {
                break;
            }
            right = active.next_chain(right_id);
        }
        Ok(true)
    };

    while let Some(ev) = heap.pop() {
        work.visit(1)?;
        if (crossed.len() as u64) >= limit {
            return Ok(Some(crossed));
        }
        let cur_y = ev.y.0;
        match ev.kind() {
            EV_START => {
                let id = ev.a();
                let x_id = chains[id].x_top;
                let s_id = chains[id].slope_top;
                active.insert(
                    id,
                    |_, other| {
                        let (xb, sb) = chain_x_and_slope(centerlines, &chains[other], cur_y);
                        x_id.total_cmp(&xb)
                            .then_with(|| s_id.total_cmp(&sb))
                            .then_with(|| id.cmp(&other))
                    },
                    work,
                )?;
                if !check_neighbors(
                    id,
                    cur_y,
                    &active,
                    &mut crossed,
                    &mut schedule,
                    &mut heap,
                    &mut cross_events,
                    work,
                )? {
                    return Ok(None);
                }
                if heap.len() == heap.capacity() {
                    heap.try_reserve(1)
                        .map_err(|_| LayoutError::AllocationLimit)?;
                }
                heap.push(Event::new(chains[id].y_bot, EV_END, id as u32, 0));
            }
            EV_HORIZ => {
                let h = &horizontals[ev.a()];
                let x_lo = h.x_min - COINCIDENT_EPS;
                let x_hi = h.x_max + COINCIDENT_EPS;
                let chord_h = centerlines.chord(h.primitive, 0);
                let h_bounds = chord_h.bounds();
                let mut cur = active.lower_bound_x(x_lo, cur_y, centerlines, chains, work)?;
                while let Some(cid) = cur {
                    work.visit(1)?;
                    let c = &chains[cid];
                    let points_c = centerlines.points(c.primitive);
                    let k = chord_index_monotonic(points_c, c, cur_y);
                    let (chord_c, ord_c) = ordered_chord_in(points_c, c, k);
                    let xc = x_at_y(ord_c, cur_y);
                    if xc > x_hi {
                        break;
                    }
                    if c.edge != h.edge {
                        let pair = edge_pair(h.edge, c.edge);
                        if !crossed.contains(pair) {
                            let mut hit = chord_c.bounds().overlaps(h_bounds)
                                && !bounds_in_shared_fan(
                                    h_bounds,
                                    chord_c.bounds(),
                                    ends[h.edge],
                                    ends[c.edge],
                                    out,
                                )
                                && centerlines.crossing(
                                    chord_h,
                                    chord_c,
                                    &|at| shared_fan(at, ends[h.edge], ends[c.edge], out),
                                    work,
                                )?;
                            if !hit && k > 0 && ord_c[0].y == cur_y {
                                let (prev_chord, _) = ordered_chord_in(points_c, c, k - 1);
                                hit = prev_chord.bounds().overlaps(h_bounds)
                                    && !bounds_in_shared_fan(
                                        h_bounds,
                                        prev_chord.bounds(),
                                        ends[h.edge],
                                        ends[c.edge],
                                        out,
                                    )
                                    && centerlines.crossing(
                                        chord_h,
                                        prev_chord,
                                        &|at| shared_fan(at, ends[h.edge], ends[c.edge], out),
                                        work,
                                    )?;
                            }
                            if hit {
                                crossed.insert(pair, work)?;
                                if (crossed.len() as u64) >= limit {
                                    return Ok(Some(crossed));
                                }
                            }
                        }
                    }
                    cur = active.next_chain(cid);
                }
            }
            EV_END => {
                let id = ev.a();
                if !active.is_active(id) {
                    continue;
                }
                let left_neighbor = active.prev_chain(id);
                let right_neighbor = active.next_chain(id);
                // Immediate neighbors were already checked across their full overlap when they
                // became adjacent; only check non-adjacent coincident-`x` bundle peers outside
                // `shared_fan`.
                let xt = chain_x_at_y(centerlines, &chains[id], cur_y);
                let at = Point { x: xt, y: cur_y };
                let id_edge = chains[id].edge;
                if let Some(l0) = left_neighbor
                    && (chain_x_at_y(centerlines, &chains[l0], cur_y) - xt).abs() <= COINCIDENT_EPS
                    && !shared_fan(at, ends[chains[l0].edge], ends[id_edge], out)
                {
                    let mut left = active.prev_chain(l0);
                    while let Some(left_id) = left {
                        work.visit(1)?;
                        if (chain_x_at_y(centerlines, &chains[left_id], cur_y) - xt).abs()
                            > COINCIDENT_EPS
                        {
                            break;
                        }
                        if !check_pair(
                            left_id,
                            id,
                            cur_y,
                            false,
                            &mut crossed,
                            &mut schedule,
                            &mut heap,
                            &mut cross_events,
                            work,
                        )? {
                            return Ok(None);
                        }
                        if shared_fan(at, ends[chains[left_id].edge], ends[id_edge], out) {
                            break;
                        }
                        left = active.prev_chain(left_id);
                    }
                }
                if let Some(r0) = right_neighbor
                    && (chain_x_at_y(centerlines, &chains[r0], cur_y) - xt).abs() <= COINCIDENT_EPS
                    && !shared_fan(at, ends[id_edge], ends[chains[r0].edge], out)
                {
                    let mut right = active.next_chain(r0);
                    while let Some(right_id) = right {
                        work.visit(1)?;
                        if (chain_x_at_y(centerlines, &chains[right_id], cur_y) - xt).abs()
                            > COINCIDENT_EPS
                        {
                            break;
                        }
                        if !check_pair(
                            id,
                            right_id,
                            cur_y,
                            false,
                            &mut crossed,
                            &mut schedule,
                            &mut heap,
                            &mut cross_events,
                            work,
                        )? {
                            return Ok(None);
                        }
                        if shared_fan(at, ends[id_edge], ends[chains[right_id].edge], out) {
                            break;
                        }
                        right = active.next_chain(right_id);
                    }
                }
                active.remove(id, work)?;
                if let (Some(l), Some(r)) = (left_neighbor, right_neighbor)
                    && !check_pair(
                        l,
                        r,
                        cur_y,
                        false,
                        &mut crossed,
                        &mut schedule,
                        &mut heap,
                        &mut cross_events,
                        work,
                    )?
                {
                    return Ok(None);
                }
            }
            EV_CROSS => {
                let a_id = ev.a();
                let b_id = ev.b as usize;
                if !active.is_adjacent(a_id, b_id) {
                    continue;
                }
                let ca = &chains[a_id];
                let cb = &chains[b_id];
                let y_bot = ca.y_bot.min(cb.y_bot);
                if y_bot <= cur_y {
                    continue;
                }
                let points_a = centerlines.points(ca.primitive);
                let points_b = centerlines.points(cb.primitive);
                let ka = chord_index_monotonic(points_a, ca, cur_y);
                let kb = chord_index_monotonic(points_b, cb, cur_y);
                let ord_a = ordered_ends_in(points_a, ca, ka);
                let ord_b = ordered_ends_in(points_b, cb, kb);
                let y_hi = ord_a[1].y.min(ord_b[1].y);
                if y_hi <= cur_y {
                    continue;
                }
                let d_lo = x_at_y(ord_a, cur_y) - x_at_y(ord_b, cur_y);
                let d_hi = x_at_y(ord_a, y_hi) - x_at_y(ord_b, y_hi);
                if d_hi < 0.0 || (d_hi == 0.0 && d_lo <= 0.0) {
                    continue;
                }

                // Swap `a` and `b` in `O(1)` (`b` moves left, `a` moves right).
                active.swap_adjacent(a_id, b_id);

                // Re-check `(b, a)` strictly below `cur_y` when either curve has multiple chords
                // so a double-crossing lens between two curves still triggers its second swap.
                if (ca.chord_count > 1 || cb.chord_count > 1)
                    && !check_pair(
                        b_id,
                        a_id,
                        cur_y,
                        true,
                        &mut crossed,
                        &mut schedule,
                        &mut heap,
                        &mut cross_events,
                        work,
                    )?
                {
                    return Ok(None);
                }
                if let Some(left_id) = active.prev_chain(b_id)
                    && !check_pair(
                        left_id,
                        b_id,
                        cur_y,
                        false,
                        &mut crossed,
                        &mut schedule,
                        &mut heap,
                        &mut cross_events,
                        work,
                    )?
                {
                    return Ok(None);
                }
                if let Some(right_id) = active.next_chain(a_id)
                    && !check_pair(
                        a_id,
                        right_id,
                        cur_y,
                        false,
                        &mut crossed,
                        &mut schedule,
                        &mut heap,
                        &mut cross_events,
                        work,
                    )?
                {
                    return Ok(None);
                }
            }
            _ => unreachable!(),
        }
    }

    Ok(Some(crossed))
}
