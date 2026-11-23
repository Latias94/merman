//! Route between occupied bands in canonical Down coordinates.

use super::{Component, Drawing, Expanded, Kind, acyclic::Orientation, canonical_size};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, Graph, LayoutError, Options, Point, PortSideHint, Rect,
    RoutePoint, RoutingStyle, Side, Size, WorkError,
};
use std::{cmp::Reverse, collections::BinaryHeap};

const LEAD: f64 = 6.0;
const LEVEL_CLEAR: f64 = 8.0;
const LEVEL_TOUCH: f64 = 4.0;
const SAMPLES: usize = 16;
const PRECISION: LayoutError = LayoutError::InvalidInput("route exceeds coordinate precision");

fn finite(value: f64) -> LayoutResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        Err(WorkError::ArithmeticOverflow.into())
    }
}

fn advance(start: f64, distance: f64) -> LayoutResult<f64> {
    let end = finite(start + distance)?;
    if distance > 0.0 && end <= start {
        return Err(PRECISION);
    }
    if end - start < distance {
        finite(end.next_up())
    } else {
        Ok(end)
    }
}

#[derive(Default)]
struct Band {
    logical: u64,
    height: f64,
    top: f64,
    bottom: f64,
    has_node: bool,
    leaves: bool,
    arrives: bool,
    label_directions: u8,
    label_count: usize,
    optical_shift: f64,
}

#[derive(Clone, Copy, Default)]
struct EndRoom {
    marker: f64,
    labels: f64,
}

impl EndRoom {
    fn include(
        &mut self,
        marker: f64,
        label: Option<Size>,
        dir: Direction,
        along: f64,
    ) -> LayoutResult<()> {
        self.marker = self.marker.max(marker);
        if let Some(size) = label {
            self.labels = advance(
                self.labels,
                finite(along + canonical_size(dir, size).height)?,
            )?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct Span {
    from: f64,
    to: f64,
}

impl Span {
    fn lo(self) -> f64 {
        self.from.min(self.to)
    }
    fn hi(self) -> f64 {
        self.from.max(self.to)
    }
    fn width(self) -> f64 {
        self.hi() - self.lo()
    }
    fn contains(self, x: f64) -> bool {
        self.lo() <= x && x <= self.hi()
    }
    fn mates(self, other: Self) -> bool {
        self.from == other.from || self.to == other.to
    }
    fn clashes(self, other: Self) -> bool {
        !self.mates(other) && self.lo().max(other.lo()) < self.hi().min(other.hi())
    }
    fn crossings_above(self, other: Self) -> u8 {
        u8::from(other.contains(self.to)) + u8::from(self.contains(other.from))
    }
}

struct Piece {
    span: Span,
    lower: usize,
    track: Option<usize>,
}

#[derive(Default)]
struct Gap {
    pieces: Vec<Piece>,
    cursor: usize,
    upper_guard: f64,
    lower_guard: f64,
    minimum: f64,
    height: f64,
    tracks: usize,
}

fn track_metrics(style: RoutingStyle, edge_sep: f64) -> (f64, f64, f64) {
    match style {
        RoutingStyle::Orthogonal => (18.0, edge_sep.max(10.0), edge_sep.max(14.0)),
        _ => (28.0, edge_sep.max(12.0), edge_sep.max(12.0)),
    }
}

fn bands(g: &Expanded, work: &mut Work<'_>) -> LayoutResult<(Vec<Band>, Vec<usize>)> {
    work.charge(g.layers.len())?;
    let mut bands = reserved(g.layers.len())?;
    let mut index = filled(g.layers.len(), usize::MAX)?;
    for (rank, layer) in g.layers.iter().enumerate() {
        let mut band = Band {
            logical: g.logical_rank[rank],
            ..Band::default()
        };
        let mut retained = false;
        for &vertex in layer {
            work.visit(1)?;
            let vertex = vertex as usize;
            band.height = band.height.max(g.height[vertex]);
            band.has_node |= g.kind[vertex] == Kind::Node;
            retained |= g.kind[vertex] != Kind::Dummy || g.keep[vertex];
        }
        if retained {
            index[rank] = bands.len();
            bands.push(band);
        }
    }
    Ok((bands, index))
}

struct RankedComponent<'a> {
    component: &'a Component,
    expanded: &'a Expanded,
    orientation: &'a Orientation,
}

fn collect_pieces(
    ranked: RankedComponent<'_>,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    bands: &mut [Band],
    index: &[usize],
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Gap>> {
    let g = ranked.expanded;
    let node_count = ranked.component.nodes.len();
    let gap_count = bands.len().saturating_sub(1);
    work.charge(gap_count)?;
    work.charge(node_count)?;
    let mut ends = filled(node_count, [EndRoom::default(); 2])?;
    let mut delta = filled(bands.len(), 0i64)?;
    let row = |v: usize| index[g.rank[v] as usize];
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if !g.direct[edge] {
            delta[row(chain[0] as usize)] += 1;
            delta[row(chain[chain.len() - 1] as usize)] -= 1;
        }
    }
    let mut gaps = reserved(gap_count)?;
    let mut crossing = 0i64;
    for (pair, change) in bands.windows(2).zip(delta) {
        crossing += change;
        let count = usize::try_from(crossing).map_err(|_| WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let distance = pair[1].logical - pair[0].logical;
        gaps.push(Gap {
            pieces: reserved(count)?,
            height: finite(distance as f64 * options.spacing.rank)?,
            ..Gap::default()
        });
    }
    let mut segment = 0;
    for (edge, link) in ranked.component.links.iter().enumerate() {
        work.visit(1)?;
        let chain = &g.chains[edge];
        let first = chain[0] as usize;
        let last = chain[chain.len() - 1] as usize;
        bands[row(first)].leaves = true;
        bands[row(last)].arrives = true;
        let spec = &graph.edges[link.edge as usize];
        let reversed = ranked.orientation.reversed[edge];
        let mut terminals = [
            (spec.start_marker.length, spec.start_label),
            (spec.end_marker.length, spec.end_label),
        ];
        if reversed {
            terminals.swap(0, 1);
        }
        for ((marker, label), (vertex, face)) in terminals.into_iter().zip([(first, 0), (last, 1)])
        {
            ends[vertex][face].include(
                marker,
                label,
                graph.direction,
                options.spacing.end_label_along,
            )?;
        }
        if let Some(label) = g.label_vertex[edge] {
            let band = &mut bands[row(label as usize)];
            band.label_directions |= if reversed { 2 } else { 1 };
            band.label_count += 1;
            let bias = (terminals[0].0 - terminals[1].0) * 0.5;
            band.optical_shift += (bias - band.optical_shift) / band.label_count as f64;
        }
        if g.direct[edge] {
            continue;
        }
        let mut upper = row(first);
        let mut from = finite(x[first] + g.source_offset[segment])?;
        for &lower in &chain[1..] {
            work.visit(1)?;
            let lower = lower as usize;
            let next = row(lower);
            if next != usize::MAX {
                let to = finite(x[lower] + g.target_offset[segment])?;
                let span = Span { from, to };
                finite(span.width())?;
                gaps[upper].pieces.push(Piece {
                    span,
                    lower,
                    track: None,
                });
                upper = next;
                if lower != last {
                    from = finite(x[lower] + g.source_offset[segment + 1])?;
                }
            }
            segment += 1;
        }
    }
    for (vertex, [outgoing, incoming]) in ends.into_iter().enumerate() {
        work.visit(1)?;
        let row = row(vertex);
        if let Some(gap) = gaps.get_mut(row) {
            gap.upper_guard = gap
                .upper_guard
                .max(finite(outgoing.marker + outgoing.labels)?);
        }
        if row > 0 {
            let gap = &mut gaps[row - 1];
            gap.lower_guard = gap
                .lower_guard
                .max(finite(incoming.marker + incoming.labels)?);
        }
    }
    Ok(gaps)
}

/// In left-endpoint order, all successors through the current right endpoint meet the
/// current interval. There is at most one additional comparison per interval.
fn meetings(
    pieces: &[Piece],
    by_left: &[usize],
    work: &mut Work<'_>,
    mut pair: impl FnMut(usize, usize, &mut Work<'_>) -> LayoutResult<()>,
) -> LayoutResult<()> {
    for (left, &id) in by_left.iter().enumerate() {
        work.visit(1)?;
        let limit = pieces[id].span.hi();
        for (right, &other) in by_left.iter().enumerate().skip(left + 1) {
            if pieces[other].span.lo() > limit {
                break;
            }
            work.visit(1)?;
            pair(left, right, work)?;
        }
    }
    Ok(())
}

fn sample(span: Span, height: f64) -> [Point; SAMPLES + 1] {
    let ease = (height / span.width().max(1.0) * 4.0).clamp(0.25, 0.5);
    std::array::from_fn(|i| {
        let t = i as f64 / SAMPLES as f64;
        let u = 1.0 - t;
        Point {
            x: span.from + (span.to - span.from) * (t * t * (3.0 - 2.0 * t)),
            y: height * (3.0 * ease * t * u * u + 3.0 * (1.0 - ease) * t * t * u + t * t * t),
        }
    })
}

fn reaches(points: &[Point], line: &[Point], limit: f64) -> bool {
    let mut first = 0;
    for p in points {
        while first + 1 < line.len() && p.y - line[first + 1].y >= limit {
            first += 1;
        }
        for segment in line[first..].windows(2) {
            let (a, b) = (segment[0], segment[1]);
            if a.y - p.y >= limit {
                break;
            }
            if a.x.min(b.x) - p.x >= limit || p.x - a.x.max(b.x) >= limit {
                continue;
            }
            let (dx, dy) = (b.x - a.x, b.y - a.y);
            let length = dx.hypot(dy);
            let (ux, uy) = if length > 0.0 {
                (dx / length, dy / length)
            } else {
                (0.0, 0.0)
            };
            let (px, py) = (p.x - a.x, p.y - a.y);
            let along = (px * ux + py * uy).clamp(0.0, length);
            if (px - along * ux).hypot(py - along * uy) < limit {
                return true;
            }
        }
    }
    false
}

fn curved_levels(
    pieces: &[Piece],
    ids: &[usize],
    height: f64,
    room: f64,
    promoted: &mut [bool],
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.charge(
        ids.len()
            .checked_mul(SAMPLES + 1)
            .ok_or(WorkError::ArithmeticOverflow)?,
    )?;
    let mut curves = reserved(ids.len())?;
    for &id in ids {
        curves.push(sample(pieces[id].span, room));
    }
    meetings(pieces, ids, work, |left, right, work| {
        let (a, b) = (pieces[ids[left]].span, pieces[ids[right]].span);
        if !a.clashes(b) {
            return Ok(());
        }
        let wide_a = a.width() * 0.5 > height;
        let wide_b = b.width() * 0.5 > height;
        promoted[left] |= wide_a;
        promoted[right] |= wide_b;
        if promoted[left] && promoted[right] || (a.from < a.to) != (b.from < b.to) {
            return Ok(());
        }
        let crossing = (a.from < b.from) != (a.to < b.to);
        let limit = if wide_a || wide_b {
            LEVEL_CLEAR
        } else {
            LEVEL_TOUCH
        };
        work.visit(2 * SAMPLES * (SAMPLES + 1))?;
        if crossing
            || reaches(&curves[left], &curves[right], limit)
            || reaches(&curves[right], &curves[left], limit)
        {
            promoted[left] = true;
            promoted[right] = true;
        }
        Ok(())
    })?;
    let (mut crossed, mut alongside) = (0u64, 0u64);
    meetings(pieces, ids, work, |left, right, _| {
        if promoted[left] && promoted[right] {
            let (a, b) = (pieces[ids[left]].span, pieces[ids[right]].span);
            if a.clashes(b) {
                if (a.from < b.from) != (a.to < b.to) {
                    crossed += 1;
                } else {
                    alongside += 1;
                }
            }
        }
        Ok(())
    })?;
    Ok(crossed > alongside)
}

/// Buckets are ordered by the number of unplaced predecessors, then by fan priority.
/// A piece can visit only waiting+1 slots, so all buckets together occupy O(K+I).
/// Packed summaries select the first occupied slot without rescanning the gap on cycles.
struct Waiting {
    base: Vec<usize>,
    slot: Vec<usize>,
    owner: Vec<usize>,
    bits: Vec<Vec<u64>>,
}

impl Waiting {
    fn new(counts: &[usize], priority: &[usize], work: &mut Work<'_>) -> LayoutResult<Self> {
        let mut cursor = filled(counts.iter().max().copied().unwrap_or(0) + 1, 0usize)?;
        let mut base = reserved(counts.len())?;
        let mut slots = 0usize;
        for &count in counts {
            base.push(slots);
            slots = slots
                .checked_add(count + 1)
                .ok_or(WorkError::ArithmeticOverflow)?;
            work.visit(count + 1)?;
            for size in &mut cursor[..=count] {
                *size += 1;
            }
        }
        work.charge(slots.checked_mul(2).ok_or(WorkError::ArithmeticOverflow)?)?;
        let mut end = 0;
        for size in &mut cursor {
            let start = end;
            end += *size;
            *size = start;
        }
        let mut slot = filled(slots, 0)?;
        let mut owner = filled(slots, 0)?;
        for &id in priority {
            work.visit(counts[id] + 1)?;
            for (degree, next) in cursor[..=counts[id]].iter_mut().enumerate() {
                slot[base[id] + degree] = *next;
                owner[*next] = id;
                *next += 1;
            }
        }
        let mut len = slots;
        let mut bits = Vec::new();
        loop {
            len = len.div_ceil(u64::BITS as usize);
            work.charge(len)?;
            push(&mut bits, filled(len, 0u64)?)?;
            if len <= 1 {
                break;
            }
        }
        let mut result = Self {
            base,
            slot,
            owner,
            bits,
        };
        for (id, &count) in counts.iter().enumerate() {
            result.mark(id, count, true);
        }
        Ok(result)
    }

    fn mark(&mut self, id: usize, degree: usize, present: bool) {
        let mut bit = self.slot[self.base[id] + degree];
        for level in &mut self.bits {
            let mask = 1u64 << (bit % 64);
            bit /= 64;
            let word = &mut level[bit];
            let empty = *word == 0;
            if present {
                *word |= mask;
            } else {
                *word &= !mask;
            }
            if empty == (*word == 0) {
                break;
            }
        }
    }

    fn first(&self) -> usize {
        let mut at = 0;
        for level in self.bits.iter().rev() {
            at = at * 64 + level[at].trailing_zeros() as usize;
        }
        self.owner[at]
    }
}

#[derive(Clone, Copy)]
struct Meeting {
    other: usize,
    above: bool,
    clash: bool,
}

fn weighed_tracks(pieces: &mut [Piece], ids: &[usize], work: &mut Work<'_>) -> LayoutResult<usize> {
    let count = ids.len();
    if count == 0 {
        return Ok(0);
    }
    if let [only] = ids {
        pieces[*only].track = Some(0);
        return Ok(1);
    }
    work.charge(count.checked_mul(5).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut neighbors = filled(count, Vec::new())?;
    let mut waiting = filled(count, 0usize)?;
    meetings(pieces, ids, work, |left, right, work| {
        let (a, b) = (pieces[ids[left]].span, pieces[ids[right]].span);
        if a.mates(b) {
            return Ok(());
        }
        let ab = a.crossings_above(b);
        let ba = b.crossings_above(a);
        let clash = a.clashes(b);
        if clash || ab != ba {
            work.visit(2)?;
            push(
                &mut neighbors[left],
                Meeting {
                    other: right,
                    above: ab < ba,
                    clash,
                },
            )?;
            push(
                &mut neighbors[right],
                Meeting {
                    other: left,
                    above: ba < ab,
                    clash,
                },
            )?;
            waiting[left] += usize::from(ba < ab);
            waiting[right] += usize::from(ab < ba);
        }
        Ok(())
    })?;
    let mut priority = reserved(count)?;
    priority.extend(0..count);
    work.sort(&mut priority, |&a, &b| {
        pieces[ids[b]]
            .span
            .width()
            .total_cmp(&pieces[ids[a]].span.width())
            .then(a.cmp(&b))
    })?;
    let mut ready = Waiting::new(&waiting, &priority, work)?;
    let mut floor = filled(count, 0usize)?;
    let mut taken = filled(count, usize::MAX)?;
    let mut rows = 0;
    for turn in 0..count {
        let current = ready.first();
        ready.mark(current, waiting[current], false);
        for contact in &neighbors[current] {
            work.visit(1)?;
            if contact.clash
                && let Some(track) = pieces[ids[contact.other]].track
            {
                taken[track] = turn;
            }
        }
        let mut track = floor[current];
        while taken[track] == turn {
            track += 1;
        }
        pieces[ids[current]].track = Some(track);
        rows = rows.max(track + 1);
        for &Meeting { other, above, .. } in &neighbors[current] {
            work.visit(1)?;
            if above && pieces[ids[other]].track.is_none() {
                floor[other] = floor[other].max(track + 1);
                ready.mark(other, waiting[other], false);
                waiting[other] -= 1;
                ready.mark(other, waiting[other], true);
            }
        }
    }
    Ok(rows)
}

fn greedy_tracks(
    pieces: &mut [Piece],
    ids: &[usize],
    curved: bool,
    work: &mut Work<'_>,
) -> LayoutResult<usize> {
    let count = ids.len();
    work.charge(count.checked_mul(7).ok_or(WorkError::ArithmeticOverflow)?)?;
    let mut permutation = reserved(count)?;
    permutation.extend(0..count);
    let mut owner = filled(count, usize::MAX)?;
    for source in [true, false] {
        let contact = |i: usize| {
            let span = pieces[ids[i]].span;
            if source { span.from } else { span.to }
        };
        if !source {
            permutation.retain(|&i| owner[i] == usize::MAX);
        }
        work.sort(&mut permutation, |&a, &b| {
            contact(a).total_cmp(&contact(b)).then(a.cmp(&b))
        })?;
        for bus in permutation.chunk_by(|&a, &b| contact(a) == contact(b)) {
            work.visit(bus.len())?;
            if bus.len() > 1 || !source {
                for &i in bus {
                    owner[i] = bus[0];
                }
            }
        }
    }
    let mut bounds = filled(count, (f64::INFINITY, f64::NEG_INFINITY))?;
    permutation.clear();
    for (i, &bus) in owner.iter().enumerate() {
        let span = pieces[ids[i]].span;
        bounds[bus] = (bounds[bus].0.min(span.lo()), bounds[bus].1.max(span.hi()));
        if i == bus {
            permutation.push(bus);
        }
    }
    work.sort(&mut permutation, |&a, &b| {
        bounds[a].0.total_cmp(&bounds[b].0).then(a.cmp(&b))
    })?;
    if curved {
        let mut reach = f64::NEG_INFINITY;
        let mut kept = 0;
        for i in 0..permutation.len() {
            let bus = permutation[i];
            let overlaps = bounds[bus].0 < reach
                || permutation
                    .get(i + 1)
                    .is_some_and(|&next| bounds[next].0 < bounds[bus].1);
            reach = reach.max(bounds[bus].1);
            if overlaps {
                permutation[kept] = bus;
                kept += 1;
            }
        }
        permutation.truncate(kept);
    }
    let mut ends = reserved(permutation.len())?;
    ends.extend_from_slice(&permutation);
    work.sort(&mut ends, |&a, &b| {
        bounds[a].1.total_cmp(&bounds[b].1).then(a.cmp(&b))
    })?;
    let mut free = BinaryHeap::new();
    free.try_reserve(ends.len())
        .map_err(|_| LayoutError::AllocationLimit)?;
    let mut track = filled(count, None)?;
    let mut released = 0;
    let mut rows = 0;
    let heap_cost = count.ilog2() as usize + 1;
    for bus in permutation {
        work.visit(heap_cost)?;
        while released < ends.len() && bounds[ends[released]].1 <= bounds[bus].0 {
            work.visit(heap_cost)?;
            free.push(Reverse(
                track[ends[released]].expect("a nonempty bus has already started"),
            ));
            released += 1;
        }
        track[bus] = Some(free.pop().map_or_else(
            || {
                rows += 1;
                rows - 1
            },
            |Reverse(row)| row,
        ));
    }
    for (i, &bus) in owner.iter().enumerate() {
        pieces[ids[i]].track = track[bus];
    }
    Ok(rows)
}

impl Gap {
    fn grade(&mut self, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
        work.charge(self.pieces.len())?;
        let mut ids = reserved(self.pieces.len())?;
        ids.extend(
            self.pieces
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.span.from != p.span.to).then_some(i)),
        );
        if !ids.is_empty() {
            let lead = LEAD.max(options.spacing.edge * 0.5);
            self.upper_guard = advance(self.upper_guard, lead)?;
            self.lower_guard = advance(self.lower_guard, lead)?;
        }
        let guard = finite(self.upper_guard + self.lower_guard)?;
        self.minimum = finite(guard + if ids.is_empty() { 0.0 } else { 2.0 * LEAD })?;
        self.height = self.height.max(self.minimum);
        if ids.is_empty() || options.routing == RoutingStyle::Polyline {
            return Ok(());
        }
        let curved = options.routing == RoutingStyle::Curved;
        if curved && ids.len() == 1 {
            return Ok(());
        }
        if ids.len() > options.caps.max_weighed_gap_pieces {
            self.tracks = greedy_tracks(&mut self.pieces, &ids, curved, work)?;
        } else {
            work.sort(&mut ids, |&a, &b| {
                self.pieces[a]
                    .span
                    .lo()
                    .total_cmp(&self.pieces[b].span.lo())
                    .then(a.cmp(&b))
            })?;
            if curved {
                let mut promoted = filled(ids.len(), false)?;
                if curved_levels(
                    &self.pieces,
                    &ids,
                    self.height,
                    self.height - guard,
                    &mut promoted,
                    work,
                )? {
                    let widest = ids
                        .iter()
                        .map(|&i| self.pieces[i].span.width())
                        .fold(0.0, f64::max);
                    self.minimum = self
                        .minimum
                        .max((3.0 * options.spacing.rank).min(widest * 0.5));
                    self.height = self.height.max(self.minimum);
                    return Ok(());
                }
                let mut flags = promoted.into_iter();
                ids.retain(|_| flags.next() == Some(true));
            }
            self.tracks = weighed_tracks(&mut self.pieces, &ids, work)?;
        }
        if self.tracks > 0 {
            let (margin, pitch, _) = track_metrics(options.routing, options.spacing.edge);
            self.minimum = self.minimum.max(finite(
                guard + 2.0 * margin + (self.tracks - 1) as f64 * pitch,
            )?);
            self.height = self.height.max(self.minimum);
        }
        Ok(())
    }
}

fn place_bands(
    bands: &mut [Band],
    gaps: &mut [Gap],
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    for gap in gaps.iter_mut() {
        gap.grade(options, work)?;
    }
    for row in 1..gaps.len() {
        work.visit(1)?;
        let band = &bands[row];
        if band.has_node || !matches!(band.label_directions, 1 | 2) {
            continue;
        }
        let (before, after) = gaps.split_at_mut(row);
        let above = &mut before[row - 1];
        let below = &mut after[0];
        let shift = band
            .optical_shift
            .clamp(above.minimum - above.height, below.height - below.minimum);
        above.height = finite(above.height + shift)?.max(above.minimum);
        below.height = finite(below.height - shift)?.max(below.minimum);
    }
    let mut top = 0.0;
    for (row, band) in bands.iter_mut().enumerate() {
        work.visit(1)?;
        band.top = top;
        band.bottom = advance(top, band.height)?;
        if let Some(gap) = gaps.get(row) {
            top = advance(band.bottom, gap.height)?;
        }
    }
    Ok(())
}

fn knot(points: &mut Vec<RoutePoint>, x: f64, y: f64, axis: Axis) {
    if !points.last().is_some_and(|p| p.x == x && p.y == y) {
        points.push(RoutePoint { x, y, axis });
    }
}

/// For a curved shelf the short S at either end pins the following rounded elbow inside
/// this gap, even when a painter coalesces collinear knots across the adjacent band.
fn draw_piece(
    points: &mut Vec<RoutePoint>,
    piece: &Piece,
    upper: &Band,
    lower: &Band,
    gap: &Gap,
    options: &Options,
) -> LayoutResult<()> {
    let Span { from, to } = piece.span;
    if from == to {
        return Ok(());
    }
    let start = advance(upper.bottom, gap.upper_guard)?;
    let end = -advance(-lower.top, gap.lower_guard)?;
    if end <= start {
        return Err(PRECISION);
    }
    knot(points, from, start, Axis::Vertical);
    if let Some(track) = piece.track {
        let (margin, _, spread) = track_metrics(options.routing, options.spacing.edge);
        let steps = (gap.tracks - 1) as f64;
        let pitch = if steps > 0.0 {
            spread.min((end - start - 2.0 * margin) / steps)
        } else {
            0.0
        };
        let midpoint = start.midpoint(end);
        let level = |track: usize| midpoint + (track as f64 - steps * 0.5) * pitch;
        let shelf = finite(level(track))?;
        if shelf <= start || shelf >= end || track > 0 && level(track - 1) >= shelf {
            return Err(PRECISION);
        }
        if options.routing == RoutingStyle::Curved {
            let shoulder = (shelf - start)
                .min(end - shelf)
                .min(piece.span.width() * 0.25)
                * 0.5;
            let across = shoulder.copysign(to - from);
            let left = from + across;
            let right = to - across;
            if left != from && right != to && start + shoulder > start && end - shoulder < end {
                knot(points, left, start + shoulder, Axis::Vertical);
                knot(points, left, shelf, Axis::Vertical);
                knot(points, right, shelf, Axis::Horizontal);
                knot(points, right, end - shoulder, Axis::Vertical);
            }
        } else {
            knot(points, from, shelf, Axis::Vertical);
            knot(points, to, shelf, Axis::Horizontal);
        }
    }
    knot(points, to, end, Axis::Vertical);
    Ok(())
}

fn reverse_route(points: &mut [RoutePoint]) {
    points.reverse();
    for i in (1..points.len()).rev() {
        points[i].axis = points[i - 1].axis;
    }
    points[0].axis = points[1].axis;
}

fn direct_port(rect: Rect, hint: PortSideHint, dir: Direction, side: Side) -> f64 {
    let offset = match super::to_canonical_port(dir, hint) {
        PortSideHint::FixedOffset {
            side: fixed,
            offset,
        } if fixed == side => offset,
        _ => 0.0,
    };
    (rect.center().x + offset).clamp(rect.x, rect.x + rect.width)
}

pub(super) fn route(
    g: &Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    work: &mut Work<'_>,
) -> LayoutResult<Drawing> {
    work.check()?;
    if g.keep.len() != g.rank.len() || x.len() != g.rank.len() {
        return Err(LayoutError::InvalidInput("incomplete refined vertices"));
    }
    let (mut bands, index) = bands(g, work)?;
    let ranked = RankedComponent {
        component: c,
        expanded: g,
        orientation,
    };
    let mut gaps = collect_pieces(ranked, graph, options, x, &mut bands, &index, work)?;
    place_bands(&mut bands, &mut gaps, options, work)?;
    work.charge(c.nodes.len())?;
    let mut nodes = reserved(c.nodes.len())?;
    let row = |vertex: usize| index[g.rank[vertex] as usize];
    for (id, &center) in x[..c.nodes.len()].iter().enumerate() {
        work.visit(1)?;
        let band = &bands[row(id)];
        let height = g.height[id];
        let width = g.width[id];
        let y = match (band.leaves, band.arrives) {
            (true, false) => band.bottom - height,
            (false, true) => band.top,
            _ => band.top + (band.height - height) * 0.5,
        };
        let left = finite(center - width * 0.5)?;
        let middle = y + height * 0.5;
        if (width > 0.0 && !(left < center && center < finite(left + width)?))
            || (height > 0.0 && !(y < middle && middle < finite(y + height)?))
        {
            return Err(PRECISION);
        }
        nodes.push(Rect {
            x: left,
            y,
            width,
            height,
        });
    }
    work.charge(c.links.len())?;
    let mut edges = reserved(c.links.len())?;
    for (edge, link) in c.links.iter().enumerate() {
        work.visit(1)?;
        let reversed = orientation.reversed[edge];
        let (first, last) = if reversed {
            (link.to, link.from)
        } else {
            (link.from, link.to)
        };
        let (first, last) = (first as usize, last as usize);
        let (a, b) = (row(first), row(last));
        let spec = &graph.edges[link.edge as usize];
        let direct = g.direct[edge];
        let mut capacity = 2usize;
        if !direct {
            for gap in &gaps[a..b] {
                work.visit(1)?;
                let piece = &gap.pieces[gap.cursor];
                let bends = if piece.span.from == piece.span.to {
                    0
                } else if piece.track.is_none() {
                    2
                } else if options.routing == RoutingStyle::Curved {
                    6
                } else {
                    4
                };
                let guards = if piece.lower == last {
                    0
                } else if bands[row(piece.lower)].height == 0.0 {
                    1
                } else {
                    2 + usize::from(g.keep[piece.lower])
                };
                capacity = capacity
                    .checked_add(bends + guards)
                    .ok_or(WorkError::ArithmeticOverflow)?;
            }
        }
        work.charge(capacity)?;
        let mut points = reserved(capacity)?;
        let (source_x, target_x) = if direct {
            let (s, t) = if reversed {
                (spec.target_port, spec.source_port)
            } else {
                (spec.source_port, spec.target_port)
            };
            (
                direct_port(nodes[first], s, graph.direction, Side::Bottom),
                direct_port(nodes[last], t, graph.direction, Side::Top),
            )
        } else {
            (
                gaps[a].pieces[gaps[a].cursor].span.from,
                gaps[b - 1].pieces[gaps[b - 1].cursor].span.to,
            )
        };
        points.push(RoutePoint {
            x: source_x,
            y: nodes[first].y + nodes[first].height,
            axis: Axis::Vertical,
        });
        if !direct {
            for level in a..b {
                work.visit(1)?;
                let gap = &mut gaps[level];
                let piece = &gap.pieces[gap.cursor];
                draw_piece(
                    &mut points,
                    piece,
                    &bands[level],
                    &bands[level + 1],
                    gap,
                    options,
                )?;
                if level + 1 < b {
                    let lower = &bands[level + 1];
                    knot(&mut points, piece.span.to, lower.top, Axis::Vertical);
                    if g.keep[piece.lower] {
                        knot(
                            &mut points,
                            piece.span.to,
                            lower.top.midpoint(lower.bottom),
                            Axis::Vertical,
                        );
                    }
                    knot(&mut points, piece.span.to, lower.bottom, Axis::Vertical);
                }
                gap.cursor += 1;
            }
        }
        points.push(RoutePoint {
            x: target_x,
            y: nodes[last].y,
            axis: Axis::Vertical,
        });
        let label_center = g.label_vertex[edge].map(|label| {
            let band = &bands[row(label as usize)];
            Point {
                x: x[label as usize],
                y: band.top.midpoint(band.bottom),
            }
        });
        if reversed {
            reverse_route(&mut points);
        }
        edges.push((
            link.edge,
            EdgeGeometry {
                points,
                ports: if reversed {
                    (Side::Top, Side::Bottom)
                } else {
                    (Side::Bottom, Side::Top)
                },
                label_center,
                start_label_center: None,
                end_label_center: None,
                reversed_for_ranking: reversed,
            },
        ));
    }
    Ok(Drawing { nodes, edges })
}

#[cfg(test)]
mod tests;
