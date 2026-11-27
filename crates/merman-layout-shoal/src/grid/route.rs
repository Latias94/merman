//! Route each scope against its immediate walls, then stitch contacts at group faces.

mod captions;
mod comb;
mod gutters;

use gutters::gutter_route;

use super::*;
use crate::geom::Bounds;
use crate::ortho::{OrthoRequest, OrthoRouter, OrthoSegment};
use crate::{
    BorderStitch, EdgeGeometry, EdgeId, MarkerMetrics, Rect, RoutePoint, TitlePosition,
    TruncatedRoute,
};

const SNAP_EPS: f64 = 1e-9;

#[derive(Clone, Copy)]
struct Site {
    owner: usize,
    side: Side,
    preferred: f64,
    tie_preferred: f64,
    offset: Option<f64>,
    inner: bool,
    marker: MarkerMetrics,
    reach: f64,
    at: Point,
    lane: FaceLane,
    aligned_dist: Option<f64>,
    straight: bool,
    scope: usize,
}

#[derive(Clone, Copy, Default)]
struct FaceLane {
    ordinal: usize,
    degree: usize,
}

#[derive(Clone, Copy)]
struct End {
    site: usize,
    inner: bool,
}

#[derive(Clone, Copy)]
struct Part {
    edge: usize,
    from: End,
    to: End,
    labels: [Option<Size>; 3],
}

#[derive(Clone, Copy)]
struct Slot {
    scope: usize,
    index: usize,
}

struct Plan {
    sites: Vec<Site>,
    scopes: Vec<Vec<Part>>,
    paths: Vec<Vec<Slot>>,
    truncated: Vec<TruncatedRoute>,
    border_ports_left: usize,
}

fn opposite(side: Side) -> Side {
    match side {
        Side::Top => Side::Bottom,
        Side::Right => Side::Left,
        Side::Bottom => Side::Top,
        Side::Left => Side::Right,
    }
}

fn side_toward(from: Rect, to: Point, preferred: Side) -> Side {
    let [dx, dy] = [to.x - from.center().x, to.y - from.center().y];
    let clearance = [from.width * 0.5, from.height * 0.5];
    let (axis, sign) = normal(preferred);
    if [dx, dy][axis] * sign as f64 >= clearance[axis] {
        return preferred;
    }
    if dx.abs() - clearance[0] >= dy.abs() - clearance[1] {
        if dx >= 0.0 { Side::Right } else { Side::Left }
    } else if dy >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

fn cross(side: Side, p: Point) -> f64 {
    if side.axis() == Axis::Vertical {
        p.x
    } else {
        p.y
    }
}

fn anchor(graph: &Graph, drawing: &LayoutOutput, item: usize) -> Point {
    let body = rect(drawing, item);
    let y = graph
        .nodes
        .get(item)
        .and_then(|node| node.anchor_y)
        .map_or(body.center().y, |offset| body.y + offset);
    Point {
        x: body.center().x,
        y,
    }
}

impl Plan {
    fn new(context: &Routing<'_>, work: &mut Work<'_>) -> LayoutResult<Self> {
        let scopes = context.hierarchy.scopes.len();
        let edges = context.graph.edges.len();
        work.charge(scopes + edges)?;
        Ok(Self {
            sites: Vec::new(),
            scopes: filled(scopes, Vec::new())?,
            paths: reserved(edges)?,
            truncated: Vec::new(),
            border_ports_left: context.options.caps.max_border_ports,
        })
    }

    fn site(&mut self, site: Site, inner: bool) -> LayoutResult<End> {
        let id = self.sites.len();
        push(&mut self.sites, site)?;
        Ok(End { site: id, inner })
    }

    fn leg(&mut self, scope: usize, part: Part) -> LayoutResult<Slot> {
        let index = self.scopes[scope].len();
        push(&mut self.scopes[scope], part)?;
        Ok(Slot { scope, index })
    }

    fn edge(
        &mut self,
        context: &Routing<'_>,
        projection: Projection,
        sides: [Side; 2],
        edge: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let spec = &context.graph.edges[edge];
        let h = context.hierarchy;
        let scope = projection.scope;
        let scope_item = h.node_count + scope;
        let transit = |item: usize| {
            if item == scope_item {
                0
            } else {
                h.scopes[h.parent[item]].depth - h.scopes[scope].depth
            }
        };
        let needed = transit(projection.ends[0])
            .checked_add(transit(projection.ends[1]))
            .ok_or(WorkError::ArithmeticOverflow)?;
        let mut ends = projection.ends;
        let kept = if needed <= self.border_ports_left {
            self.border_ports_left -= needed;
            needed
        } else {
            for (end, item) in ends.iter_mut().zip(projection.items) {
                *end = item.unwrap_or(*end);
            }
            push(
                &mut self.truncated,
                TruncatedRoute {
                    edge: EdgeId(edge as u32),
                    source: h.endpoint(ends[0]),
                    target: h.endpoint(ends[1]),
                },
            )?;
            0
        };
        let mut steps = reserved(kept.checked_add(1).ok_or(WorkError::ArithmeticOverflow)?)?;
        let hints = [spec.source_port, spec.target_port];
        let markers = [spec.start_marker, spec.end_marker];
        let mut terminals = [End {
            site: 0,
            inner: false,
        }; 2];
        let mut incoming = 0;
        for (end, (&item, &side)) in ends.iter().zip(&sides).enumerate() {
            work.visit(1)?;
            let other = anchor(context.graph, context.drawing, ends[1 - end]);
            let from_rect = rect(context.drawing, item);
            let from = anchor(context.graph, context.drawing, item);
            let inner = item == scope_item;
            let offset = match hints[end] {
                PortSideHint::FixedOffset { offset, .. } if item == projection.ends[end] => {
                    Some(offset)
                }
                _ => None,
            };
            let aligned_dist = if !inner && item < h.node_count && sides[1 - end] == opposite(side)
            {
                let (axis, sign) = normal(side);
                let delta = [other.x - from.x, other.y - from.y][axis] * sign as f64;
                ((cross(side, other) - cross(side, from)).abs() <= 1.0 && delta > 0.0)
                    .then_some(delta)
            } else {
                None
            };
            let terminal_side = side;
            let mut terminal = self.site(
                Site {
                    owner: item,
                    side,
                    preferred: cross(side, other),
                    tie_preferred: cross(side, other),
                    offset,
                    inner,
                    marker: markers[end],
                    reach: 0.0,
                    at: Point::default(),
                    lane: FaceLane::default(),
                    aligned_dist,
                    straight: false,
                    scope: if inner { scope } else { h.parent[item] },
                },
                inner,
            )?;
            let mut owner = if inner { scope } else { h.parent[item] };
            if end == 1 {
                incoming = steps.len();
            }
            while owner != scope {
                work.visit(1)?;
                let group = h.node_count + owner;
                let side = side_toward(rect(context.drawing, group), other, side);
                // When the terminal face is perpendicular to the group border it exits through
                // (e.g., entering `orders:L` via `core:Top`), align the border crossing with the
                // terminal's outward lead corridor instead of overshooting to its center.
                let preferred = if terminal_side.axis() != side.axis() {
                    let (t_axis, t_sign) = normal(terminal_side);
                    let b = Bounds::of(from_rect);
                    let edge_coord = [b.x, b.y][t_axis][usize::from(t_sign > 0)];
                    edge_coord + t_sign as f64 * (markers[end].length + context.metrics.separation)
                } else {
                    cross(side, from)
                };
                let outer = self.site(
                    Site {
                        owner: group,
                        side,
                        preferred,
                        tie_preferred: cross(side, other),
                        offset: None,
                        inner: true,
                        marker: MarkerMetrics::default(),
                        reach: 0.0,
                        at: Point::default(),
                        lane: FaceLane::default(),
                        aligned_dist: None,
                        straight: false,
                        scope: h.parent[group],
                    },
                    true,
                )?;
                let (from, to) = if end == 0 {
                    (terminal, outer)
                } else {
                    (outer, terminal)
                };
                steps.push(self.leg(
                    owner,
                    Part {
                        edge,
                        from,
                        to,
                        labels: [None; 3],
                    },
                )?);
                terminal = End {
                    inner: false,
                    ..outer
                };
                owner = h.parent[group];
            }
            terminals[end] = terminal;
        }
        let [from, to] = terminals;
        steps.push(self.leg(
            scope,
            Part {
                edge,
                from,
                to,
                labels: [spec.label, spec.start_label, spec.end_label],
            },
        )?);
        steps[incoming..].reverse();
        self.paths.push(steps);
        Ok(())
    }

    fn mark_straight_entries(
        &mut self,
        node_count: usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let part = |slot: &Slot| self.scopes[slot.scope][slot.index];
        let direction = |site: Site, end: End| normal(site.side).1 * if end.inner { -1 } else { 1 };
        'paths: for path in &self.paths {
            work.visit(1)?;
            if path.len() < 2 {
                continue;
            }
            let from = self.sites[part(&path[0]).from.site];
            let to = self.sites[part(path.last().unwrap()).to.site];
            let sign = normal(from.side).1;
            if from.owner >= node_count
                || to.owner >= node_count
                || from.side.axis() != Axis::Vertical
                || to.side != opposite(from.side)
                || (from.at.x - to.at.x).abs() > SNAP_EPS
                || (to.at.y - from.at.y) * sign as f64 <= 0.0
            {
                continue;
            }
            // The measured terminal slots and every intermediate contact must already
            // admit the same column before a title can get credit for preserving it.
            for slot in path {
                work.visit(1)?;
                let Part {
                    from: start,
                    to: end,
                    ..
                } = part(slot);
                for (end, sign) in [(start, sign), (end, -sign)] {
                    let site = self.sites[end.site];
                    if site.side.axis() != Axis::Vertical
                        || direction(site, end) != sign
                        || (site.owner >= node_count
                            && (site.preferred - from.at.x).abs() > SNAP_EPS)
                    {
                        continue 'paths;
                    }
                }
            }
            for slot in path {
                work.visit(1)?;
                let Part { from, to, .. } = part(slot);
                for end in [from, to] {
                    let site = &mut self.sites[end.site];
                    if site.owner >= node_count && site.side == Side::Top {
                        site.straight = true;
                    }
                }
            }
        }
        Ok(())
    }
}

fn site_order(sites: &[Site], work: &mut Work<'_>) -> LayoutResult<Vec<usize>> {
    let mut indices = reserved(sites.len())?;
    work.charge(sites.len())?;
    indices.extend(0..sites.len());
    work.sort(&mut indices, |&a, &b| {
        let u = &sites[a];
        let v = &sites[b];
        (u.owner, u.side as u8)
            .cmp(&(v.owner, v.side as u8))
            .then_with(|| u.preferred.total_cmp(&v.preferred))
            .then_with(|| u.tie_preferred.total_cmp(&v.tie_preferred))
            .then(a.cmp(&b))
    })?;
    Ok(indices)
}

#[derive(Clone, Copy)]
struct GroupSlots {
    range: [f64; 2],
    count: usize,
    pitch: f64,
    split: Option<(usize, f64, f64)>,
}

impl GroupSlots {
    fn beside(self, gap: (f64, f64), preferred: impl Iterator<Item = f64>) -> Self {
        let [lo, hi] = self.range;
        let left_hi = gap.0.min(hi);
        let right_lo = gap.1.max(lo);
        let capacity = |available: f64| {
            if available < 0.0 {
                0
            } else if self.pitch > 0.0 {
                ((available / self.pitch).floor() as usize)
                    .saturating_add(1)
                    .min(self.count)
            } else {
                self.count
            }
        };
        let left = capacity(left_hi - lo);
        let right = capacity(hi - right_lo);
        let middle = gap.0.midpoint(gap.1);
        let want = preferred.filter(|&x| x < middle).count();
        let split = if left == 0 && right == 0 {
            if left_hi - lo >= hi - right_lo {
                self.count
            } else {
                0
            }
        } else {
            want.max(self.count.saturating_sub(right)).min(left)
        };
        Self {
            split: Some((split, left_hi.max(lo), right_lo.min(hi))),
            ..self
        }
    }

    fn at(self, preferred: f64, rank: usize, last: f64) -> f64 {
        let [lo, hi] = self.range;
        let (lo, hi, count, rank) = match self.split {
            Some((split, left_hi, _)) if rank < split => (lo, left_hi, split, rank),
            Some((split, _, right_lo)) => (right_lo, hi, self.count - split, rank - split),
            None => (lo, hi, self.count, rank),
        };
        let span = count.saturating_sub(1) as f64;
        let pitch = self.pitch.min((hi - lo).max(0.0) / span.max(1.0));
        let high = hi - (span - rank as f64) * pitch;
        let low = if rank == 0 {
            lo
        } else {
            (last + pitch).max(lo)
        };
        preferred.clamp(low.min(high), high)
    }
}

struct Routing<'a> {
    graph: &'a Graph,
    hierarchy: &'a Hierarchy,
    drawing: &'a LayoutOutput,
    metrics: Metrics,
    options: &'a Options,
}

impl Routing<'_> {
    fn bounds(&self, scope: usize) -> Rect {
        self.drawing.groups.get(scope).map_or(
            Rect {
                width: self.drawing.bounds.width,
                height: self.drawing.bounds.height,
                ..Rect::default()
            },
            |group| group.rect,
        )
    }

    fn title(&self, group: usize) -> Option<Rect> {
        let at = self.drawing.groups.get(group)?.title_center?;
        let size = self.graph.groups[group].title?;
        Some(Rect {
            x: at.x - size.width / 2.0,
            y: at.y - size.height / 2.0,
            width: size.width,
            height: size.height,
        })
    }

    fn seat(&self, owner: usize, side: Side, inner: bool) -> [f64; 3] {
        let body = rect(self.drawing, owner);
        let transverse = 1 - normal(side).0;
        let [lo, hi] = [Bounds::of(body).x, Bounds::of(body).y][transverse];
        if let Some(node) = self.graph.nodes.get(owner) {
            let center = if transverse == 1 {
                body.y + node.anchor_y.unwrap_or(body.height / 2.0)
            } else {
                body.center().x
            };
            let span = if side.axis() == self.graph.direction.axis() {
                node.port_span
            } else {
                node.perpendicular_port_span.or(node.port_span)
            };
            let half = span.unwrap_or(hi - lo) / 2.0;
            return [(center - half).max(lo), center, (center + half).min(hi)];
        }
        let border = self
            .options
            .spacing
            .corner_clearance
            .max(self.metrics.separation);
        let inset = border.min((hi - lo) / 2.0);
        let mut range = [lo + inset, hi - inset];
        if inner
            && matches!(side, Side::Left | Side::Right)
            && let Some(title) = self.title(owner - self.hierarchy.node_count)
        {
            let group = &self.graph.groups[owner - self.hierarchy.node_count];
            let title_is_top = place::is_top(group.title_position);
            let clear = self.metrics.separation;
            if title_is_top {
                range[0] = range[0].max(title.y + title.height + clear);
            } else {
                range[1] = range[1].min(title.y - clear);
            }
        }
        if range[0] > range[1] {
            range = [lo.midpoint(hi); 2];
        }
        [range[0], lo.midpoint(hi), range[1]]
    }

    fn border_title_gap(&self, owner: usize, side: Side, inner: bool) -> Option<(f64, f64)> {
        if !inner || owner < self.hierarchy.node_count || !matches!(side, Side::Top | Side::Bottom)
        {
            return None;
        }
        let group_id = owner - self.hierarchy.node_count;
        let title = self.title(group_id)?;
        let title_is_top = place::is_top(self.graph.groups[group_id].title_position);
        if title_is_top != (side == Side::Top) {
            return None;
        }
        let clear = self.metrics.separation;
        Some((title.x - clear, title.x + title.width + clear))
    }

    fn ports(
        &self,
        indices: &mut [usize],
        sites: &mut [Site],
        work: &mut Work<'_>,
    ) -> LayoutResult<()> {
        let mut cursor = 0;
        while cursor < indices.len() {
            let first = sites[indices[cursor]];
            let mut next = cursor + 1;
            let mut inner = first.inner;
            let mut reach = first.marker.length;
            let mut count = usize::from(first.offset.is_none());
            while let Some(&i) = indices.get(next) {
                let site = sites[i];
                work.visit(1)?;
                if site.owner != first.owner || site.side != first.side {
                    break;
                }
                inner |= site.inner;
                reach = reach.max(site.marker.length);
                count += usize::from(site.offset.is_none());
                next += 1;
            }
            let body = rect(self.drawing, first.owner);
            let [lo, center, hi] = self.seat(first.owner, first.side, inner);
            // When multiple collinear edges share a node face (e.g., Admin and User both
            // targeting Inventory:L), the farther edge must detour around the closer one.
            // Nudge the farther edge toward the open scope interior so it takes the side port
            // matching its detour gutter instead of crossing the direct neighbor's straight line.
            if first.owner < self.hierarchy.node_count && count >= 2 {
                work.visit(next - cursor)?;
                let min_dist = indices[cursor..next]
                    .iter()
                    .filter_map(|&i| {
                        sites[i]
                            .offset
                            .is_none()
                            .then_some(sites[i].aligned_dist)
                            .flatten()
                    })
                    .fold(f64::INFINITY, f64::min);
                if min_dist.is_finite() {
                    let mut nudged = false;
                    work.visit(next - cursor)?;
                    for &i in &indices[cursor..next] {
                        let site = &mut sites[i];
                        if site.offset.is_none()
                            && let Some(dist) = site.aligned_dist
                            && dist > min_dist + 1.0
                        {
                            let scope_center = cross(first.side, self.bounds(site.scope).center());
                            let dir = if center <= scope_center { 1.0 } else { -1.0 };
                            site.preferred = center + dir * 2.0;
                            nudged = true;
                        }
                    }
                    if nudged {
                        work.charge_sort(next - cursor)?;
                        indices[cursor..next].sort_unstable_by(|&a, &b| {
                            sites[a]
                                .preferred
                                .total_cmp(&sites[b].preferred)
                                .then_with(|| {
                                    sites[a].tie_preferred.total_cmp(&sites[b].tie_preferred)
                                })
                                .then(a.cmp(&b))
                        });
                        work.check()?;
                    }
                }
            }
            let (axis, sign) = normal(first.side);
            let bounds = Bounds::of(body);
            let [face_lo, face_hi] = [bounds.x, bounds.y][1 - axis];
            let face_center = face_lo.midpoint(face_hi);
            let span = count.saturating_sub(1) as f64;
            let pitch = self
                .options
                .spacing
                .port_pitch
                .min((hi - lo) / span.max(1.0));
            // If a single unpinned site on a node face is aligned with the face center,
            // anchor it at `center` so straight direct connections avoid an unnecessary jog.
            let mut anchored_start = None;
            if first.owner < self.hierarchy.node_count && count >= 2 {
                work.visit(next - cursor)?;
                let mut aligned_slot = None;
                let mut aligned_count = 0usize;
                let mut unpinned_rank = 0usize;
                for &i in &indices[cursor..next] {
                    if sites[i].offset.is_some() {
                        continue;
                    }
                    if (sites[i].preferred - center).abs() <= 1.0 {
                        aligned_slot = Some(unpinned_rank);
                        aligned_count += 1;
                    }
                    unpinned_rank += 1;
                }
                if aligned_count == 1
                    && let Some(slot) = aligned_slot
                {
                    let candidate = center - slot as f64 * pitch;
                    if candidate >= lo && candidate + span * pitch <= hi {
                        anchored_start = Some(candidate);
                    }
                }
            }
            let start = anchored_start.unwrap_or_else(|| {
                (center - span * pitch / 2.0).clamp(lo, (hi - span * pitch).max(lo))
            });
            let mut group_slots = GroupSlots {
                range: [lo, hi],
                count,
                pitch: self.options.spacing.port_pitch,
                split: None,
            };
            if let Some(gap) = self.border_title_gap(first.owner, first.side, inner) {
                work.visit(next - cursor)?;
                group_slots = group_slots.beside(
                    gap,
                    indices[cursor..next].iter().filter_map(|&i| {
                        let site = sites[i];
                        site.offset.is_none().then_some(site.preferred)
                    }),
                );
            }
            let mut rank = 0usize;
            let mut last = lo - pitch;
            for (ordinal, &i) in indices[cursor..next].iter().enumerate() {
                work.visit(1)?;
                let site = &mut sites[i];
                let cross = if let Some(offset) = site.offset {
                    let at = face_center + offset;
                    if at < face_lo || at > face_hi {
                        return Err(LayoutError::InvalidInput(
                            "fixed grid port lies outside its face",
                        ));
                    }
                    at
                } else {
                    let at = if first.owner >= self.hierarchy.node_count {
                        group_slots.at(site.preferred, rank, last)
                    } else {
                        start + rank as f64 * pitch
                    };
                    rank += 1;
                    last = at;
                    at
                };
                let mut along = [bounds.x, bounds.y][axis][usize::from(sign > 0)];
                if let Some(node) = self.graph.nodes.get(first.owner) {
                    let depth = [body.width, body.height][axis];
                    let inset = node
                        .contour
                        .inset_at(node.size, first.side, cross - face_center);
                    if inset.is_finite() {
                        along -= sign as f64 * inset.clamp(0.0, depth);
                    }
                }
                site.at = if axis == 0 {
                    Point { x: along, y: cross }
                } else {
                    Point { x: cross, y: along }
                };
                site.reach = reach;
                site.lane = FaceLane {
                    ordinal,
                    degree: next - cursor,
                };
            }
            cursor = next;
        }
        Ok(())
    }

    fn lead(&self, site: Site, inside: bool) -> (RoutePoint, Side) {
        let side = if inside {
            opposite(site.side)
        } else {
            site.side
        };
        let body = rect(self.drawing, site.owner);
        let bounds = Bounds::of(body);
        let (axis, sign) = normal(side);
        let (_, face_sign) = normal(site.side);
        let face = [bounds.x, bounds.y][axis][usize::from(face_sign > 0)];
        let mut at = [site.at.x, site.at.y];
        let d = site.reach + self.metrics.separation;
        at[axis] = if sign > 0 {
            (face + self.metrics.separation).max(at[axis] + d)
        } else {
            (face - self.metrics.separation).min(at[axis] - d)
        };
        (knot(point(at), side.axis()), side)
    }

    fn walls(&self, scope: usize, work: &mut Work<'_>) -> LayoutResult<Vec<Rect>> {
        let items = &self.hierarchy.scopes[scope].items;
        work.charge(items.len() + 1)?;
        let mut walls = reserved(items.len() + 1)?;
        for &item in items {
            work.visit(1)?;
            walls.push(rect(self.drawing, item));
        }
        walls.extend(self.title(scope));
        Ok(walls)
    }
}

fn copy_points(points: &[RoutePoint]) -> LayoutResult<Vec<RoutePoint>> {
    let mut out = reserved(points.len())?;
    out.extend_from_slice(points);
    Ok(out)
}

fn knot(at: Point, axis: Axis) -> RoutePoint {
    RoutePoint::new(at.x, at.y, axis)
}

fn position(at: RoutePoint) -> Point {
    Point { x: at.x, y: at.y }
}

fn leaves(side: Side, from: RoutePoint, toward: RoutePoint) -> bool {
    let (axis, sign) = normal(side);
    [toward.x - from.x, toward.y - from.y][axis] * sign as f64 >= 0.0
}

pub(crate) fn clear(
    router: &OrthoRouter,
    path: &[RoutePoint],
    bounds: Rect,
    sides: [Side; 2],
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    work.visit(path.len())?;
    if path.len() < 2 {
        return Ok(false);
    }
    let last = path.len() - 1;
    for (start, side) in [(0, sides[0]), (last, sides[1])] {
        let next = if start == 0 {
            path.iter().find(|&&p| position(p) != position(path[start]))
        } else {
            path.iter()
                .rev()
                .find(|&&p| position(p) != position(path[start]))
        };
        if next.is_none_or(|&next| !leaves(side, path[start], next)) {
            return Ok(false);
        }
    }
    for leg in path.windows(2) {
        if position(leg[0]) != position(leg[1]) {
            let Some(seg) = OrthoSegment::between(leg[0], leg[1]) else {
                return Ok(false);
            };
            if router.channel(seg, Some(bounds), work)?.is_none() {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn dogleg(a: RoutePoint, b: RoutePoint, axis: Axis, value: f64) -> [RoutePoint; 4] {
    let (p, q) = match axis {
        Axis::Horizontal => (
            RoutePoint::new(value, a.y, axis),
            RoutePoint::new(value, b.y, Axis::Vertical),
        ),
        Axis::Vertical => (
            RoutePoint::new(a.x, value, axis),
            RoutePoint::new(b.x, value, Axis::Horizontal),
        ),
    };
    [a, p, q, b]
}

fn fan_channel(
    router: &OrthoRouter,
    [start, end]: [(RoutePoint, Side); 2],
    lanes: [FaceLane; 2],
    bounds: Rect,
    work: &mut Work<'_>,
) -> LayoutResult<Option<[RoutePoint; 4]>> {
    let hub = usize::from(lanes[1].degree > lanes[0].degree);
    let lane = lanes[hub];
    if lane.degree < crate::ortho::comb::MIN_DEGREE || start.1 != opposite(end.1) {
        return Ok(None);
    }
    let [(from, side), (to, _)] = if hub == 0 { [start, end] } else { [end, start] };
    if !leaves(side, from, to) || cross(side, position(from)) == cross(side, position(to)) {
        return Ok(None);
    }
    let (a, b) = match side.axis() {
        Axis::Horizontal => (from.x, to.x),
        Axis::Vertical => (from.y, to.y),
    };
    if a == b {
        return Ok(None);
    }
    // Keep the outside branches' turns nearer the fan face. Consecutive branches can
    // then occupy distinct tracks without crossing each other's approach legs.
    let ordinal = if cross(side, position(to)) < cross(side, position(from)) {
        lane.ordinal + 1
    } else {
        lane.degree - lane.ordinal
    };
    let fraction = ordinal as f64 / (lane.degree as f64 + 1.0);
    let track = (b - a).mul_add(fraction, a);
    let path = dogleg(start.0, end.0, side.axis(), track);
    Ok(clear(router, &path, bounds, [start.1, end.1], work)?.then_some(path))
}

// The visibility graph may reach its cap on a dense grid. Try the shared channel query
// on the facing midline and the four open perimeter tracks before declining the route.
fn span(
    router: &mut OrthoRouter,
    start: (RoutePoint, Side),
    end: (RoutePoint, Side),
    bounds: Rect,
    separation: f64,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Vec<RoutePoint>>> {
    let [a, b] = [start.0, end.0];
    let sides = [start.1, end.1];
    if (position(a) != position(b) || start.1 == opposite(end.1))
        && let Some(path) = router.route(
            OrthoRequest {
                start: a,
                end: b,
                start_side: Some(start.1),
                end_side: Some(end.1),
                bounds: Some(bounds),
            },
            work,
        )?
    {
        return Ok(Some(path));
    }
    if position(a) == position(b) {
        // Opposed leads already meet. The visibility query tested their shared knot.
        if start.1 == opposite(end.1) {
            return Ok(None);
        }
        let (axis, sign) = normal(start.1);
        let cross = 1 - axis;
        for bend in [separation, -separation] {
            let mut coords = [a.x, a.y];
            let mut path = [a; 5];
            coords[cross] = [a.x, a.y][cross] + bend;
            path[1] = knot(point(coords), Axis::Horizontal);
            coords[axis] += sign as f64 * separation;
            path[2] = knot(point(coords), Axis::Horizontal);
            coords[cross] -= bend;
            path[3] = knot(point(coords), Axis::Horizontal);
            if clear(router, &path, bounds, sides, work)? {
                return Ok(Some(copy_points(&path)?));
            }
        }
        return Ok(None);
    }
    let [x, y] = [Bounds::of(bounds).x, Bounds::of(bounds).y];
    for (axis, values) in [
        (Axis::Horizontal, [a.x.midpoint(b.x), a.x, b.x, x[0], x[1]]),
        (Axis::Vertical, [a.y.midpoint(b.y), a.y, b.y, y[0], y[1]]),
    ] {
        for value in values {
            let path = dogleg(a, b, axis, value);
            if clear(router, &path, bounds, sides, work)? {
                return Ok(Some(copy_points(&path)?));
            }
        }
    }
    Ok(None)
}

fn add_point(points: &mut Vec<RoutePoint>, mut next: RoutePoint) {
    if let Some(last) = points.last_mut() {
        if position(*last) == position(next) {
            return;
        }
        next.axis = if last.x == next.x {
            Axis::Vertical
        } else {
            Axis::Horizontal
        };
        if points.len() == 1 {
            points[0].axis = next.axis;
        }
    }
    points.push(next);
}

/// Align transverse site coordinates that differ only by floating-point ULP noise
/// (e.g., two nodes of different widths centered on the same grid track), keeping every
/// snapped site inside its owning face and routing scope bounds.
fn snap_collinear_sites(
    context: &Routing<'_>,
    sites: &mut [Site],
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let fits = |site: Site, candidate: f64| {
        let transverse = 1 - normal(site.side).0;
        let body = Bounds::of(rect(context.drawing, site.owner));
        let scope = Bounds::of(context.bounds(site.scope));
        let [face_lo, face_hi] = [body.x, body.y][transverse];
        let [scope_lo, scope_hi] = [scope.x, scope.y][transverse];
        candidate >= face_lo.max(scope_lo) && candidate <= face_hi.min(scope_hi)
    };
    for axis in [Axis::Vertical, Axis::Horizontal] {
        let mut indices = reserved(sites.len())?;
        work.charge(sites.len())?;
        for (i, site) in sites.iter().enumerate() {
            work.visit(1)?;
            if site.side.axis() == axis {
                indices.push(i);
            }
        }
        if indices.len() < 2 {
            continue;
        }
        work.charge_sort(indices.len())?;
        indices.sort_unstable_by(|&a, &b| {
            let ca = cross(sites[a].side, sites[a].at);
            let cb = cross(sites[b].side, sites[b].at);
            ca.total_cmp(&cb).then(a.cmp(&b))
        });
        let mut anchor = cross(sites[indices[0]].side, sites[indices[0]].at);
        for &i in &indices[1..] {
            work.visit(1)?;
            let value = cross(sites[i].side, sites[i].at);
            if (value - anchor).abs() <= SNAP_EPS && fits(sites[i], anchor) {
                match axis {
                    Axis::Vertical => sites[i].at.x = anchor,
                    Axis::Horizontal => sites[i].at.y = anchor,
                }
            } else {
                anchor = value;
            }
        }
    }
    Ok(())
}

pub(crate) enum Gutters<'a> {
    Architecture(&'a [Vec<f64>; 2]),
    CardGrid(&'a [[Vec<f64>; 2]]),
}

impl Gutters<'_> {
    fn tracks(&self, scope: usize) -> [&[f64]; 2] {
        let axes = match *self {
            Self::Architecture(axes) => axes,
            Self::CardGrid(scopes) => &scopes[scope],
        };
        axes.each_ref().map(Vec::as_slice)
    }
}

fn adapt_auto_titles(
    graph: &Graph,
    sites: &[Site],
    indices: &[usize],
    metrics: Metrics,
    options: &Options,
    drawing: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let margin = metrics.margin();
    let clear = metrics.separation;
    work.visit(indices.len())?;
    let mut top_faces = indices
        .chunk_by(|&a, &b| (sites[a].owner, sites[a].side) == (sites[b].owner, sites[b].side))
        .filter(|face| sites[face[0]].side == Side::Top)
        .peekable();
    for (group_id, (spec, geom)) in graph.groups.iter().zip(&mut drawing.groups).enumerate() {
        work.visit(1)?;
        let owner = graph.nodes.len() + group_id;
        let face = top_faces
            .next_if(|face| sites[face[0]].owner == owner)
            .unwrap_or_default();
        if spec.title_position != TitlePosition::TopAuto {
            continue;
        }
        let Some(title) = spec.title else { continue };
        let Some(current) = geom.title_center else {
            continue;
        };
        let left = geom.rect.x + margin + title.width / 2.0;
        let right = (geom.rect.x + geom.rect.width - margin - title.width / 2.0).max(left);
        let center = geom.rect.center().x.clamp(left, right);
        let candidates = [left, right, center];
        let mut scores = [(0usize, 0usize, 0.0_f64); 3];
        let x0 = geom.rect.x;
        let x1 = geom.rect.x + geom.rect.width;
        let inset = options
            .spacing
            .corner_clearance
            .max(clear)
            .min((x1 - x0) / 2.0);
        work.visit(face.len())?;
        let movable_count = face.iter().filter(|&&i| sites[i].offset.is_none()).count();
        let slots = GroupSlots {
            range: [x0 + inset, x1 - inset],
            count: movable_count,
            pitch: options.spacing.port_pitch,
            split: None,
        };
        let seats = if face.iter().any(|&i| sites[i].straight) {
            let preferred = || {
                face.iter().filter_map(|&i| {
                    let site = sites[i];
                    site.offset.is_none().then_some(site.preferred)
                })
            };
            for _ in 0..candidates.len() {
                work.visit(face.len())?;
            }
            Some(candidates.map(|x| {
                let start = x - title.width / 2.0;
                slots.beside((start - clear, start + title.width + clear), preferred())
            }))
        } else {
            None
        };
        let mut natural = slots.range[0];
        let mut last = [natural; 3];
        let mut rank = 0;
        let half = title.width / 2.0 + clear;
        let face_center = geom.rect.center().x;
        for &i in face {
            work.visit(1)?;
            let site = sites[i];
            let at = if let Some(offset) = site.offset {
                face_center + offset
            } else {
                natural = slots.at(site.preferred, rank, natural);
                natural
            };
            for (index, (detours, hits, shift)) in scores.iter_mut().enumerate() {
                if let Some(seats) = seats
                    && site.offset.is_none()
                {
                    last[index] = seats[index].at(site.preferred, rank, last[index]);
                    let lane_blocked = (site.preferred - candidates[index]).abs() + SNAP_EPS < half;
                    *detours += usize::from(
                        site.straight
                            && (lane_blocked || (last[index] - site.preferred).abs() > SNAP_EPS),
                    );
                }
                let shortfall = half - (at - candidates[index]).abs();
                if shortfall > 0.0 {
                    *hits += 1;
                    *shift += shortfall;
                }
            }
            rank += usize::from(site.offset.is_none());
        }
        let best = candidates
            .into_iter()
            .zip(scores)
            .min_by(|(_, a), (_, b)| {
                a.0.cmp(&b.0)
                    .then(a.1.cmp(&b.1))
                    .then_with(|| a.2.total_cmp(&b.2))
            })
            .expect("three title anchors")
            .0;
        geom.title_center = Some(Point { x: best, ..current });
    }
    Ok(())
}

/// Route against a scope's walls and use its placement gutters when visibility is exhausted.
#[allow(clippy::too_many_arguments)]
pub(crate) fn draw(
    graph: &Graph,
    hierarchy: &Hierarchy,
    edges: &[(Projection, [Side; 2])],
    metrics: Metrics,
    options: &Options,
    drawing: &mut LayoutOutput,
    gutters: Gutters<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    let mut plan = {
        let context = Routing {
            graph,
            hierarchy,
            drawing,
            metrics,
            options,
        };
        let mut plan = Plan::new(&context, work)?;
        for (index, &(projection, sides)) in edges.iter().enumerate() {
            work.visit(1)?;
            plan.edge(&context, projection, sides, index, work)?;
        }
        plan
    };
    let mut indices = site_order(&plan.sites, work)?;
    let mut group_start = 0;
    if options.group_policy.adaptive_title_clearance {
        work.visit(graph.groups.len())?;
        if graph
            .groups
            .iter()
            .any(|group| group.title_position == TitlePosition::TopAuto && group.title.is_some())
        {
            work.visit(indices.len().max(1).ilog2() as usize + 1)?;
            group_start = indices.partition_point(|&i| plan.sites[i].owner < hierarchy.node_count);
            Routing {
                graph,
                hierarchy,
                drawing,
                metrics,
                options,
            }
            .ports(&mut indices[..group_start], &mut plan.sites, work)?;
            plan.mark_straight_entries(hierarchy.node_count, work)?;
            adapt_auto_titles(
                graph,
                &plan.sites,
                &indices[group_start..],
                metrics,
                options,
                drawing,
                work,
            )?;
        }
    }
    let context = Routing {
        graph,
        hierarchy,
        drawing,
        metrics,
        options,
    };
    context.ports(&mut indices[group_start..], &mut plan.sites, work)?;
    snap_collinear_sites(&context, &mut plan.sites, work)?;
    let mut routed = reserved(plan.scopes.len())?;
    let mut has_captions = false;
    for (scope, parts) in plan.scopes.iter().enumerate() {
        work.visit(1)?;
        let mut routes = reserved(parts.len())?;
        if !parts.is_empty() {
            work.charge(parts.len())?;
            let walls = context.walls(scope, work)?;
            let scope_bounds = context.bounds(scope);
            let mut router = OrthoRouter::new(
                &walls,
                metrics.separation,
                options.caps.max_ortho_grid_nodes,
                work,
            )?;
            for part in parts {
                work.visit(1)?;
                let from = plan.sites[part.from.site];
                let to = plan.sites[part.to.site];
                let start = context.lead(from, part.from.inner);
                let end = context.lead(to, part.to.inner);
                let labelled = part.labels.iter().any(Option::is_some);
                has_captions |= labelled;
                let fan = if !labelled {
                    fan_channel(
                        &router,
                        [start, end],
                        [from.lane, to.lane],
                        scope_bounds,
                        work,
                    )?
                } else {
                    None
                };
                let ordinary = if let Some(fan) = fan {
                    Some(copy_points(&fan)?)
                } else {
                    span(
                        &mut router,
                        start,
                        end,
                        scope_bounds,
                        metrics.separation,
                        work,
                    )?
                };
                let middle = if let Some(points) = ordinary {
                    points
                } else {
                    gutter_route(
                        &router,
                        [start, end],
                        scope_bounds,
                        gutters.tracks(scope),
                        work,
                    )?
                    .ok_or(LayoutError::InvalidInput(
                        "no clear orthogonal channel in grid scope",
                    ))?
                };
                let point_count = middle
                    .len()
                    .checked_add(2)
                    .ok_or(WorkError::ArithmeticOverflow)?;
                work.charge(point_count)?;
                let mut points = reserved(point_count)?;
                add_point(&mut points, knot(from.at, start.1.axis()));
                for p in middle {
                    add_point(&mut points, p);
                }
                add_point(&mut points, knot(to.at, end.1.axis()));
                router.record_route(&points, work)?;
                routes.push(points);
            }
        }
        routed.push(routes);
    }
    let mut routes = reserved(edges.len())?;
    let mut stitches = Vec::new();
    for (index, (steps, &(_, sides))) in plan.paths.iter().zip(edges).enumerate() {
        work.visit(1)?;
        let mut points = Vec::<RoutePoint>::new();
        for step in steps {
            let route = &routed[step.scope][step.index];
            let mut first = 0;
            work.charge(route.len())?;
            points
                .try_reserve(route.len())
                .map_err(|_| LayoutError::AllocationLimit)?;
            if let Some(previous) = points.last() {
                if Some(position(*previous)) != route.first().copied().map(position) {
                    return Err(LayoutError::InvalidInput("discontinuous grid group route"));
                }
                let part = plan.scopes[step.scope][step.index];
                let site = plan.sites[part.from.site];
                push(
                    &mut stitches,
                    BorderStitch {
                        edge: EdgeId(index as u32),
                        point: points.len() - 1,
                        group: GroupId((site.owner - hierarchy.node_count) as u32),
                        side: site.side,
                    },
                )?;
                first = 1;
            }
            points.extend_from_slice(&route[first..]);
        }
        routes.push(EdgeGeometry {
            points,
            ports: (sides[0], sides[1]),
            label_center: None,
            start_label_center: None,
            end_label_center: None,
            shared_bus: false,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        });
    }
    drop(routed);
    if has_captions {
        captions::place(&context, &plan, &mut routes, work)?;
    }
    drawing.edges = routes;
    drawing.border_stitches = stitches;
    let origin = normalize(graph, drawing, work)?;
    if matches!(gutters, Gutters::Architecture(_))
        && options.bus_routing
        && options.routing == crate::RoutingStyle::Orthogonal
        && edges.len() >= crate::ortho::comb::MIN_DEGREE
    {
        // Finish the required routes before spending surplus work on shared buses.
        let context = Routing {
            graph,
            hierarchy,
            drawing,
            metrics,
            options,
        };
        for (edge, points) in comb::plan(&context, &plan, origin, work)? {
            let route = &mut drawing.edges[edge];
            route.points = points;
            route.shared_bus = true;
        }
    }
    drawing.truncated_routes = plan.truncated;
    Ok(())
}

fn normalize(
    graph: &Graph,
    drawing: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Point> {
    let mut extent: Option<Bounds> = None;
    let mut include = |bounds| {
        extent = Some(extent.map_or(bounds, |extent| extent.union(bounds)));
    };
    for rect in drawing
        .nodes
        .iter()
        .copied()
        .chain(drawing.groups.iter().map(|group| group.rect))
    {
        work.visit(1)?;
        include(Bounds::of(rect));
    }
    for (edge, spec) in drawing.edges.iter().zip(&graph.edges) {
        work.visit(1)?;
        for &knot in &edge.points {
            work.visit(1)?;
            let at = position(knot);
            include(Bounds::between(at, at));
        }
        for (center, size) in [
            (edge.label_center, spec.label),
            (edge.start_label_center, spec.start_label),
            (edge.end_label_center, spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                include(Bounds::centered(center, size));
            }
        }
    }
    let Some(extent) = extent else {
        return Ok(Point::default());
    };
    let origin = [extent.x[0], extent.y[0]];
    for node in &mut drawing.nodes {
        work.visit(1)?;
        node.x -= origin[0];
        node.y -= origin[1];
    }
    let translate = |p: &mut Point| {
        p.x -= origin[0];
        p.y -= origin[1];
    };
    for group in &mut drawing.groups {
        work.visit(1)?;
        group.rect.x -= origin[0];
        group.rect.y -= origin[1];
        if let Some(title) = &mut group.title_center {
            translate(title);
        }
    }
    for edge in &mut drawing.edges {
        work.visit(1)?;
        for knot in &mut edge.points {
            work.visit(1)?;
            knot.x -= origin[0];
            knot.y -= origin[1];
        }
        for center in [
            &mut edge.label_center,
            &mut edge.start_label_center,
            &mut edge.end_label_center,
        ]
        .into_iter()
        .flatten()
        {
            translate(center);
        }
    }
    drawing.bounds = Size {
        width: extent.x[1] - origin[0],
        height: extent.y[1] - origin[1],
    };
    Ok(point(origin))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{NodeSpec, NoopWorkControl};

    #[test]
    fn snap_collinear_sites_preserves_owning_face_and_scope_bounds() {
        let graph = Graph {
            nodes: vec![
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 20.0,
                    },
                    0,
                ),
                NodeSpec::new(
                    Size {
                        width: 40.0,
                        height: 20.0,
                    },
                    1,
                ),
            ],
            ..Graph::default()
        };
        let options = Options::default();
        let mut control = NoopWorkControl;
        let mut work = Work::new(&mut control);
        let hierarchy = Hierarchy::new(&graph, &mut work).unwrap();
        let metrics = Metrics::new(&options);
        let drawing = LayoutOutput {
            nodes: vec![
                Rect {
                    x: 60.0,
                    y: 0.0,
                    width: 40.0,
                    height: 20.0,
                },
                Rect {
                    x: 100.0,
                    y: 60.0,
                    width: 40.0,
                    height: 20.0,
                },
            ],
            bounds: Size {
                width: 200.0,
                height: 120.0,
            },
            ..LayoutOutput::default()
        };
        let context = Routing {
            graph: &graph,
            hierarchy: &hierarchy,
            drawing: &drawing,
            metrics,
            options: &options,
        };
        let make_site = |owner: usize, side: Side, at: Point| Site {
            owner,
            side,
            preferred: cross(side, at),
            tie_preferred: cross(side, at),
            offset: None,
            inner: false,
            marker: MarkerMetrics::default(),
            reach: 0.0,
            at,
            lane: FaceLane::default(),
            aligned_dist: None,
            straight: false,
            scope: 0,
        };
        // Site 0 lies 5e-10 left of node 1's left edge (`x = 100.0`); snapping site 1 to site 0
        // would push site 1 outside node 1's owning face `[100.0, 140.0]`.
        let mut sites = [
            make_site(
                0,
                Side::Bottom,
                Point {
                    x: 100.0 - 5e-10,
                    y: 20.0,
                },
            ),
            make_site(1, Side::Top, Point { x: 100.0, y: 60.0 }),
            make_site(
                1,
                Side::Top,
                Point {
                    x: 100.0 + 4e-10,
                    y: 60.0,
                },
            ),
        ];
        snap_collinear_sites(&context, &mut sites, &mut work).unwrap();
        assert_eq!(sites[1].at.x, 100.0, "must not snap outside owning face");
        assert_eq!(
            sites[2].at.x, 100.0,
            "in-bounds neighbor should still snap cleanly to the valid anchor"
        );
    }
}
