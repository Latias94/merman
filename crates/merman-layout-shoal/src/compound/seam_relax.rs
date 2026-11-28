//! Remove free elbows around opposing sibling ports after the final routing pass.

mod face_order;
mod projection;

use super::solve::TITLE_CLEAR;
#[cfg(test)]
use crate::Rect;
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, BorderStitch, Endpoint, Graph, GroupId, KnotKind, LayoutError, LayoutOutput, Options,
    Point, PortSideHint, RoutePoint, RoutingStyle, Side, Size,
};
use projection::Lane;
use std::ops::ControlFlow;

const MAX_LANE_SHIFT: f64 = 12.0;

struct Scene<'a> {
    graph: &'a Graph,
    options: &'a Options,
}

#[derive(Clone, Copy)]
struct Travel {
    axis: Axis,
    sign: f64,
}

impl Travel {
    fn from_face(face: Side) -> Self {
        Self {
            axis: face.axis(),
            sign: match face {
                Side::Right | Side::Bottom => 1.0,
                Side::Left | Side::Top => -1.0,
            },
        }
    }

    fn along(self, point: RoutePoint) -> f64 {
        self.sign
            * match self.axis {
                Axis::Horizontal => point.x,
                Axis::Vertical => point.y,
            }
    }

    fn across(self, point: RoutePoint) -> f64 {
        match self.axis {
            Axis::Horizontal => point.y,
            Axis::Vertical => point.x,
        }
    }

    fn on_lane(self, point: RoutePoint, lane: f64) -> RoutePoint {
        match self.axis {
            Axis::Horizontal => RoutePoint { y: lane, ..point },
            Axis::Vertical => RoutePoint { x: lane, ..point },
        }
    }
}

impl Bounds {
    fn route(points: &[RoutePoint], work: &mut Work<'_>) -> LayoutResult<Self> {
        work.visit(points.len())?;
        let first = points[0];
        let mut bounds = Self::centered(
            Point {
                x: first.x,
                y: first.y,
            },
            Size::default(),
        );
        for point in &points[1..] {
            bounds.include(Point {
                x: point.x,
                y: point.y,
            });
        }
        Ok(bounds)
    }

    fn contains_bounds(self, other: Self) -> bool {
        self.x[0] <= other.x[0]
            && self.y[0] <= other.y[0]
            && self.x[1] >= other.x[1]
            && self.y[1] >= other.y[1]
    }
}

enum Obstacle {
    Box,
    Edge(usize),
    Group(GroupId),
}

struct Obstacles(Bvh<(Bounds, Obstacle)>);

impl Obstacles {
    fn new(graph: &Graph, output: &LayoutOutput, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(output.nodes.len())?;
        let mut boxes = reserved(output.nodes.len())?;
        boxes.extend(
            output
                .nodes
                .iter()
                .filter(|node| node.width > 0.0 && node.height > 0.0)
                .map(|&node| (Bounds::of(node), Obstacle::Box)),
        );
        for (index, group) in output.groups.iter().enumerate() {
            work.visit(1)?;
            push(
                &mut boxes,
                (
                    Bounds::of(group.rect),
                    Obstacle::Group(GroupId(index as u32)),
                ),
            )?;
            if let (Some(center), Some(size)) = (
                group.title_center,
                graph.groups.get(index).and_then(|spec| spec.title),
            ) {
                let padding = 2.0 * TITLE_CLEAR;
                let clearance = Size {
                    width: size.width + padding,
                    height: size.height + padding,
                };
                push(
                    &mut boxes,
                    (Bounds::centered(center, clearance), Obstacle::Box),
                )?;
            }
        }
        for (index, (edge, spec)) in output.edges.iter().zip(&graph.edges).enumerate() {
            work.visit(1)?;
            push(
                &mut boxes,
                (Bounds::route(&edge.points, work)?, Obstacle::Edge(index)),
            )?;
            for (center, size) in [
                (edge.label_center, spec.label),
                (edge.start_label_center, spec.start_label),
                (edge.end_label_center, spec.end_label),
            ] {
                if let (Some(center), Some(size)) = (center, size) {
                    push(&mut boxes, (Bounds::centered(center, size), Obstacle::Box))?;
                }
            }
            for (point, marker) in [
                (edge.points[0], spec.start_marker),
                (*edge.points.last().unwrap(), spec.end_marker),
            ] {
                let radius = marker.length.max(marker.width / 2.0);
                if radius > 0.0 {
                    push(
                        &mut boxes,
                        (
                            Bounds::centered(
                                Point {
                                    x: point.x,
                                    y: point.y,
                                },
                                Size {
                                    width: radius * 2.0,
                                    height: radius * 2.0,
                                },
                            ),
                            Obstacle::Box,
                        ),
                    )?;
                }
            }
        }
        Ok(Self(Bvh::build_with_work(boxes, |item| item.0, work)?))
    }

    fn clear(
        &self,
        bounds: Bounds,
        edge: usize,
        groups: [GroupId; 2],
        drawing: &LayoutOutput,
        accepted: impl Iterator<Item = (usize, Bounds)>,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        let blocked = self.0.visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            let (obstacle, owner) = match entry {
                BvhEntry::Node(region) => {
                    return Ok(ControlFlow::Continue(bounds.overlaps(region)));
                }
                BvhEntry::Item((obstacle, owner)) => (*obstacle, owner),
            };
            if !bounds.overlaps(obstacle) {
                return Ok(ControlFlow::Continue(true));
            }
            match *owner {
                Obstacle::Edge(other) if other == edge => return Ok(ControlFlow::Continue(true)),
                Obstacle::Group(group) => {
                    if groups.contains(&group) {
                        return Ok(ControlFlow::Continue(true));
                    }
                    let mut ancestor = drawing.groups[groups[0].0 as usize].parent;
                    while let Some(id) = ancestor {
                        work.visit(1)?;
                        if id == group {
                            return Ok(ControlFlow::Continue(true));
                        }
                        ancestor = drawing.groups[id.0 as usize].parent;
                    }
                }
                _ => {}
            }
            Ok(ControlFlow::Break(()))
        })?;
        if blocked.is_break() {
            return Ok(false);
        }
        for (accepted_edge, footprint) in accepted {
            work.visit(1)?;
            if accepted_edge != edge && bounds.overlaps(footprint) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

fn inboard_track(
    route: &[RoutePoint],
    seam: usize,
    terminal: usize,
    travel: Travel,
    stub_limit: f64,
    orthogonal: bool,
    work: &mut Work<'_>,
) -> LayoutResult<Option<usize>> {
    let next = |index: usize| {
        if terminal < seam {
            index - 1
        } else {
            index + 1
        }
    };
    let lane = travel.across(route[seam]);
    let mut index = seam;
    while next(index) != terminal && travel.across(route[next(index)]) == lane {
        work.visit(1)?;
        index = next(index);
    }
    let first = next(index);
    if first == terminal
        || travel.along(route[first]) != travel.along(route[index])
        || (travel.along(route[seam]) - travel.along(route[index])).abs() > stub_limit
    {
        return Ok(None);
    }
    let track = travel.across(route[first]);
    if (track - lane).abs() > MAX_LANE_SHIFT {
        return Ok(None);
    }
    let mut far = first;
    while next(far) != terminal && travel.across(route[next(far)]) == track {
        work.visit(1)?;
        far = next(far);
    }
    let anchor = next(far);
    let old = route[far];
    let anchor_lane = travel.across(route[anchor]);
    Ok((anchor != terminal
        && (travel.along(old) - travel.along(route[first])).abs() >= stub_limit
        && (orthogonal || travel.along(old) != travel.along(route[anchor]))
        && (lane - anchor_lane) * (track - lane) > 0.0)
        .then_some(far))
}

struct Candidate {
    start: usize,
    end: usize,
    lane: Option<f64>,
    footprint: Bounds,
}

#[derive(Clone, Copy)]
struct SiblingRoute<'a> {
    points: &'a [RoutePoint],
    seams: [BorderStitch; 2],
    travel: Travel,
}

fn between_siblings<'a>(
    ctx: &Scene<'_>,
    output: &'a LayoutOutput,
    [first, last]: [BorderStitch; 2],
    work: &mut Work<'_>,
) -> LayoutResult<Option<SiblingRoute<'a>>> {
    let edge = first.edge.0 as usize;
    let spec = &ctx.graph.edges[edge];
    let (Endpoint::Node(source), Endpoint::Node(target)) = (spec.source, spec.target) else {
        return Ok(None);
    };
    let groups = [first.group, last.group];
    if groups[0] == groups[1]
        || ctx.graph.nodes[source.0 as usize].parent != Some(groups[0])
        || ctx.graph.nodes[target.0 as usize].parent != Some(groups[1])
        || first.side == last.side
        || first.side.axis() != last.side.axis()
        || output.groups[groups[0].0 as usize].parent != output.groups[groups[1].0 as usize].parent
        || matches!(spec.source_port, PortSideHint::FixedOffset { .. })
        || matches!(spec.target_port, PortSideHint::FixedOffset { .. })
    {
        return Ok(None);
    }
    let route = &output.edges[edge].points;
    let [left, right] = [first.point, last.point];
    let travel = Travel::from_face(first.side);
    let outer = &route[left..=right];
    let from = travel.along(route[left]);
    let to = travel.along(route[right]);
    if to <= from || route[right].axis != travel.axis {
        return Ok(None);
    }
    work.charge(route.len())?;
    if route
        .windows(2)
        .any(|pair| travel.along(pair[0]) > travel.along(pair[1]))
        || outer[1..outer.len() - 1]
            .iter()
            .any(|point| point.kind != KnotKind::Free)
    {
        return Ok(None);
    }

    Ok(Some(SiblingRoute {
        points: route,
        seams: [first, last],
        travel,
    }))
}

fn fixed_seams(
    ctx: &Scene<'_>,
    output: &LayoutOutput,
    route: SiblingRoute<'_>,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Candidate>> {
    let SiblingRoute {
        points: route,
        seams: [first, last],
        travel,
    } = route;
    let [left, right] = [first.point, last.point];
    let outer = &route[left..=right];
    let from = travel.along(route[left]);
    let to = travel.along(route[right]);
    let lane = travel.across(route[left]);
    let delta = travel.across(route[right]) - lane;
    if delta != 0.0 {
        if ctx.options.routing == RoutingStyle::Orthogonal {
            return Ok(None);
        }
        let mut steps = outer
            .windows(2)
            .filter(|pair| travel.across(pair[0]) != travel.across(pair[1]));
        if delta.abs() > 2.0 * (to - from)
            || !steps.next().is_some_and(|pair| {
                let along = travel.along(pair[0]);
                from < along && along < to && along == travel.along(pair[1])
            })
            || steps.next().is_some()
        {
            return Ok(None);
        }
        return Ok(Some(Candidate {
            start: left,
            end: right,
            lane: None,
            footprint: Bounds::route(outer, work)?,
        }));
    }
    if outer.iter().any(|&point| travel.across(point) != lane) {
        return Ok(None);
    }
    let stub_limit = ctx.options.spacing.port_sep;
    let orthogonal = ctx.options.routing == RoutingStyle::Orthogonal;
    let Some(start) = inboard_track(route, left, 0, travel, stub_limit, orthogonal, work)? else {
        return Ok(None);
    };
    let Some(end) = inboard_track(
        route,
        right,
        route.len() - 1,
        travel,
        stub_limit,
        orthogonal,
        work,
    )?
    else {
        return Ok(None);
    };
    if travel.across(route[start]) != travel.across(route[end])
        || route[start..left]
            .iter()
            .chain(&route[right + 1..=end])
            .any(|point| point.kind != KnotKind::Free)
        || !Bounds::of(output.groups[first.group.0 as usize].rect)
            .contains_bounds(Bounds::route(&route[start - 1..=left], work)?)
        || !Bounds::of(output.groups[last.group.0 as usize].rect)
            .contains_bounds(Bounds::route(&route[right..=end + 1], work)?)
    {
        return Ok(None);
    }
    Ok(Some(Candidate {
        start,
        end,
        lane: Some(lane),
        footprint: Bounds::route(&route[start - 1..=end + 1], work)?,
    }))
}

type Corridor = [(GroupId, u8); 2];

struct FaceContact {
    face: (GroupId, u8),
    edge: usize,
    point: usize,
    stitched: bool,
}

struct FaceLanes {
    corridors: Vec<Option<Corridor>>,
    accepted: Vec<Option<f64>>,
    contacts: Vec<FaceContact>,
}

impl FaceLanes {
    fn new(graph: &Graph, output: &LayoutOutput, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(output.edges.len())?;
        let mut contacts = reserved(output.border_stitches.len())?;
        for seam in &output.border_stitches {
            work.visit(1)?;
            contacts.push(FaceContact {
                face: (seam.group, seam.side as u8),
                edge: seam.edge.0 as usize,
                point: seam.point,
                stitched: true,
            });
        }
        for (index, (edge, spec)) in output.edges.iter().zip(&graph.edges).enumerate() {
            work.visit(1)?;
            for (end, side, point) in [
                (spec.source, edge.ports.0, 0),
                (spec.target, edge.ports.1, edge.points.len() - 1),
            ] {
                if let Endpoint::Group(id) = end {
                    push(
                        &mut contacts,
                        FaceContact {
                            face: (id, side as u8),
                            edge: index,
                            point,
                            stitched: false,
                        },
                    )?;
                }
            }
        }
        work.charge_sort(contacts.len())?;
        contacts.sort_unstable_by_key(|contact| (contact.face, contact.edge, contact.point));
        work.check()?;
        Ok(Self {
            corridors: filled(output.edges.len(), None)?,
            accepted: filled(output.edges.len(), None)?,
            contacts,
        })
    }

    fn on_face(
        &self,
        group: GroupId,
        side: Side,
        work: &mut Work<'_>,
    ) -> LayoutResult<impl Iterator<Item = &FaceContact> + use<'_>> {
        let face = (group, side as u8);
        work.visit(self.contacts.len().max(1).ilog2() as usize + 1)?;
        let start = self.contacts.partition_point(|contact| contact.face < face);
        Ok(self.contacts[start..]
            .iter()
            .take_while(move |contact| contact.face == face))
    }
}

#[derive(Clone, Copy)]
struct Slide<'a> {
    pair: SiblingRoute<'a>,
    stitch: usize,
    start: usize,
    end: usize,
    preferred: f64,
    corridor: Corridor,
    originals: [f64; 2],
}

impl Slide<'_> {
    fn edge(self) -> usize {
        self.pair.seams[0].edge.0 as usize
    }
}

fn restrict_lane(limits: &mut (f64, f64), lane: f64, neighbor: f64, current: f64, pitch: f64) {
    let gap = lane - neighbor;
    if gap >= 0.0 {
        limits.0 = limits.0.max(current + gap.min(pitch));
    }
    if gap <= 0.0 {
        limits.1 = limits.1.min(current - (-gap).min(pitch));
    }
}

fn lane_limits(
    ctx: &Scene<'_>,
    output: &LayoutOutput,
    slide: Slide<'_>,
    faces: &FaceLanes,
    work: &mut Work<'_>,
) -> LayoutResult<(f64, f64)> {
    let SiblingRoute {
        points,
        seams,
        travel,
    } = slide.pair;
    let mut limits = (f64::NEG_INFINITY, f64::INFINITY);
    let spacing = ctx.options.spacing;
    for seam in seams {
        let rect = output.groups[seam.group.0 as usize].rect;
        let (start, length) = match travel.axis {
            Axis::Horizontal => (rect.y, rect.height),
            Axis::Vertical => (rect.x, rect.width),
        };
        let corner = spacing.corner_clearance.min(length / 2.0);
        let lane = travel.across(points[seam.point]);
        limits.0 = limits.0.max(start + corner).max(lane - MAX_LANE_SHIFT);
        limits.1 = limits
            .1
            .min(start + length - corner)
            .min(lane + MAX_LANE_SHIFT);
        for neighbor in faces.on_face(seam.group, seam.side, work)? {
            work.visit(1)?;
            let edge = neighbor.edge;
            if neighbor.stitched && faces.corridors[edge] == Some(slide.corridor) {
                continue;
            }
            let original = travel.across(output.edges[edge].points[neighbor.point]);
            let current = if neighbor.stitched {
                faces.accepted[edge].unwrap_or(original)
            } else {
                original
            };
            restrict_lane(&mut limits, lane, original, current, spacing.min_port_pitch);
        }
    }
    Ok(limits)
}

fn keeps_anchor(
    fixed: RoutePoint,
    anchor: RoutePoint,
    old_track: RoutePoint,
    lane: f64,
    travel: Travel,
) -> bool {
    let across = travel.across(anchor);
    anchor.kind != KnotKind::Free
        || travel.across(fixed) != across
        || (travel.across(old_track) != across && lane != across)
}

fn prepare_slide<'a>(
    ctx: &Scene<'_>,
    pair: SiblingRoute<'a>,
    stitch: usize,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Slide<'a>>> {
    let SiblingRoute {
        points,
        seams: [first, last],
        travel,
    } = pair;
    let [left, right] = [first.point, last.point];
    let from = travel.across(points[left]);
    let to = travel.across(points[right]);
    if (from - to).abs() > MAX_LANE_SHIFT
        || [points[left], points[right]]
            .iter()
            .any(|point| point.kind != KnotKind::Free || point.axis != travel.axis)
    {
        return Ok(None);
    }
    let terminal = points.len() - 1;
    let stub = ctx.options.spacing.port_sep;
    let orthogonal = ctx.options.routing == RoutingStyle::Orthogonal;
    let start = inboard_track(points, left, 0, travel, stub, orthogonal, work)?.unwrap_or(left - 1);
    let end = inboard_track(points, right, terminal, travel, stub, orthogonal, work)?
        .unwrap_or(right + 1);
    if start < 2 || end + 1 >= terminal {
        return Ok(None);
    }
    let before = points[start - 1];
    let after = points[end + 1];
    let first_track = travel.across(points[start]);
    let last_track = travel.across(points[end]);
    let leads_forward = |a, b| {
        let run = travel.along(b) - travel.along(a);
        run > 0.0 || orthogonal && run == 0.0 && travel.across(a) != travel.across(b)
    };
    if points[start..left]
        .iter()
        .chain(&points[right + 1..=end])
        .any(|p| p.kind != KnotKind::Free)
        || travel.along(points[start - 2]) >= travel.along(before)
        || travel.along(after) >= travel.along(points[end + 2])
        || !keeps_anchor(
            points[start - 2],
            before,
            points[start],
            first_track,
            travel,
        )
        || !keeps_anchor(points[end + 2], after, points[end], last_track, travel)
        || !leads_forward(before, points[start])
        || travel.along(points[start]) >= travel.along(points[left])
        || travel.along(points[right]) >= travel.along(points[end])
        || !leads_forward(points[end], after)
        || !orthogonal && (points[start].axis != travel.axis || after.axis != travel.axis)
    {
        return Ok(None);
    }
    let mut corridor = [
        (first.group, first.side as u8),
        (last.group, last.side as u8),
    ];
    let mut originals = [from, to];
    if corridor[0] > corridor[1] {
        corridor.swap(0, 1);
        originals.swap(0, 1);
    }
    Ok(Some(Slide {
        pair,
        stitch,
        start,
        end,
        preferred: first_track.midpoint(last_track),
        corridor,
        originals,
    }))
}

fn sliding_candidate(
    output: &LayoutOutput,
    slide: Slide<'_>,
    lane: f64,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<Option<Candidate>> {
    let Slide {
        start, end, pair, ..
    } = slide;
    let SiblingRoute {
        points,
        seams: [first, last],
        travel,
    } = pair;
    let before = points[start - 1];
    let after = points[end + 1];
    if !keeps_anchor(points[start - 2], before, points[start], lane, travel)
        || !keeps_anchor(points[end + 2], after, points[end], lane, travel)
    {
        return Ok(None);
    }
    let [left, right] = [first.point, last.point];
    let moved = [
        before,
        travel.on_lane(points[start], lane),
        travel.on_lane(points[left], lane),
        travel.on_lane(points[right], lane),
        travel.on_lane(points[end], lane),
        after,
    ];
    for leg in moved.windows(2) {
        let run = travel.along(leg[1]) - travel.along(leg[0]);
        let shift = (travel.across(leg[1]) - travel.across(leg[0])).abs();
        if run < 0.0 || (style != RoutingStyle::Orthogonal && (run == 0.0 || shift > 2.0 * run)) {
            return Ok(None);
        }
    }
    let [old_left, old_right] = [
        Bounds::route(&points[start - 1..=left], work)?,
        Bounds::route(&points[right..=end + 1], work)?,
    ];
    let left_sweep = old_left.union(Bounds::route(&moved[..3], work)?);
    let right_sweep = old_right.union(Bounds::route(&moved[3..], work)?);
    if !Bounds::of(output.groups[first.group.0 as usize].rect).contains_bounds(left_sweep)
        || !Bounds::of(output.groups[last.group.0 as usize].rect).contains_bounds(right_sweep)
    {
        return Ok(None);
    }
    let old = &points[start - 1..=end + 1];
    Ok(Some(Candidate {
        start,
        end,
        lane: Some(lane),
        footprint: Bounds::route(old, work)?
            .union(left_sweep)
            .union(right_sweep),
    }))
}

fn route_length(route: &[RoutePoint], style: RoutingStyle) -> f64 {
    route
        .windows(2)
        .map(|leg| {
            let x = (leg[1].x - leg[0].x).abs();
            let y = (leg[1].y - leg[0].y).abs();
            if style == RoutingStyle::Orthogonal {
                x + y
            } else {
                x.hypot(y)
            }
        })
        .sum()
}

struct Edit {
    edge: usize,
    stitch: usize,
    point: usize,
    route: Vec<RoutePoint>,
    footprint: Bounds,
}

fn make_edit(
    pair: SiblingRoute<'_>,
    stitch: usize,
    candidate: Candidate,
    style: RoutingStyle,
    work: &mut Work<'_>,
) -> LayoutResult<Edit> {
    let SiblingRoute {
        points,
        seams: [first, last],
        travel,
    } = pair;
    let at = |index| {
        candidate
            .lane
            .map_or(points[index], |lane| travel.on_lane(points[index], lane))
    };
    let orthogonal = style == RoutingStyle::Orthogonal;
    let across_axis = match travel.axis {
        Axis::Horizontal => Axis::Vertical,
        Axis::Vertical => Axis::Horizontal,
    };
    let capacity = points
        .len()
        .checked_add(2 * usize::from(orthogonal))
        .ok_or(crate::WorkError::ArithmeticOverflow)?;
    work.charge(capacity)?;
    let mut route = reserved(capacity)?;
    route.extend_from_slice(&points[..candidate.start]);
    if candidate.start < first.point {
        let mut start = at(candidate.start);
        start.axis = travel.axis;
        let previous = points[candidate.start - 1];
        if orthogonal && travel.across(previous) != travel.across(start) {
            if travel.along(previous) != travel.along(start) {
                route.push(travel.on_lane(start, travel.across(previous)));
            }
            start.axis = across_axis;
        }
        route.push(start);
    }
    let point = route.len();
    route.extend([at(first.point), at(last.point)]);
    let mut tail = &points[candidate.end + 1..];
    if candidate.end > last.point {
        let mut end = at(candidate.end);
        end.axis = travel.axis;
        route.push(end);
        let next = tail[0];
        if orthogonal && travel.across(next) != travel.across(end) {
            if travel.along(next) == travel.along(end) {
                route.push(RoutePoint {
                    axis: across_axis,
                    ..next
                });
                tail = &tail[1..];
            } else {
                route.push(RoutePoint {
                    axis: across_axis,
                    ..travel.on_lane(end, travel.across(next))
                });
            }
        }
    }
    route.extend_from_slice(tail);
    Ok(Edit {
        edge: first.edge.0 as usize,
        stitch,
        point,
        route,
        footprint: candidate.footprint,
    })
}

fn seam_paths(output: &LayoutOutput) -> impl Iterator<Item = (usize, &[BorderStitch])> {
    let mut offset = 0;
    output
        .border_stitches
        .chunk_by(|a, b| a.edge == b.edge)
        .map(move |seams| {
            let start = offset;
            offset += seams.len();
            (start, seams)
        })
}

fn fixed_edits(
    ctx: &Scene<'_>,
    output: &LayoutOutput,
    obstacles: &mut Option<Obstacles>,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Edit>> {
    let mut edits = Vec::new();
    for (stitch, seams) in seam_paths(output) {
        work.visit(seams.len())?;
        let &[first, last] = seams else {
            continue;
        };
        let Some(pair) = between_siblings(ctx, output, [first, last], work)? else {
            continue;
        };
        let Some(candidate) = fixed_seams(ctx, output, pair, work)? else {
            continue;
        };
        if obstacles.is_none() {
            *obstacles = Some(Obstacles::new(ctx.graph, output, work)?);
        }
        let edge = first.edge.0 as usize;
        if !obstacles.as_ref().unwrap().clear(
            candidate.footprint,
            edge,
            [first.group, last.group],
            output,
            edits.iter().map(|edit: &Edit| (edit.edge, edit.footprint)),
            work,
        )? {
            continue;
        }
        push(
            &mut edits,
            make_edit(pair, stitch, candidate, ctx.options.routing, work)?,
        )?;
    }
    Ok(edits)
}

fn slide_edits(
    ctx: &Scene<'_>,
    output: &LayoutOutput,
    fixed: &[Edit],
    obstacles: &mut Option<Obstacles>,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Edit>> {
    let mut slides = Vec::new();
    for (stitch, seams) in seam_paths(output) {
        work.visit(seams.len())?;
        if let &[first, last] = seams
            && let Some(pair) = between_siblings(ctx, output, [first, last], work)?
            && let Some(slide) = prepare_slide(ctx, pair, stitch, work)?
        {
            push(&mut slides, slide)?;
        }
    }
    if slides.is_empty() {
        return Ok(Vec::new());
    }
    let mut order = reserved(slides.len())?;
    order.extend(0..slides.len());
    work.sort(&mut order, |&a, &b| {
        slides[a]
            .corridor
            .cmp(&slides[b].corridor)
            .then_with(|| slides[a].originals[0].total_cmp(&slides[b].originals[0]))
            .then(a.cmp(&b))
    })?;
    let mut faces = FaceLanes::new(ctx.graph, output, work)?;
    for slide in &slides {
        work.visit(1)?;
        faces.corridors[slide.edge()] = Some(slide.corridor);
    }
    let mut lanes = reserved(slides.len())?;
    let mut planned = reserved(slides.len())?;
    let mut edits = reserved(slides.len())?;
    let pitch = ctx.options.spacing.min_port_pitch;
    'corridors: for members in order.chunk_by(|&a, &b| slides[a].corridor == slides[b].corridor) {
        lanes.clear();
        planned.clear();
        let mut previous = None::<[f64; 2]>;
        for &index in members {
            let slide = slides[index];
            let mut gap: f64 = 0.;
            if let Some(before) = previous {
                for (original, prior) in slide.originals.into_iter().zip(before) {
                    if original <= prior {
                        continue 'corridors;
                    }
                    gap = gap.max((original - prior).min(pitch));
                }
            }
            previous = Some(slide.originals);
            let (min, max) = lane_limits(ctx, output, slide, &faces, work)?;
            lanes.push(Lane {
                target: slide.preferred,
                min,
                max,
                gap,
            });
        }
        if !projection::project(&mut lanes, work)? {
            continue;
        }
        for (&index, lane) in members.iter().zip(&lanes) {
            let slide = slides[index];
            if slide.originals == [lane.target; 2] {
                continue;
            }
            let Some(candidate) =
                sliding_candidate(output, slide, lane.target, ctx.options.routing, work)?
            else {
                continue 'corridors;
            };
            if obstacles.is_none() {
                *obstacles = Some(Obstacles::new(ctx.graph, output, work)?);
            }
            let edge = slide.edge();
            let groups = slide.pair.seams.map(|seam| seam.group);
            if !obstacles.as_ref().unwrap().clear(
                candidate.footprint,
                edge,
                groups,
                output,
                fixed
                    .iter()
                    .chain(&edits)
                    .chain(&planned)
                    .map(|edit| (edit.edge, edit.footprint)),
                work,
            )? {
                continue 'corridors;
            }
            let edit = make_edit(
                slide.pair,
                slide.stitch,
                candidate,
                ctx.options.routing,
                work,
            )?;
            let previous = fixed
                .binary_search_by_key(&edge, |fixed| fixed.edge)
                .ok()
                .map_or(slide.pair.points, |index| fixed[index].route.as_slice());
            if route_length(&edit.route, ctx.options.routing)
                > route_length(previous, ctx.options.routing)
            {
                continue 'corridors;
            }
            planned.push(edit);
        }
        for (&index, lane) in members.iter().zip(&lanes) {
            work.visit(1)?;
            faces.accepted[slides[index].edge()] = Some(lane.target);
        }
        edits.append(&mut planned);
    }
    Ok(edits)
}

pub(super) fn eligible(options: &Options, output: &LayoutOutput) -> bool {
    let caps = options.caps;
    output.truncated_routes.is_empty()
        && output.groups.len() >= 2
        && (2..=caps.max_ordered_compound_ports).contains(&output.border_stitches.len())
        && output.edges.len() <= caps.max_ordered_compound_edges
}

pub(super) fn relax(
    graph: &Graph,
    options: &Options,
    output: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if !eligible(options, output) {
        return Ok(());
    }
    let scene = Scene { graph, options };
    face_order::relax(&scene, output, work)?;
    let mut obstacles = None;
    let fixed = match work.compare(|trial| fixed_edits(&scene, output, &mut obstacles, trial)) {
        Ok(Some(edits)) => edits,
        Ok(None) | Err(LayoutError::AllocationLimit) => return Ok(()),
        Err(error) => return Err(error),
    };
    let slid =
        match work.compare(|trial| slide_edits(&scene, output, &fixed, &mut obstacles, trial)) {
            Ok(Some(edits)) => edits,
            Ok(None) | Err(LayoutError::AllocationLimit) => Vec::new(),
            Err(error) => return Err(error),
        };
    for edit in fixed.into_iter().chain(slid) {
        output.edges[edit.edge].points = edit.route;
        output.border_stitches[edit.stitch].point = edit.point;
        output.border_stitches[edit.stitch + 1].point = edit.point + 1;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
