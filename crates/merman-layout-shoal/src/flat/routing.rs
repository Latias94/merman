//! Route between occupied bands in canonical Down coordinates.

mod compact;
pub(super) mod decision;
mod exterior;

pub(super) use compact::compact_ranks;
use exterior::SidePort;
pub(super) use exterior::side_docking_allowed;

use super::{
    Component, Drawing, Expanded, Kind, acyclic::Orientation, canonical_size, contour_inset,
};
use crate::scratch::{capacity_bytes, clear_reserve};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, EdgeGeometry, Graph, KnotKind, LayoutError, MIN_MARKER_LEAD as LEAD,
    MarkerMetrics, Options, Point, PortSideHint, Rect, RoutePoint, RoutingStyle, ShapeContourHint,
    Side, Size, WorkError, sweep_handle_fraction,
};
use std::{cmp::Reverse, collections::BinaryHeap};

const LEVEL_CLEAR: f64 = 8.0;
const LEVEL_TOUCH: f64 = 4.0;
const SAMPLES: usize = 16;
const MAX_SWEEP_GAPS: usize = 12;
const MAX_SWEEP_PEERS: usize = 32;
const MAX_SWEEP_TRIALS: usize = 3;
const SWEEP_INVERSE_STEPS: usize = 12;
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

#[derive(Debug, Default)]
struct Band {
    logical: u64,
    height: f64,
    top: f64,
    bottom: f64,
    has_node: bool,
    has_compound_seam: bool,
    leaves: bool,
    arrives: bool,
    label_directions: u8,
    label_count: usize,
    inserted_label_tier: bool,
    optical_shift: f64,
    face_separation: f64,
}

#[derive(Clone, Copy, Debug, Default)]
struct EndRoom {
    marker: f64,
    runway: f64,
    marker_half_width: f64,
    labels: f64,
    has_label: bool,
    horizon: Option<usize>,
}

impl EndRoom {
    fn include(
        &mut self,
        marker: MarkerMetrics,
        runway: f64,
        label: Option<Size>,
        dir: Direction,
        along: f64,
    ) -> LayoutResult<()> {
        self.marker = self.marker.max(marker.length);
        self.runway = self.runway.max(runway);
        self.marker_half_width = self.marker_half_width.max(marker.width * 0.5);
        if let Some(size) = label {
            self.has_label = true;
            self.labels = advance(
                self.labels,
                finite(along + canonical_size(dir, size).height)?,
            )?;
        }
        Ok(())
    }

    fn reach(self) -> LayoutResult<f64> {
        finite(self.runway)
    }

    fn guard(self, curved: bool) -> LayoutResult<f64> {
        let runway = if curved { self.reach()? } else { self.marker };
        finite(runway + self.labels)
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
    fn hull(self, other: Self) -> Self {
        Self {
            from: self.lo().min(other.lo()),
            to: self.hi().max(other.hi()),
        }
    }
}

struct Piece {
    span: Span,
    occupied: Span,
    leads: [f64; 2],
    horizon: Option<usize>,
    keep_landing_runway: bool,
    lower: usize,
    track: Option<usize>,
}

#[derive(Default)]
struct Gap {
    pieces: Vec<Piece>,
    cursor: usize,
    upper_guard: f64,
    lower_guard: f64,
    marker_half_width: f64,
    has_end_labels: bool,
    allow_local_sweeps: bool,
    minimum: f64,
    height: f64,
    tracks: usize,
}

fn track_metrics(style: RoutingStyle, edge_sep: f64) -> (f64, f64, f64) {
    match style {
        RoutingStyle::Orthogonal => (18.0, edge_sep, edge_sep),
        _ => (28.0, edge_sep, edge_sep),
    }
}

#[derive(Debug, Default)]
pub(super) struct RoutingScratch {
    bands: Vec<Band>,
    index: Vec<usize>,
    ends: Vec<[EndRoom; 2]>,
    delta: Vec<i64>,
    rooms: Vec<Option<(f64, f64)>>,
    ids: Vec<usize>,
}

impl RoutingScratch {
    pub(super) fn retained_bytes(&self) -> usize {
        capacity_bytes(&self.bands)
            + capacity_bytes(&self.index)
            + capacity_bytes(&self.ends)
            + capacity_bytes(&self.delta)
            + capacity_bytes(&self.rooms)
            + capacity_bytes(&self.ids)
    }
}

fn bands(
    g: &Expanded,
    c: &Component,
    bands: &mut Vec<Band>,
    index: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    work.charge(g.layers.len())?;
    clear_reserve(bands, g.layers.len())?;
    clear_reserve(index, g.layers.len())?;
    index.resize(g.layers.len(), usize::MAX);
    let mut inserted = g.inserted_ranks.iter().copied().peekable();
    for (rank, layer) in g.layers.iter().enumerate() {
        let mut band = Band {
            logical: g.logical_rank[rank],
            ..Band::default()
        };
        let mut retained = false;
        let inserted_between_nodes = inserted.next_if_eq(&(rank as u32)).is_some();
        for &vertex in layer {
            work.visit(1)?;
            let vertex = vertex as usize;
            band.height = band.height.max(g.height[vertex]);
            let kind = g.kind[vertex];
            band.has_node |= kind == Kind::Node;
            band.has_compound_seam |= matches!(kind, Kind::StartPort | Kind::EndPort)
                || (kind == Kind::Node
                    && c.nodes.get(vertex).is_some_and(|node| node.group_proxy));
            retained |= kind != Kind::Dummy || g.keep[vertex];
            band.inserted_label_tier |= inserted_between_nodes && kind == Kind::Label;
        }
        if retained {
            index[rank] = bands.len();
            bands.push(band);
        }
    }
    Ok(())
}

struct RankedComponent<'a> {
    component: &'a Component,
    expanded: &'a Expanded,
    orientation: &'a Orientation,
    side_ports: &'a [[Option<SidePort>; 2]],
}

fn collect_pieces(
    ranked: &RankedComponent<'_>,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    scratch: &mut RoutingScratch,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Gap>> {
    let RoutingScratch {
        bands,
        index,
        ends,
        delta,
        ..
    } = scratch;
    let g = ranked.expanded;
    let node_count = ranked.component.nodes.len();
    let gap_count = bands.len().saturating_sub(1);
    work.charge(gap_count)?;
    work.charge(node_count)?;
    clear_reserve(ends, node_count)?;
    ends.resize(node_count, [EndRoom::default(); 2]);
    clear_reserve(delta, bands.len())?;
    delta.resize(bands.len(), 0);
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
    // Flat gap intervals see neither the approach knots of a compound border stitch nor
    // routes that bypass the expansion. Apply the shared runway only to gaps adjacent to
    // compound boundaries or group proxies so interior cluster gaps keep clean local sweeps.
    let can_local_sweep = options.routing == RoutingStyle::Curved && !g.direct.contains(&true);
    for (pair, &change) in bands.windows(2).zip(delta.iter()) {
        crossing += change;
        let count = usize::try_from(crossing).map_err(|_| WorkError::ArithmeticOverflow)?;
        work.charge(count)?;
        let distance = pair[1].logical - pair[0].logical;
        // A short edge's inserted label tier shares the original gap with its two
        // neighbors. Gap::grade still reserves the space the actual routes require.
        let ranks = if distance == 1 && (pair[0].inserted_label_tier || pair[1].inserted_label_tier)
        {
            0.5
        } else {
            distance as f64
        };
        gaps.push(Gap {
            pieces: reserved(count)?,
            height: finite(ranks * options.spacing.rank)?,
            allow_local_sweeps: can_local_sweep
                && !pair[0].has_compound_seam
                && !pair[1].has_compound_seam,
            ..Gap::default()
        });
    }
    if ranked.component.boundaries && !gaps.is_empty() {
        let start = g.kind[g.layers[0][0] as usize] == Kind::StartPort;
        let end = g.kind[g.layers.last().unwrap()[0] as usize] == Kind::EndPort;
        for (gap, outer) in [(0, start), (gap_count - 1, end)] {
            if outer {
                let distance = bands[gap + 1].logical - bands[gap].logical;
                let ports = if gap_count == 1 && start && end {
                    distance.min(2)
                } else {
                    1
                };
                gaps[gap].height = finite(
                    (distance - ports) as f64 * options.spacing.rank
                        + ports as f64 * options.spacing.port_sep,
                )?;
            }
        }
    }
    let mut segment = 0;
    for (edge, link) in ranked.component.links.iter().enumerate() {
        work.visit(1)?;
        let chain = &g.chains[edge];
        let first = chain[0] as usize;
        let last = chain[chain.len() - 1] as usize;
        let [side_start, side_end] = ranked.side_ports.get(edge).copied().unwrap_or_default();
        bands[row(first)].leaves = true;
        bands[row(last)].arrives = true;
        let spec = &graph.edges[link.edge as usize];
        let reversed = ranked.orientation.reversed[edge];
        let [start_runway, end_runway] = spec.sweep_runways();
        let mut terminals = [
            (spec.start_marker, start_runway, spec.start_label),
            (spec.end_marker, end_runway, spec.end_label),
        ];
        if reversed {
            terminals.swap(0, 1);
        }
        for ((marker, runway, label), (vertex, face, side)) in terminals
            .into_iter()
            .zip([(first, 0, side_start), (last, 1, side_end)])
        {
            if side.is_none() {
                let room = &mut ends[vertex][face];
                room.include(
                    marker,
                    runway,
                    label,
                    graph.direction,
                    options.spacing.end_label_along,
                )?;
                if face == 0 && !g.direct[edge] {
                    // Stop sibling departure sweeps at the first physical barrier (such as an
                    // inserted label band or kept waypoint), not only the destination node.
                    let stop = chain[1..]
                        .iter()
                        .copied()
                        .find(|&v| {
                            let v = v as usize;
                            row(v) != usize::MAX && (g.kind[v] != Kind::Dummy || g.keep[v])
                        })
                        .map_or(last, |v| v as usize);
                    let horizon = row(stop);
                    room.horizon = Some(room.horizon.map_or(horizon, |old| old.min(horizon)));
                }
            }
        }
        if let Some(label) = g.label_vertex[edge] {
            let band = &mut bands[row(label as usize)];
            band.label_directions |= if reversed { 2 } else { 1 };
            band.label_count += 1;
            let mut visible = terminals.map(|(marker, _, _)| marker.length);
            if !g.direct[edge] {
                let target = segment + chain.len() - 2;
                let nodes = &ranked.component.nodes;
                if side_start.is_none() {
                    visible[0] -= contour_inset(
                        nodes[first],
                        graph.direction,
                        Side::Bottom,
                        g.source_offset[segment],
                    );
                }
                if side_end.is_none() {
                    visible[1] -= contour_inset(
                        nodes[last],
                        graph.direction,
                        Side::Top,
                        g.target_offset[target],
                    );
                }
            }
            let bias = visible[0] * 0.5 - visible[1] * 0.5;
            band.optical_shift += (bias - band.optical_shift) / band.label_count as f64;
        }
        if !g.direct[edge] {
            segment += chain.len() - 1;
        }
    }
    segment = 0;
    for (edge, chain) in g.chains.iter().enumerate() {
        work.visit(1)?;
        if g.direct[edge] {
            continue;
        }
        let first = chain[0] as usize;
        let last = chain[chain.len() - 1] as usize;
        let [side_start, side_end] = ranked.side_ports.get(edge).copied().unwrap_or_default();
        let mut upper = row(first);
        let mut start_lead = if side_start.is_none() {
            ends[first][0].reach()?
        } else {
            0.0
        };
        let mut from =
            finite(side_start.map_or(x[first] + g.source_offset[segment], |port| port.rail))?;
        for &lower in &chain[1..] {
            work.visit(1)?;
            let lower = lower as usize;
            let next = row(lower);
            if next != usize::MAX {
                let to = if lower == last
                    && let Some(port) = side_end
                {
                    port.rail
                } else {
                    finite(x[lower] + g.target_offset[segment])?
                };
                let span = Span { from, to };
                finite(span.width())?;
                let horizon = (upper == row(first) && side_start.is_none() && from != to)
                    .then_some(ends[first][0].horizon)
                    .flatten();
                gaps[upper].pieces.push(Piece {
                    span,
                    occupied: span,
                    leads: [
                        start_lead,
                        if lower == last && side_end.is_none() {
                            ends[last][1].reach()?
                        } else {
                            0.0
                        },
                    ],
                    horizon,
                    keep_landing_runway: horizon == Some(next) && lower != last,
                    lower,
                    track: None,
                });
                start_lead = 0.0;
                upper = next;
                if lower != last {
                    from = finite(x[lower] + g.source_offset[segment + 1])?;
                }
            }
            segment += 1;
        }
    }
    let curved = options.routing == RoutingStyle::Curved;
    for (vertex, &[outgoing, incoming]) in ends.iter().enumerate() {
        work.visit(1)?;
        let row = row(vertex);
        if let Some(gap) = gaps.get_mut(row) {
            gap.upper_guard = gap.upper_guard.max(outgoing.guard(curved)?);
            gap.include_terminals(outgoing);
        }
        if row > 0 {
            let gap = &mut gaps[row - 1];
            gap.lower_guard = gap.lower_guard.max(incoming.guard(curved)?);
            gap.include_terminals(incoming);
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

fn sweep_x(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn sweep_y(t: f64, ease: f64) -> f64 {
    let u = 1.0 - t;
    3.0 * ease * t * u * u + 3.0 * (1.0 - ease) * t * t * u + t * t * t
}

fn sample(span: Span, height: f64) -> [Point; SAMPLES + 1] {
    let ease = sweep_handle_fraction(span.width(), height);
    std::array::from_fn(|i| {
        let t = i as f64 / SAMPLES as f64;
        Point {
            x: span.from + (span.to - span.from) * sweep_x(t),
            y: height * sweep_y(t, ease),
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

fn greedy_tracks(pieces: &mut [Piece], ids: &[usize], work: &mut Work<'_>) -> LayoutResult<usize> {
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
    fn include_terminals(&mut self, room: EndRoom) {
        self.marker_half_width = self.marker_half_width.max(room.marker_half_width);
        self.has_end_labels |= room.has_label;
    }

    fn next_piece(&self) -> &Piece {
        &self.pieces[self.cursor]
    }

    fn clear_of_peers(&self, span: Span, sep: f64, work: &mut Work<'_>) -> LayoutResult<bool> {
        let margin = sep.max(self.marker_half_width);
        for (i, peer) in self.pieces.iter().enumerate() {
            work.visit(1)?;
            if i != self.cursor
                && span.lo() - peer.occupied.hi() < margin
                && peer.occupied.lo() - span.hi() < margin
            {
                return Ok(false);
            }
        }
        Ok(true)
    }

    #[cfg(test)]
    fn grade(&mut self, options: &Options, work: &mut Work<'_>) -> LayoutResult<()> {
        self.grade_with_scratch(options, &mut Vec::new(), work)
    }

    fn grade_with_scratch(
        &mut self,
        options: &Options,
        ids: &mut Vec<usize>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        work.charge(self.pieces.len())?;
        clear_reserve(ids, self.pieces.len())?;
        ids.extend(
            self.pieces
                .iter()
                .enumerate()
                .filter_map(|(i, p)| (p.span.from != p.span.to).then_some(i)),
        );
        let curved = options.routing == RoutingStyle::Curved;
        let dense_curves = curved && ids.len() > MAX_SWEEP_PEERS;
        let isolated_curve = curved && self.pieces.len() == 1 && ids.len() == 1;
        if !ids.is_empty() && (!curved || !self.allow_local_sweeps && !isolated_curve) {
            let lead = LEAD.max(options.spacing.edge * 0.5);
            self.upper_guard = advance(self.upper_guard, lead)?;
            self.lower_guard = advance(self.lower_guard, lead)?;
        } else if dense_curves {
            // On a shallow, wide fan the sweep otherwise reaches neighboring boxes
            // while its stroke is still inside a rank's outline. Start it beyond the rank.
            let clearance = options.spacing.edge * 0.25;
            self.upper_guard = self.upper_guard.max(clearance);
            self.lower_guard = self.lower_guard.max(clearance);
        }
        let guard = finite(self.upper_guard + self.lower_guard)?;
        self.minimum = finite(guard + if ids.is_empty() { 0.0 } else { 2.0 * LEAD })?;
        if curved && !ids.is_empty() {
            let widest = ids
                .iter()
                .map(|&i| self.pieces[i].span.width())
                .fold(0.0, f64::max);
            // Guarantee enough net vertical room for wide curved sweeps when guards or
            // inserted-label slits squeeze a gap below healthy curvature, without inflating
            // ordinary flat rank gaps or extreme synthetic spans.
            let min_room = (widest * 0.35).min(options.spacing.rank * 0.75);
            let mut target = finite(guard + min_room)?.min(options.spacing.rank);
            let nominal_room = (self.height - guard).max(0.0);
            if !self.allow_local_sweeps
                && widest > nominal_room * 4.0
                && widest <= options.spacing.rank * 8.0
            {
                let wide_room = (widest / 3.2).min(options.spacing.rank * 1.15);
                target = target.max(finite(guard + wide_room)?);
            }
            self.minimum = self.minimum.max(target);
        }
        self.height = self.height.max(self.minimum);
        if isolated_curve || ids.is_empty() || options.routing == RoutingStyle::Polyline {
            return Ok(());
        }
        if curved && ids.len() == 1 {
            return Ok(());
        }
        if ids.len() > options.caps.max_weighed_gap_pieces {
            if curved {
                return Ok(());
            }
            self.tracks = greedy_tracks(&mut self.pieces, ids, work)?;
        } else {
            work.sort(ids, |&a, &b| {
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
                    ids,
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
            self.tracks = weighed_tracks(&mut self.pieces, ids, work)?;
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
    ids: &mut Vec<usize>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    for gap in gaps.iter_mut() {
        gap.grade_with_scratch(options, ids, work)?;
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

fn sweep_clearance(
    g: &Expanded,
    c: &Component,
    options: &Options,
    x: &[f64],
    index: &[usize],
    rooms: &mut Vec<Option<(f64, f64)>>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    rooms.clear();
    let half_sep = options.spacing.edge * 0.5;
    for (layer, &physical) in g.layers.iter().zip(index) {
        work.visit(1)?;
        if physical == usize::MAX {
            continue;
        }
        for (slot, &vertex) in layer.iter().enumerate() {
            work.visit(1)?;
            let v = vertex as usize;
            if g.kind[v] != Kind::Dummy || g.keep[v] {
                continue;
            }
            if rooms.is_empty() {
                work.charge(x.len())?;
                clear_reserve(rooms, x.len())?;
                rooms.resize(x.len(), None);
            }
            let neighbor = |peer: u32, direction: f64| {
                let u = peer as usize;
                if g.kind[u] == Kind::Dummy {
                    let midpoint = x[u].midpoint(x[v]);
                    let pad = (x[u] - midpoint).abs().min(half_sep);
                    midpoint + direction * pad
                } else {
                    let (left, right) = super::position::extents(g, c, options, peer);
                    x[u] + direction * (if direction > 0.0 { right } else { left } + half_sep)
                }
            };
            let lo = slot
                .checked_sub(1)
                .map_or(f64::NEG_INFINITY, |left| neighbor(layer[left], 1.0));
            let hi = layer
                .get(slot + 1)
                .map_or(f64::INFINITY, |&right| neighbor(right, -1.0));
            rooms[v] = Some((lo, hi));
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Sweep {
    span: Span,
    start: f64,
    end: f64,
    ease: f64,
}

impl Sweep {
    fn at(self, y: f64, work: &mut Work<'_>) -> LayoutResult<Span> {
        let py = ((y - self.start) / (self.end - self.start)).clamp(0.0, 1.0);
        let x = |unit| self.span.from + (self.span.to - self.span.from) * unit;
        if py == 0.0 || py == 1.0 {
            return Ok(Span {
                from: x(py),
                to: x(py),
            });
        }
        work.visit(SWEEP_INVERSE_STEPS)?;
        let (mut low, mut high) = (0.0_f64, 1.0_f64);
        for _ in 0..SWEEP_INVERSE_STEPS {
            let mid = low.midpoint(high);
            if sweep_y(mid, self.ease) < py {
                low = mid;
            } else {
                high = mid;
            }
        }
        let curved = Span {
            from: x(sweep_x(low)),
            to: x(sweep_x(high)),
        };
        Ok(curved.hull(Span {
            from: x(py),
            to: x(py),
        }))
    }
}

struct SuperGap {
    sweep: Sweep,
    occupied: [Span; MAX_SWEEP_GAPS],
    steps: usize,
}

fn prove_sweep(
    gaps: &[Gap],
    bands: &[Band],
    rooms: &[Option<(f64, f64)>],
    sep: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<SuperGap>> {
    let first = &gaps[0];
    let last = &gaps[gaps.len() - 1];
    let span = Span {
        from: first.next_piece().span.from,
        to: last.next_piece().span.to,
    };
    if span.from == span.to || !span.width().is_finite() {
        return Ok(None);
    }
    let start = advance(bands[0].bottom, first.upper_guard)?;
    let end = -advance(-bands[gaps.len()].top, last.lower_guard)?;
    if end <= start {
        return Ok(None);
    }
    let sweep = Sweep {
        span,
        start,
        end,
        ease: sweep_handle_fraction(span.width(), end - start),
    };
    let mut occupied = [span; MAX_SWEEP_GAPS];
    let mut above = sweep.at(bands[0].bottom, work)?;
    for (step, (gap, lower)) in gaps.iter().zip(&bands[1..]).enumerate() {
        work.visit(1)?;
        let top = sweep.at(lower.top, work)?;
        let aisle = above.hull(top);
        if !gap.clear_of_peers(aisle, sep, work)? {
            return Ok(None);
        }
        occupied[step] = aisle;
        if step + 1 < gaps.len() {
            let (lo, hi) = rooms[gap.next_piece().lower].unwrap();
            let bottom = sweep.at(lower.bottom, work)?;
            let through = top.hull(bottom);
            if through.from < lo || through.to > hi {
                return Ok(None);
            }
            above = bottom;
        }
    }
    Ok(Some(SuperGap {
        sweep,
        occupied,
        steps: gaps.len(),
    }))
}

fn super_gap(
    gaps: &[Gap],
    bands: &[Band],
    rooms: &[Option<(f64, f64)>],
    sep: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<SuperGap>> {
    let usable = |gap: &Gap| {
        gap.pieces.len() <= MAX_SWEEP_PEERS
            && !gap.has_end_labels
            && gap.next_piece().track.is_none()
    };
    if gaps.len() < 2 || rooms.is_empty() || !usable(&gaps[0]) {
        return Ok(None);
    }
    let mut current = gaps[0].next_piece();
    if rooms[current.lower].is_none() {
        return Ok(None);
    }
    let start_x = current.span.from;
    let mut steps = 1;
    for gap in gaps.iter().take(MAX_SWEEP_GAPS).skip(1) {
        work.visit(1)?;
        let next = gap.next_piece();
        let previous = current.span.to - start_x;
        let forward = next.span.to - next.span.from;
        if !usable(gap)
            || current.span.to != next.span.from
            || (previous < 0.0 && forward > 0.0)
            || (previous > 0.0 && forward < 0.0)
        {
            break;
        }
        steps += 1;
        current = next;
        if rooms[current.lower].is_none() {
            break;
        }
    }
    for count in (2..=steps).rev().take(MAX_SWEEP_TRIALS) {
        if let Some(merge) = prove_sweep(&gaps[..count], bands, rooms, sep, work)? {
            return Ok(Some(merge));
        }
    }
    Ok(None)
}

fn knot(points: &mut Vec<RoutePoint>, point: RoutePoint) {
    if let Some(previous) = points.last_mut()
        && previous.x == point.x
        && previous.y == point.y
    {
        if point.kind == KnotKind::GapBoundary {
            previous.kind = point.kind;
        }
    } else {
        points.push(point);
    }
}

fn draw_piece(
    points: &mut Vec<RoutePoint>,
    piece: &Piece,
    upper: &Band,
    lower: &Band,
    gap: &Gap,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let Span { from, to } = piece.span;
    if from == to {
        return Ok(());
    }
    let mut guides = [gap.upper_guard, gap.lower_guard];
    // Keep the shared runway for tracks and nearby shafts. A separated curve may use
    // the free part of the gap while keeping the leads at its own physical terminals.
    if gap.allow_local_sweeps
        && piece.track.is_none()
        && !gap.has_end_labels
        && (2..=MAX_SWEEP_PEERS).contains(&gap.pieces.len())
        && gap.clear_of_peers(piece.span, options.spacing.edge, work)?
    {
        guides[0] = guides[0].min(piece.leads[0]);
        if !piece.keep_landing_runway {
            guides[1] = guides[1].min(piece.leads[1]);
        }
    }
    let start = advance(upper.bottom, guides[0])?;
    let end = -advance(-lower.top, guides[1])?;
    if end <= start {
        return Err(PRECISION);
    }
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
        knot(
            points,
            RoutePoint::gap_boundary(from, start, Axis::Vertical),
        );
        knot(points, RoutePoint::new(from, shelf, Axis::Vertical));
        knot(points, RoutePoint::new(to, shelf, Axis::Horizontal));
        knot(points, RoutePoint::gap_boundary(to, end, Axis::Vertical));
    } else {
        knot(points, RoutePoint::new(from, start, Axis::Vertical));
        knot(points, RoutePoint::new(to, end, Axis::Vertical));
    }
    Ok(())
}

fn reverse_route(points: &mut [RoutePoint]) {
    points.reverse();
    for i in (1..points.len()).rev() {
        points[i].axis = points[i - 1].axis;
    }
    points[0].axis = points[1].axis;
}

fn face_separation(
    ranked: &RankedComponent<'_>,
    graph: &Graph,
    nodes: &[Rect],
    bands: &mut [Band],
    index: &[usize],
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    let c = ranked.component;
    let g = ranked.expanded;
    if c.boundaries
        || c.group_proxies
        || !c.loops.is_empty()
        || !graph.groups.is_empty()
        || !graph.lane_order.is_empty()
        || nodes.len() != graph.nodes.len()
        || ranked.orientation.cyclic_sccs != 0
        || bands.len() < 2
    {
        return Ok(false);
    }
    // Inline and terminal labels may appear after routing, with no final box in a band yet.
    work.visit(graph.edges.len())?;
    if graph
        .edges
        .iter()
        .any(|edge| edge.label.is_some() || edge.start_label.is_some() || edge.end_label.is_some())
    {
        return Ok(false);
    }
    for (layer, &row) in g.layers.iter().zip(index) {
        work.visit(1)?;
        let Some(band) = bands.get_mut(row) else {
            continue;
        };
        let mut separation = f64::INFINITY;
        let mut only_nodes = !layer.is_empty();
        let mut right = f64::NEG_INFINITY;
        for &v in layer {
            work.visit(1)?;
            let v = v as usize;
            let Some(node) = nodes.get(v).filter(|_| g.kind[v] == Kind::Node) else {
                only_nodes = false;
                continue;
            };
            if matches!(c.nodes[v].contour, ShapeContourHint::OffsetDiamond { x, y } if x != 0.0 || y != 0.0)
            {
                return Ok(false);
            }
            // The neighboring band bounds also have to enclose their real boxes.
            if node.y < band.top || node.y + node.height > band.bottom {
                return Ok(false);
            }
            separation = separation.min(node.x - right);
            right = node.x + node.width;
        }
        if only_nodes {
            band.face_separation = separation.max(0.0);
        }
    }
    Ok(true)
}

fn sweep_clearance_margin(
    points: &[RoutePoint],
    separation: f64,
    [upper_face, lower_face]: [f64; 2],
    [start_runway, end_runway]: [f64; 2],
) -> f64 {
    if separation == 0.0 {
        return 0.0;
    }
    let Some(i) = points.windows(2).position(|leg| leg[0].x != leg[1].x) else {
        return separation * 0.49;
    };
    if points[i + 1..].iter().any(|p| p.x != points[i + 1].x) {
        return 0.0;
    }
    let [from, to] = [points[i], points[i + 1]];
    // Painting can replace the first or last sweep knot with the end of a marker shaft.
    let start = from.y.max(points[0].y + start_runway);
    let end = to.y.min(points[points.len() - 1].y - end_runway);
    let along = end - start;
    let runway = (from.y - upper_face).min(lower_face - to.y);
    if along <= 0.0 || runway < 0.0 {
        return 0.0;
    }
    // The axial derivative of a sweep is at least 3/4 of its length. In the strip
    // within r of either rank the cross-axis motion is at most 3*across*(r/(.75*along))².
    // Use only half of the neighbor separation for that motion and half for the stroke.
    let across = (to.x - from.x).abs();
    let reach = 0.75 * along * (separation / across / 6.0).sqrt();
    (runway + (along * 0.25).min(reach)).min(separation * 0.5) * 0.99
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

#[cfg(test)]
pub(super) fn route(
    g: &Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    work: &mut Work<'_>,
) -> LayoutResult<Drawing> {
    let ranked = RankedComponent {
        component: c,
        expanded: g,
        orientation,
        side_ports: &[],
    };
    route_ranked(
        ranked,
        graph,
        options,
        x,
        &mut RoutingScratch::default(),
        work,
    )
    .map(|(drawing, _)| drawing)
}

pub(super) fn route_with_label_gaps(
    g: &mut Expanded,
    c: &Component,
    orientation: &Orientation,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    work: &mut Work<'_>,
) -> LayoutResult<(Drawing, Vec<[f64; 2]>)> {
    let side_ports = exterior::attach_exterior_ports(g, c, orientation, graph, options, x, work)?;
    g.spine_ports.resolve(
        [&mut g.target_offset, &mut g.source_offset],
        &options.spacing,
        |edge, end| {
            side_ports
                .get(edge)
                .is_some_and(|ports| ports[end].is_some())
        },
        work,
    )?;
    let mut scratch = std::mem::take(&mut g.routing_scratch);
    let ranked = RankedComponent {
        component: c,
        expanded: g,
        orientation,
        side_ports: &side_ports,
    };
    let result = route_ranked(ranked, graph, options, x, &mut scratch, work);
    g.routing_scratch = scratch;
    result
}

fn route_ranked(
    ranked: RankedComponent<'_>,
    graph: &Graph,
    options: &Options,
    x: &[f64],
    scratch: &mut RoutingScratch,
    work: &mut Work<'_>,
) -> LayoutResult<(Drawing, Vec<[f64; 2]>)> {
    work.check()?;
    let g = ranked.expanded;
    if g.keep.len() != g.rank.len() || x.len() != g.rank.len() {
        return Err(LayoutError::InvalidInput("incomplete refined vertices"));
    }
    bands(
        g,
        ranked.component,
        &mut scratch.bands,
        &mut scratch.index,
        work,
    )?;
    let mut gaps = collect_pieces(&ranked, graph, options, x, scratch, work)?;
    let bands = &mut scratch.bands;
    let index = &scratch.index;
    let (c, orientation, side_ports) = (ranked.component, ranked.orientation, ranked.side_ports);
    place_bands(bands, &mut gaps, options, &mut scratch.ids, work)?;
    let ranked_curves = options.routing == RoutingStyle::Curved && !g.direct.contains(&true);
    let clearance = if ranked_curves {
        sweep_clearance(g, c, options, x, index, &mut scratch.rooms, work)?;
        scratch.rooms.as_slice()
    } else {
        &[]
    };
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
    // An editor cache can replay a component after other components have been added.
    let prove_gaps = ranked_curves
        && !options.reuse_unchanged_components
        && face_separation(&ranked, graph, &nodes, bands, index, work)?;
    work.charge(c.links.len())?;
    let mut edges = reserved(c.links.len())?;
    if !g.inline_labels.is_empty() {
        work.charge(g.inline_labels.len())?;
    }
    let mut label_gaps = reserved(g.inline_labels.len())?;
    let mut inline = g.inline_labels.iter().map(|label| label.edge).peekable();
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
        if inline.next_if_eq(&edge).is_some() {
            label_gaps.push([bands[a].bottom, bands[b].top]);
        }
        let spec = &graph.edges[link.edge as usize];
        let direct = g.direct[edge];
        let [side_start, side_end] = side_ports.get(edge).copied().unwrap_or_default();
        let mut capacity =
            2 + 2 * usize::from(side_start.is_some()) + 2 * usize::from(side_end.is_some());
        if !direct {
            for gap in &gaps[a..b] {
                work.visit(1)?;
                let piece = &gap.pieces[gap.cursor];
                let bends = if piece.span.from == piece.span.to {
                    0
                } else if piece.track.is_none() {
                    2
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
        if let Some(port) = side_start {
            let y = nodes[first].center().y;
            points.push(RoutePoint::new(
                port.contact(nodes[first]),
                y,
                Axis::Horizontal,
            ));
            points.push(RoutePoint::new(port.rail, y, Axis::Horizontal));
            knot(
                &mut points,
                RoutePoint::gap_boundary(port.rail, bands[a].bottom, Axis::Vertical),
            );
        } else {
            points.push(RoutePoint::new(
                source_x,
                nodes[first].y + nodes[first].height,
                Axis::Vertical,
            ));
        }
        if !direct {
            let mut level = a;
            while level < b {
                work.visit(1)?;
                let piece = gaps[level].next_piece();
                // An adjacent sibling sets the outer limit for a shared departure sweep.
                let horizon = piece.horizon.unwrap_or(b).min(b).max(level + 1);
                let merged = super_gap(
                    &gaps[level..horizon],
                    &bands[level..],
                    clearance,
                    options.spacing.edge,
                    work,
                )?;
                let next = if let Some(merge) = merged {
                    let sweep = merge.sweep;
                    knot(
                        &mut points,
                        RoutePoint::gap_boundary(sweep.span.from, sweep.start, Axis::Vertical),
                    );
                    knot(
                        &mut points,
                        RoutePoint::gap_boundary(sweep.span.to, sweep.end, Axis::Vertical),
                    );
                    work.visit(merge.steps)?;
                    for (gap, occupied) in gaps[level..level + merge.steps]
                        .iter_mut()
                        .zip(merge.occupied)
                    {
                        gap.pieces[gap.cursor].occupied = occupied;
                    }
                    level + merge.steps
                } else {
                    let gap = &gaps[level];
                    draw_piece(
                        &mut points,
                        gap.next_piece(),
                        &bands[level],
                        &bands[level + 1],
                        gap,
                        options,
                        work,
                    )?;
                    level + 1
                };
                if next < b {
                    let piece = gaps[next - 1].next_piece();
                    let lower = &bands[next];
                    knot(
                        &mut points,
                        RoutePoint::new(piece.span.to, lower.top, Axis::Vertical),
                    );
                    if g.keep[piece.lower] {
                        knot(
                            &mut points,
                            RoutePoint::new(
                                piece.span.to,
                                lower.top.midpoint(lower.bottom),
                                Axis::Vertical,
                            ),
                        );
                    }
                    knot(
                        &mut points,
                        RoutePoint::new(piece.span.to, lower.bottom, Axis::Vertical),
                    );
                }
                for gap in &mut gaps[level..next] {
                    gap.cursor += 1;
                }
                level = next;
            }
        }
        if let Some(port) = side_end {
            let y = nodes[last].center().y;
            knot(
                &mut points,
                RoutePoint::gap_boundary(port.rail, bands[b].top, Axis::Vertical),
            );
            knot(&mut points, RoutePoint::new(port.rail, y, Axis::Vertical));
            points.push(RoutePoint::new(
                port.contact(nodes[last]),
                y,
                Axis::Horizontal,
            ));
        } else {
            let target = RoutePoint::new(target_x, nodes[last].y, Axis::Vertical);
            if points.len() == 1 {
                points.push(target);
            } else {
                knot(&mut points, target);
            }
        }
        let label_center = g.label_vertex[edge].map(|label| {
            let band = &bands[row(label as usize)];
            Point {
                x: x[label as usize],
                y: band.top.midpoint(band.bottom),
            }
        });
        let ports = (
            side_start.map_or(Side::Bottom, |port| port.side),
            side_end.map_or(Side::Top, |port| port.side),
        );
        if reversed {
            reverse_route(&mut points);
        }
        let (source, source_side) = if reversed {
            (last, ports.1)
        } else {
            (first, ports.0)
        };
        let across = points[1].x - points[0].x;
        let offset = points[0].x - x[source];
        let contour = c.nodes[source].contour;
        let contour_center = super::diamond_offset(contour, graph.direction)
            .map(|offset| offset.x)
            .or((contour == ShapeContourHint::Ellipse).then_some(0.0));
        let contour_departure = options.routing == RoutingStyle::Curved
            && !direct
            && source_side.axis() == Axis::Vertical
            && spec.start_marker.length == 0.0
            && spec.start_marker.width == 0.0
            && across != 0.0
            && points[0].y != points[1].y
            && contour_center.is_some_and(|center| (offset - center) * across >= 0.0);
        if contour_departure {
            points[0].kind = KnotKind::ContourDeparture;
        }
        // A dummy-only tier may be omitted from the physical bands. Require adjacency in
        // both views before trusting the two faces to bound the whole sweep.
        let corridor_clearance = if prove_gaps
            && !direct
            && !contour_departure
            && !reversed
            && side_start.is_none()
            && side_end.is_none()
            && b == a + 1
            && g.rank[last] == g.rank[first] + 1
            && gaps[a].pieces[gaps[a].cursor - 1].track.is_none()
            && (nodes[first].x..=nodes[first].x + nodes[first].width).contains(&source_x)
            && (nodes[last].x..=nodes[last].x + nodes[last].width).contains(&target_x)
        {
            let (upper, lower) = (&bands[a], &bands[b]);
            let separation = upper.face_separation.min(lower.face_separation);
            // Every other band's boxes are outside the outer faces of these two bands.
            let above = a
                .checked_sub(1)
                .map_or(f64::INFINITY, |row| upper.top - bands[row].bottom);
            let below = bands
                .get(b + 1)
                .map_or(f64::INFINITY, |band| band.top - lower.bottom);
            // Compaction checks foreign node moves against route envelopes with this spacing.
            let retained = if options.compact_ranks {
                options.spacing.edge
            } else {
                f64::INFINITY
            };
            sweep_clearance_margin(
                &points,
                separation,
                [upper.bottom, lower.top],
                spec.marker_runways(),
            )
            .min(0.99 * above.min(below).min(retained))
        } else {
            0.0
        };
        edges.push((
            link.edge,
            EdgeGeometry {
                points,
                ports: if reversed { (ports.1, ports.0) } else { ports },
                label_center,
                start_label_center: None,
                end_label_center: None,
                shared_bus: false,
                corridor_clearance,
                reversed_for_ranking: reversed,
            },
        ));
    }
    Ok((Drawing { nodes, edges }, label_gaps))
}

#[cfg(test)]
mod tests;
