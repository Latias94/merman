mod adjacent;
mod obstacle;

use super::*;
use crate::geom::{BoundsIndex, Bvh, BvhEntry};
use adjacent::AdjacentLines;
use obstacle::Shaft;
use std::ops::ControlFlow::{Break, Continue};

#[derive(Clone, Copy)]
struct SolidObstacle {
    bounds: Bounds,
    is_frame: bool,
}

#[derive(Clone, Copy)]
struct RouteObstacle {
    shaft: Shaft,
    edge: usize,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SeatMode {
    Beside,
    Inline,
    AcrossFrame,
    TightInline,
    AlongObstacles,
    Detached,
}

impl SeatMode {
    fn pad(self, clearance: f64) -> f64 {
        match self {
            Self::Beside => clearance,
            Self::Inline | Self::AcrossFrame => (clearance * 0.5).min(4.0),
            Self::TightInline | Self::AlongObstacles | Self::Detached => {
                (clearance * 0.25).min(1.5)
            }
        }
    }
}

struct Scene {
    solids: Bvh<SolidObstacle>,
    routes: Bvh<RouteObstacle>,
    labels: BoundsIndex,
    clearance: f64,
    card_grid: bool,
}

#[derive(Clone, Copy)]
struct Run {
    axis: usize,
    cross: f64,
    from: f64,
    to: f64,
}

impl Run {
    fn length(self) -> f64 {
        (self.to - self.from).abs()
    }

    fn at(self, distance: f64) -> f64 {
        self.from + (self.to - self.from).signum() * distance
    }
}

#[derive(Clone, Copy)]
struct CandidateRun {
    run: Run,
    ordinal: usize,
    start: f64,
    span: [f64; 2],
    preferred: f64,
    lower_cost: f64,
}

impl CandidateRun {
    fn arc(self, along: f64) -> f64 {
        self.start + (along - self.run.from).abs()
    }

    fn lower_cost(self, span: [f64; 2], target: f64) -> f64 {
        let [left, right] = span.map(|at| self.arc(at));
        let nearest = self.arc(self.run.from.clamp(span[0], span[1]));
        (target - target.clamp(nearest, left.max(right))).abs()
    }
}

#[derive(Default)]
struct SearchScratch {
    runs: Vec<Run>,
    ordered: Vec<CandidateRun>,
    blocked: Vec<[f64; 2]>,
    seats: Vec<f64>,
}

impl SearchScratch {
    fn prepare(
        &mut self,
        points: &[RoutePoint],
        markers: [f64; 2],
        fraction: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<f64> {
        self.runs.clear();
        for leg in points.windows(2) {
            work.visit(1)?;
            let [a, b] = [leg[0], leg[1]];
            if a.x == b.x && a.y == b.y {
                continue;
            }
            let run = if a.y == b.y {
                Run {
                    axis: 0,
                    cross: a.y,
                    from: a.x,
                    to: b.x,
                }
            } else if a.x == b.x {
                Run {
                    axis: 1,
                    cross: a.x,
                    from: a.y,
                    to: b.y,
                }
            } else {
                return Err(LayoutError::InvalidInput(
                    "non-orthogonal grid caption route",
                ));
            };
            if let Some(previous) = self.runs.last_mut()
                && previous.axis == run.axis
                && previous.cross == run.cross
                && previous.to == run.from
                && (previous.from < previous.to) == (run.from < run.to)
            {
                previous.to = run.to;
            } else {
                push(&mut self.runs, run)?;
            }
        }
        work.visit(self.runs.len())?;
        let length = self.runs.iter().map(|run| run.length()).sum::<f64>();
        if !length.is_finite() {
            return Err(PRECISION);
        }
        let first = markers[0].min(length);
        let last = (length - markers[1]).max(first);
        let target = (last - first).mul_add(fraction, first);
        let mut seen = 0.0;
        self.ordered.clear();
        for (ordinal, &run) in self.runs.iter().enumerate() {
            work.visit(1)?;
            let start = seen;
            seen += run.length();
            let lo = first.max(start);
            let hi = last.min(seen);
            if lo <= hi {
                let [a, b] = [run.at(lo - start), run.at(hi - start)];
                let span = [a.min(b), a.max(b)];
                let mut candidate = CandidateRun {
                    run,
                    ordinal,
                    start,
                    span,
                    preferred: run.at(target.clamp(lo, hi) - start),
                    lower_cost: 0.0,
                };
                candidate.lower_cost = candidate.lower_cost(span, target);
                push(&mut self.ordered, candidate)?;
            }
        }
        work.charge_sort(self.ordered.len())?;
        self.ordered.sort_unstable_by(|a, b| {
            a.lower_cost
                .total_cmp(&b.lower_cost)
                .then_with(|| a.ordinal.cmp(&b.ordinal))
        });
        Ok(target)
    }
}

/// Candidate centers move along a straight run, at a fixed offset from its shaft.
#[derive(Clone, Copy)]
struct SeatLine {
    axis: usize,
    span: [f64; 2],
    cross: f64,
    anchor_cross: f64,
    half: [f64; 2],
}

impl SeatLine {
    fn bounds(self, along: [f64; 2], cross: [f64; 2]) -> Bounds {
        if self.axis == 0 {
            Bounds { x: along, y: cross }
        } else {
            Bounds { x: cross, y: along }
        }
    }

    fn ink_span(self, rect: Bounds) -> Option<[f64; 2]> {
        let [along, cross] = self.axes(rect);
        let margin = self.half[1 - self.axis];
        (cross[0] < self.cross + margin && self.cross - margin < cross[1])
            .then(|| self.expand_ink_span(along))
    }

    fn expand_ink_span(self, along: [f64; 2]) -> [f64; 2] {
        let half = self.half[self.axis];
        [(along[0] - half).next_up(), (along[1] + half).next_down()]
    }

    fn axes(self, rect: Bounds) -> [[f64; 2]; 2] {
        if self.axis == 0 {
            [rect.x, rect.y]
        } else {
            [rect.y, rect.x]
        }
    }

    fn tether_span(self, rect: Bounds) -> Option<[f64; 2]> {
        let [along, cross] = self.axes(rect);
        (self.cross != self.anchor_cross
            && cross[1] > self.cross.min(self.anchor_cross)
            && cross[0] < self.cross.max(self.anchor_cross))
        .then_some(along)
    }

    fn foreign_span(self, rect: Bounds, reach: f64) -> Option<[f64; 2]> {
        let [along, cross] = self.axes(rect);
        let cross_gap = (cross[0] - self.cross).max(self.cross - cross[1]).max(0.0);
        if cross_gap > reach {
            return None;
        }
        // Project the part of the neighbor's distance field no farther away than the
        // host shaft. The caption center must remain in its own route's region.
        let radius = if reach == 0.0 {
            0.0
        } else {
            let ratio = cross_gap / reach;
            reach * (1.0 - ratio * ratio).sqrt()
        };
        Some([along[0] - radius, along[1] + radius])
    }
}

#[derive(Clone, Copy)]
struct SeatRequest {
    edge: usize,
    target: f64,
    size: Size,
}

impl Scene {
    fn add_caption(&mut self, ink: Bounds, work: &mut Work<'_>) -> LayoutResult<()> {
        self.labels.insert_with_work(ink, work)
    }

    fn closest_seat(
        &self,
        line: SeatLine,
        preferred: f64,
        mode: SeatMode,
        edge: usize,
        blocked: &mut Vec<[f64; 2]>,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<f64>> {
        blocked.clear();
        let [lo, hi] = line.span;
        let desired = preferred.clamp(lo, hi);
        let mut desired_blocked = false;
        let mut block = |span: Option<[f64; 2]>| -> LayoutResult<bool> {
            if let Some([a, b]) = span
                && a <= b
                && b >= lo
                && a <= hi
            {
                // An obstacle covering this whole run leaves no candidate to sort.
                if a <= lo && b >= hi {
                    return Ok(true);
                }
                desired_blocked |= a <= desired && desired <= b;
                push(blocked, [a, b])?;
            }
            Ok(false)
        };
        let half_along = line.half[line.axis];
        let cross_half = line.half[1 - line.axis];
        let ink = line.bounds(
            [lo - half_along, hi + half_along],
            [line.cross - cross_half, line.cross + cross_half],
        );
        if !ink.finite() {
            return Ok(None);
        }
        if self
            .labels
            .visit_overlaps_with_work(ink, work, |bounds, _| block(line.ink_span(bounds)))?
        {
            return Ok(None);
        }
        let tether = line.bounds(
            [lo, hi],
            [
                line.anchor_cross.min(line.cross),
                line.anchor_cross.max(line.cross),
            ],
        );
        let tethered = mode != SeatMode::Detached;
        let solids = if tethered { ink.union(tether) } else { ink };
        let solid_blocked = self.solids.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(solids.overlaps(bounds)),
                BvhEntry::Item(obs) => {
                    if (!obs.is_frame || matches!(mode, SeatMode::Beside | SeatMode::Inline))
                        && (block(line.ink_span(obs.bounds))?
                            || (tethered && block(line.tether_span(obs.bounds))?))
                    {
                        return Ok(Break(()));
                    }
                    Continue(true)
                }
            })
        })?;
        if solid_blocked.is_break() {
            return Ok(None);
        }
        // Keep enough separation to cover a small within-seat authored paint offset.
        let reach = (line.cross - line.anchor_cross).abs() + self.clearance.min(2.0);
        if !reach.is_finite() {
            return Ok(None);
        }
        let affinity = line.bounds(
            [lo - reach, hi + reach],
            [line.cross - reach, line.cross + reach],
        );
        let routes = ink.union(affinity);
        let route_blocked = self.routes.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(routes.overlaps(bounds)),
                BvhEntry::Item(obs) => {
                    let foreign = obs.edge != edge;
                    if ((foreign || mode == SeatMode::Beside) && block(obs.shaft.ink_span(line))?)
                        || (foreign && block(obs.shaft.foreign_span(line, reach))?)
                    {
                        return Ok(Break(()));
                    }
                    Continue(true)
                }
            })
        })?;
        if route_blocked.is_break() {
            return Ok(None);
        }
        if !desired_blocked {
            return Ok(Some(desired));
        }
        work.charge_sort(blocked.len())?;
        blocked.sort_unstable_by(|a, b| a[0].total_cmp(&b[0]).then_with(|| a[1].total_cmp(&b[1])));
        let mut best = None::<(f64, f64)>;
        let mut consider = |start, end| {
            if start <= end {
                let seat = preferred.clamp(start, end);
                let distance = (seat - preferred).abs();
                if best.is_none_or(|(old, _)| distance < old) {
                    best = Some((distance, seat));
                }
            }
        };
        let mut start = lo;
        for &[a, b] in blocked.iter() {
            work.visit(1)?;
            if b < start {
                continue;
            }
            consider(start, a.next_down().min(hi));
            start = start.max(b.next_up());
            if start > hi {
                break;
            }
        }
        consider(start, hi);
        Ok(best.map(|(_, seat)| seat))
    }

    fn adjacent_range(&self, line: SeatLine, room: [f64; 2]) -> [f64; 2] {
        let half = line.half[1 - line.axis];
        let reach = 2.0 * half + self.clearance;
        [
            (line.cross - reach).max(room[0] + half),
            (line.cross + reach).min(room[1] - half),
        ]
    }

    fn adjacent_lines(
        &self,
        line: SeatLine,
        [low, high]: [f64; 2],
        edge: usize,
        seats: &mut Vec<f64>,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        // A caption can slide around the edge of a solid or a previously seated caption.
        // Probe those boundaries before considering a detached row.
        let half = line.half[1 - line.axis];
        let along = [
            line.span[0] - line.half[line.axis],
            line.span[1] + line.half[line.axis],
        ];
        if low > high || !low.is_finite() || !high.is_finite() {
            return Ok(());
        }
        let area = line.bounds(along, [low - half, high + half]);
        let mut adjacent = AdjacentLines::new(line.cross, [low, high]);
        adjacent.insert(low, work)?;
        adjacent.insert(high, work)?;
        let mut border = |cross: [f64; 2], work: &mut Work<'_>| -> LayoutResult<()> {
            adjacent.insert((cross[0] - half).next_down(), work)?;
            adjacent.insert((cross[1] + half).next_up(), work)?;
            Ok(())
        };
        let _ = self.solids.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(bounds) => Continue(area.overlaps(bounds)),
                BvhEntry::Item(obs) => {
                    if !obs.is_frame && area.overlaps(obs.bounds) {
                        border(line.axes(obs.bounds)[1], work)?;
                    }
                    Continue(true)
                }
            })
        })?;
        if self.card_grid {
            let _ = self.routes.visit(|entry| -> LayoutResult<_> {
                work.visit(1)?;
                Ok(match entry {
                    BvhEntry::Node(bounds) => Continue(area.overlaps(bounds)),
                    BvhEntry::Item(obs) => {
                        if obs.edge != edge
                            && let Some(cross) = obs.shaft.adjacent_span(line, area)
                        {
                            border(cross, work)?;
                        }
                        Continue(true)
                    }
                })
            })?;
        }
        self.labels
            .visit_overlaps_with_work(area, work, |bounds, work| {
                border(line.axes(bounds)[1], work)?;
                Ok(false)
            })?;
        adjacent.collect(seats, work)
    }

    #[allow(clippy::too_many_arguments)]
    fn near_route(
        &self,
        points: &[RoutePoint],
        edge: usize,
        fraction: f64,
        markers: [f64; 2],
        size: Size,
        scope: Option<Bounds>,
        scratch: &mut SearchScratch,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        let clearance = 2.0 * self.clearance;
        if !(size.width + clearance).is_finite() || !(size.height + clearance).is_finite() {
            return Ok(None);
        }
        let request = SeatRequest {
            edge,
            target: scratch.prepare(points, markers, fraction, work)?,
            size,
        };
        let root = if self.card_grid {
            [f64::NEG_INFINITY, f64::INFINITY]
        } else {
            [0.0, f64::INFINITY]
        };
        let root = Bounds { x: root, y: root };
        let room = scope.unwrap_or(root);
        let local = [
            SeatMode::Beside,
            SeatMode::Inline,
            SeatMode::AcrossFrame,
            SeatMode::TightInline,
            SeatMode::AlongObstacles,
        ];
        let relaxed = if self.card_grid && scope.is_some() {
            &local[..]
        } else {
            &[]
        };
        // A local overhang keeps a stronger visual connection than a detached seat inside
        // the boundary. Only repeat a failed search where relaxing the scope changes it.
        for (room, previous, modes) in [
            (room, None, &local[..]),
            (root, scope, relaxed),
            (
                if self.card_grid { root } else { room },
                None,
                &[SeatMode::Detached][..],
            ),
        ] {
            for &mode in modes {
                if mode == SeatMode::TightInline && !self.card_grid {
                    continue;
                }
                if let Some(seat) =
                    self.search_mode(request, mode, room, previous, scratch, work)?
                {
                    return Ok(Some(seat));
                }
            }
        }
        Ok(None)
    }

    fn search_mode(
        &self,
        request: SeatRequest,
        mode: SeatMode,
        room: Bounds,
        previous: Option<Bounds>,
        scratch: &mut SearchScratch,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Point>> {
        let SeatRequest { edge, target, size } = request;
        let pad = mode.pad(self.clearance);
        let half = [size.width / 2.0 + pad, size.height / 2.0 + pad];
        if !half.into_iter().all(f64::is_finite) {
            return Ok(None);
        }
        let mut best = None::<(f64, usize, Point)>;
        for &candidate in &scratch.ordered {
            work.visit(1)?;
            if best.is_some_and(|(cost, _, _)| candidate.lower_cost > cost) {
                break;
            }
            let run = candidate.run;
            let axis = run.axis;
            let across = half[1 - axis];
            let [along_room, cross_room] = [room.x, room.y];
            let (along_room, cross_room) = if axis == 0 {
                (along_room, cross_room)
            } else {
                (cross_room, along_room)
            };
            let span = [
                candidate.span[0].max(along_room[0] + half[axis]),
                candidate.span[1].min(along_room[1] - half[axis]),
            ];
            if span[0] > span[1] {
                continue;
            }
            let along_cost = candidate.lower_cost(span, target);
            let can_win = |cost, best: Option<(f64, usize, Point)>| {
                best.is_none_or(|(old, ordinal, _)| (cost, candidate.ordinal) < (old, ordinal))
            };
            if !can_win(along_cost, best) {
                continue;
            }
            let preferred = candidate.preferred;
            let line = SeatLine {
                axis,
                span,
                cross: run.cross,
                anchor_cross: run.cross,
                half,
            };
            let previous_cross = previous.and_then(|room| {
                let [along, cross] = line.axes(room);
                (span[0] >= along[0] + half[axis] && span[1] <= along[1] - half[axis])
                    .then_some(cross)
            });
            let seats: &[f64] = match mode {
                SeatMode::Beside => &[
                    (run.cross - across).next_down(),
                    (run.cross + across).next_up(),
                ],
                SeatMode::AlongObstacles => {
                    let range = self.adjacent_range(line, cross_room);
                    if previous_cross.is_some_and(|room| self.adjacent_range(line, room) == range) {
                        continue;
                    }
                    scratch.seats.clear();
                    self.adjacent_lines(line, range, edge, &mut scratch.seats, work)?;
                    &scratch.seats
                }
                SeatMode::Detached => &[
                    run.cross - 120.0,
                    run.cross + 120.0,
                    run.cross - 180.0,
                    run.cross + 180.0,
                    run.cross - 240.0,
                    run.cross + 240.0,
                ],
                _ => &[run.cross],
            };
            for &cross in seats {
                work.visit(1)?;
                let offset = (cross - run.cross).abs();
                if !can_win(along_cost + offset, best) {
                    // Only obstacle-adjacent lines are sorted by their actual offset. The
                    // authored order of the two sides can differ by an ULP after rounding.
                    if mode == SeatMode::AlongObstacles {
                        break;
                    }
                    continue;
                }
                if cross < cross_room[0] + across || cross > cross_room[1] - across {
                    continue;
                }
                if mode != SeatMode::AlongObstacles
                    && previous_cross
                        .is_some_and(|room| cross >= room[0] + across && cross <= room[1] - across)
                {
                    continue;
                }
                let line = SeatLine { cross, ..line };
                if let Some(at) =
                    self.closest_seat(line, preferred, mode, edge, &mut scratch.blocked, work)?
                {
                    let distance = (candidate.arc(at) - target).abs() + offset;
                    if can_win(distance, best) {
                        let mut coords = [cross; 2];
                        coords[axis] = at;
                        best = Some((distance, candidate.ordinal, point(coords)));
                    }
                }
            }
        }
        Ok(best.map(|(_, _, seat)| seat))
    }
}

pub(super) fn place(
    context: &Routing<'_>,
    plan: &Plan,
    routes: &mut [EdgeGeometry],
    card_grid: bool,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let root = context.hierarchy.root();
    let solid_cap = context
        .drawing
        .nodes
        .len()
        .saturating_add(context.drawing.groups.len().saturating_mul(5))
        .saturating_add(plan.sites.len());
    work.charge(solid_cap)?;
    let mut solids = reserved(solid_cap)?;
    for &node_rect in &context.drawing.nodes {
        work.visit(1)?;
        solids.push(SolidObstacle {
            bounds: Bounds::of(node_rect),
            is_frame: false,
        });
    }
    for (group, geometry) in context.drawing.groups.iter().enumerate() {
        work.visit(1)?;
        if let Some(title) = context.title(group) {
            push(
                &mut solids,
                SolidObstacle {
                    bounds: Bounds::of(title),
                    is_frame: false,
                },
            )?;
        }
        let Bounds { x, y } = Bounds::of(geometry.rect);
        // A caption can occupy the empty interior of a boundary. Only the border carries ink.
        for bounds in [
            Bounds { x, y: [y[0]; 2] },
            Bounds { x, y: [y[1]; 2] },
            Bounds { x: [x[0]; 2], y },
            Bounds { x: [x[1]; 2], y },
        ] {
            push(
                &mut solids,
                SolidObstacle {
                    bounds,
                    is_frame: true,
                },
            )?;
        }
    }
    let route_cap = routes.len().saturating_mul(2);
    work.charge(route_cap)?;
    let mut route_obstacles = reserved(route_cap)?;
    let mut lengths = filled(routes.len(), 0.0)?;
    for (edge, route) in routes.iter().enumerate() {
        work.visit(1)?;
        let mut previous = None;
        for leg in route.points.windows(2) {
            work.visit(1)?;
            let (a, b) = (position(leg[0]), position(leg[1]));
            let bounds = Bounds::between(a, b);
            lengths[edge] += (bounds.x[1] - bounds.x[0]) + (bounds.y[1] - bounds.y[0]);
            let shaft = Shaft::new(a, b, bounds);
            if let (Some(Shaft::Ortho(run)), Shaft::Ortho(bounds)) = (&mut previous, shaft)
                && ((run.x == bounds.x && run.x[0] == run.x[1])
                    || (run.y == bounds.y && run.y[0] == run.y[1]))
            {
                *run = run.union(bounds);
            } else if let Some(shaft) = previous.replace(shaft) {
                push(&mut route_obstacles, RouteObstacle { shaft, edge })?;
            }
        }
        if let Some(shaft) = previous {
            push(&mut route_obstacles, RouteObstacle { shaft, edge })?;
        }
    }
    for site in &plan.sites {
        work.visit(1)?;
        let side = if site.inner {
            opposite(site.side)
        } else {
            site.side
        };
        if let Some(bounds) = Bounds::marker_footprint(site.at, side, site.marker) {
            push(
                &mut solids,
                SolidObstacle {
                    bounds,
                    is_frame: false,
                },
            )?;
        }
    }
    let mut scene = Scene {
        solids: Bvh::build_with_work(solids, |obs| obs.bounds, work)?,
        routes: Bvh::build_with_work(route_obstacles, |obs| obs.shaft.bounds(), work)?,
        labels: BoundsIndex::default(),
        clearance: context.metrics.label_clearance,
        card_grid,
    };
    let mut bottom = context.drawing.bounds.height;
    if let Some(ink) = scene.solids.root_bounds() {
        bottom = bottom.max(ink.y[1]);
    }
    if let Some(ink) = scene.routes.root_bounds() {
        bottom = bottom.max(ink.y[1]);
    }
    let mut scratch = SearchScratch::default();
    let mut labelled = Vec::new();
    // Narrow scopes and short routes offer fewer possible seats. Give them first choice.
    for &scope in context.hierarchy.order.iter().rev() {
        work.visit(1)?;
        let bounds = Bounds::of(context.bounds(scope));
        let constraint = (scope != root).then_some(bounds);
        labelled.clear();
        for part in &plan.scopes[scope] {
            work.visit(1)?;
            if part.labels.iter().any(Option::is_some) {
                push(&mut labelled, part)?;
            }
        }
        work.charge_sort(labelled.len())?;
        labelled.sort_unstable_by(|a, b| {
            lengths[a.edge]
                .total_cmp(&lengths[b.edge])
                .then_with(|| a.edge.cmp(&b.edge))
        });
        for &part in &labelled {
            work.visit(1)?;
            let edge = part.edge;
            let spec = &context.graph.edges[edge];
            let route = &mut routes[edge];
            let markers = [spec.start_marker.length, spec.end_marker.length];
            for ((size, fraction), center) in part.labels.into_iter().zip([0.5, 0.15, 0.85]).zip([
                &mut route.label_center,
                &mut route.start_label_center,
                &mut route.end_label_center,
            ]) {
                let Some(size) = size else { continue };
                let mut seat = scene.near_route(
                    &route.points,
                    edge,
                    fraction,
                    markers,
                    size,
                    constraint,
                    &mut scratch,
                    work,
                )?;
                if seat.is_none() && card_grid {
                    // Leave the caption unset so the caller can try against the final shaft.
                    continue;
                }
                // Placement reserves a caption row in each scope for congested routes.
                let y = bounds.y[1] - context.metrics.margin() - size.height / 2.0;
                let [lo, hi] = bounds.x;
                let inset = size.width / 2.0 + scene.clearance;
                let preferred_x = route
                    .points
                    .first()
                    .zip(route.points.last())
                    .map_or(lo.midpoint(hi), |(a, b)| a.x.midpoint(b.x));
                if seat.is_none() && lo + inset <= hi - inset {
                    let line = SeatLine {
                        axis: 0,
                        span: [lo + inset, hi - inset],
                        cross: y,
                        anchor_cross: y,
                        half: [inset, size.height / 2.0 + scene.clearance],
                    };
                    seat = scene
                        .closest_seat(
                            line,
                            preferred_x,
                            SeatMode::Detached,
                            edge,
                            &mut scratch.blocked,
                            work,
                        )?
                        .map(|x| Point { x, y });
                }
                let at = seat.unwrap_or(Point {
                    x: preferred_x.max(inset),
                    y: bottom + scene.clearance + size.height / 2.0,
                });
                let ink = Bounds::centered(at, size);
                if !ink.finite() {
                    return Err(PRECISION);
                }
                bottom = bottom.max(ink.y[1]);
                *center = Some(at);
                scene.add_caption(ink, work)?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KnotKind, NoopWorkControl};

    #[test]
    fn diagonal_does_not_block_a_caption_in_the_empty_corner_of_its_envelope() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let routes = vec![RouteObstacle {
            shaft: Shaft::Diagonal([Point { x: 0.0, y: 0.0 }, Point { x: 400.0, y: 400.0 }]),
            edge: 1,
        }];
        let scene = Scene {
            solids: Bvh::build_with_work(Vec::new(), |obs: &SolidObstacle| obs.bounds, &mut work)
                .unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.shaft.bounds(), &mut work).unwrap(),
            labels: Default::default(),
            clearance: 8.0,
            card_grid: true,
        };
        let route = [
            knot(Point { x: 100.0, y: 300.0 }, Axis::Horizontal),
            knot(Point { x: 200.0, y: 300.0 }, Axis::Horizontal),
        ];
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 30.0,
                    height: 20.0,
                },
                None,
                &mut SearchScratch::default(),
                &mut work,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            seat,
            Point {
                x: 150.0,
                y: 282.0_f64.next_down()
            }
        );
    }

    #[test]
    fn card_caption_can_leave_its_scope_before_detaching_from_the_route() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let scene = Scene {
            solids: Bvh::build_with_work(
                vec![SolidObstacle {
                    bounds: Bounds {
                        x: [0.0, 600.0],
                        y: [200.0, 325.0],
                    },
                    is_frame: false,
                }],
                |obs| obs.bounds,
                &mut work,
            )
            .unwrap(),
            routes: Bvh::build_with_work(
                Vec::new(),
                |obs: &RouteObstacle| obs.shaft.bounds(),
                &mut work,
            )
            .unwrap(),
            labels: Default::default(),
            clearance: 8.0,
            card_grid: true,
        };
        let route = [
            knot(Point { x: 100.0, y: 200.0 }, Axis::Horizontal),
            knot(Point { x: 400.0, y: 200.0 }, Axis::Horizontal),
        ];
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 160.0,
                    height: 50.0,
                },
                Some(Bounds {
                    x: [0.0, 600.0],
                    y: [200.0, 500.0],
                }),
                &mut SearchScratch::default(),
                &mut work,
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            seat,
            Point {
                x: 250.0,
                y: 167.0_f64.next_down()
            }
        );
    }

    #[test]
    fn parallel_route_captions_do_not_jump_across_foreign_wires() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // Two closely spaced parallel horizontal routes: edge 0 at y=160.4, edge 1 at y=167.4.
        // Place a solid blocker above the center of edge 0 so probe 0's upper candidate is blocked,
        // and ensure edge 0 does not jump below edge 1 (and edge 1 does not jump above edge 0).
        let solids = vec![SolidObstacle {
            bounds: Bounds {
                x: [310.0, 390.0],
                y: [100.0, 155.0],
            },
            is_frame: false,
        }];
        let routes = vec![
            RouteObstacle {
                shaft: Shaft::Ortho(Bounds {
                    x: [200.0, 500.0],
                    y: [160.4, 160.4],
                }),
                edge: 0,
            },
            RouteObstacle {
                shaft: Shaft::Ortho(Bounds {
                    x: [200.0, 500.0],
                    y: [167.4, 167.4],
                }),
                edge: 1,
            },
        ];
        let mut scene = Scene {
            solids: Bvh::build_with_work(solids, |obs| obs.bounds, &mut work).unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.shaft.bounds(), &mut work).unwrap(),
            labels: BoundsIndex::default(),
            clearance: 8.0,
            card_grid: true,
        };
        let route0 = [
            RoutePoint {
                x: 200.0,
                y: 160.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 500.0,
                y: 160.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let route1 = [
            RoutePoint {
                x: 200.0,
                y: 167.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 500.0,
                y: 167.4,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let size = Size {
            width: 80.0,
            height: 20.0,
        };
        let mut scratch = SearchScratch::default();
        let seat0 = scene
            .near_route(
                &route0,
                0,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut scratch,
                &mut work,
            )
            .unwrap()
            .expect("edge 0 should seat on its own side");
        assert!(
            seat0.y < 160.4,
            "edge 0 caption jumped across edge 1 (y=167.4) to y={}",
            seat0.y
        );
        scene
            .add_caption(Bounds::centered(seat0, size), &mut work)
            .unwrap();

        let seat1 = scene
            .near_route(
                &route1,
                1,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut scratch,
                &mut work,
            )
            .unwrap()
            .expect("edge 1 should seat on its own side");
        assert!(
            seat1.y > 167.4,
            "edge 1 caption jumped across edge 0 (y=160.4) to y={}",
            seat1.y
        );
    }

    #[test]
    fn trapped_center_wire_cannot_use_a_foreign_shaft_for_its_caption() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // A caption 24px high cannot fit in the 14px between the two outer shafts.
        let routes = vec![
            RouteObstacle {
                shaft: Shaft::Ortho(Bounds {
                    x: [100.0, 400.0],
                    y: [160.0, 160.0],
                }),
                edge: 0,
            },
            RouteObstacle {
                shaft: Shaft::Ortho(Bounds {
                    x: [100.0, 400.0],
                    y: [167.0, 167.0],
                }),
                edge: 1,
            },
            RouteObstacle {
                shaft: Shaft::Ortho(Bounds {
                    x: [100.0, 400.0],
                    y: [174.0, 174.0],
                }),
                edge: 2,
            },
        ];
        let scene = Scene {
            solids: Bvh::build_with_work(Vec::new(), |obs: &SolidObstacle| obs.bounds, &mut work)
                .unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.shaft.bounds(), &mut work).unwrap(),
            labels: BoundsIndex::default(),
            clearance: 8.0,
            card_grid: true,
        };
        let center_route = [
            RoutePoint {
                x: 100.0,
                y: 167.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 400.0,
                y: 167.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let size = Size {
            width: 90.0,
            height: 24.0,
        };
        let mut scratch = SearchScratch::default();
        let seat = scene
            .near_route(
                &center_route,
                1,
                0.5,
                [0.0, 0.0],
                size,
                None,
                &mut scratch,
                &mut work,
            )
            .unwrap();
        assert!(seat.is_none(), "seat {seat:?} would cross a foreign shaft");
    }

    #[test]
    fn non_finite_caption_dimensions_do_not_panic() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        let scene = Scene {
            solids: Bvh::build_with_work(Vec::new(), |obs: &SolidObstacle| obs.bounds, &mut work)
                .unwrap(),
            routes: Bvh::build_with_work(
                Vec::new(),
                |obs: &RouteObstacle| obs.shaft.bounds(),
                &mut work,
            )
            .unwrap(),
            labels: BoundsIndex::default(),
            clearance: f64::MAX,
            card_grid: true,
        };
        let route = [
            RoutePoint {
                x: 10.0,
                y: 20.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 200.0,
                y: 20.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let mut scratch = SearchScratch::default();
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 1e308,
                    height: 20.0,
                },
                None,
                &mut scratch,
                &mut work,
            )
            .unwrap();
        assert!(seat.is_none());
    }

    #[test]
    fn congested_caption_uses_extended_local_rescue_within_local_radius() {
        let mut noop = NoopWorkControl;
        let mut work = Work::new(&mut noop);
        // Block the immediate 120px neighborhood above and below the route with solid cards so
        // standard rings fail, while leaving an open pocket ~150px below the route.
        let solids = vec![
            SolidObstacle {
                bounds: Bounds {
                    x: [0.0, 600.0],
                    y: [0.0, 195.0],
                },
                is_frame: false,
            },
            SolidObstacle {
                bounds: Bounds {
                    x: [0.0, 600.0],
                    y: [205.0, 325.0],
                },
                is_frame: false,
            },
        ];
        let routes = vec![RouteObstacle {
            shaft: Shaft::Ortho(Bounds {
                x: [100.0, 400.0],
                y: [200.0, 200.0],
            }),
            edge: 0,
        }];
        let scene = Scene {
            solids: Bvh::build_with_work(solids, |obs| obs.bounds, &mut work).unwrap(),
            routes: Bvh::build_with_work(routes, |obs| obs.shaft.bounds(), &mut work).unwrap(),
            labels: BoundsIndex::default(),
            clearance: 8.0,
            card_grid: true,
        };
        let route = [
            RoutePoint {
                x: 100.0,
                y: 200.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
            RoutePoint {
                x: 400.0,
                y: 200.0,
                axis: Axis::Horizontal,
                kind: KnotKind::Free,
            },
        ];
        let mut scratch = SearchScratch::default();
        let seat = scene
            .near_route(
                &route,
                0,
                0.5,
                [0.0, 0.0],
                Size {
                    width: 160.0,
                    height: 50.0,
                },
                None,
                &mut scratch,
                &mut work,
            )
            .unwrap()
            .expect("extended local rescue should find the open pocket at y ~ 360");
        assert!(
            seat.y >= 350.0 && seat.y <= 400.0,
            "expected extended local seat near y=360, got {seat:?}"
        );
    }
}
