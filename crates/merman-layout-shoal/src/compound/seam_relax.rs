//! Remove free elbows around opposing sibling ports after the final routing pass.

mod projection;

use super::Pass;
use super::solve::TITLE_CLEAR;
#[cfg(test)]
use crate::Rect;
use crate::geom::Bounds;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, BorderStitch, Endpoint, Graph, GroupId, KnotKind, LayoutError, LayoutOutput, Options,
    Point, PortSideHint, RoutePoint, RoutingStyle, Side, Size,
};
use projection::Lane;

const MAX_LANE_SHIFT: f64 = 12.0;

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

struct Obstacles {
    boxes: Vec<Bounds>,
    edges: Vec<Bounds>,
}

impl Obstacles {
    fn new(graph: &Graph, output: &LayoutOutput, work: &mut Work<'_>) -> LayoutResult<Self> {
        work.charge(output.nodes.len())?;
        let mut boxes = reserved(output.nodes.len())?;
        boxes.extend(
            output
                .nodes
                .iter()
                .filter(|node| node.width > 0.0 && node.height > 0.0)
                .map(|&node| Bounds::of(node)),
        );
        for (group, spec) in output.groups.iter().zip(&graph.groups) {
            work.visit(1)?;
            if let (Some(center), Some(size)) = (group.title_center, spec.title) {
                let padding = 2.0 * TITLE_CLEAR;
                let clearance = Size {
                    width: size.width + padding,
                    height: size.height + padding,
                };
                push(&mut boxes, Bounds::centered(center, clearance))?;
            }
        }
        let mut edges = reserved(output.edges.len())?;
        for (edge, spec) in output.edges.iter().zip(&graph.edges) {
            work.visit(1)?;
            edges.push(Bounds::route(&edge.points, work)?);
            for (center, size) in [
                (edge.label_center, spec.label),
                (edge.start_label_center, spec.start_label),
                (edge.end_label_center, spec.end_label),
            ] {
                if let (Some(center), Some(size)) = (center, size) {
                    push(&mut boxes, Bounds::centered(center, size))?;
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
                    )?;
                }
            }
        }
        Ok(Self { boxes, edges })
    }

    fn clear<'a>(
        &self,
        bounds: Bounds,
        edge: usize,
        groups: [GroupId; 2],
        drawing: &LayoutOutput,
        accepted: impl Iterator<Item = &'a Edit>,
        work: &mut Work<'_>,
    ) -> LayoutResult<bool> {
        for &obstacle in &self.boxes {
            work.visit(1)?;
            if bounds.overlaps(obstacle) {
                return Ok(false);
            }
        }
        for (other, &obstacle) in self.edges.iter().enumerate() {
            work.visit(1)?;
            if other != edge && bounds.overlaps(obstacle) {
                return Ok(false);
            }
        }
        for edit in accepted {
            work.visit(1)?;
            if edit.edge != edge && bounds.overlaps(edit.footprint) {
                return Ok(false);
            }
        }
        for (index, group) in drawing.groups.iter().enumerate() {
            work.visit(1)?;
            if !bounds.overlaps(Bounds::of(group.rect)) || groups.contains(&GroupId(index as u32)) {
                continue;
            }
            let mut ancestor = drawing.groups[groups[0].0 as usize].parent;
            while let Some(id) = ancestor {
                work.visit(1)?;
                if id.0 as usize == index {
                    break;
                }
                ancestor = drawing.groups[id.0 as usize].parent;
            }
            if ancestor.is_none() {
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
        && travel.along(old) != travel.along(route[anchor])
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
    ctx: &Pass<'_>,
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
    ctx: &Pass<'_>,
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
    let Some(start) = inboard_track(route, left, 0, travel, stub_limit, work)? else {
        return Ok(None);
    };
    let Some(end) = inboard_track(route, right, route.len() - 1, travel, stub_limit, work)? else {
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

struct FaceLanes {
    corridors: Vec<Option<Corridor>>,
    accepted: Vec<Option<f64>>,
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
    ctx: &Pass<'_>,
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
        for neighbor in &output.border_stitches {
            work.visit(1)?;
            if neighbor.group != seam.group || neighbor.side != seam.side {
                continue;
            }
            let edge = neighbor.edge.0 as usize;
            if faces.corridors[edge] == Some(slide.corridor) {
                continue;
            }
            let original = travel.across(output.edges[edge].points[neighbor.point]);
            let current = faces.accepted[edge].unwrap_or(original);
            restrict_lane(&mut limits, lane, original, current, spacing.min_port_pitch);
        }
        for (edge, spec) in output.edges.iter().zip(&ctx.graph.edges) {
            work.visit(1)?;
            let endpoint = Endpoint::Group(seam.group);
            let last = edge.points.len() - 1;
            for (end, side, point) in [
                (spec.source, edge.ports.0, edge.points[0]),
                (spec.target, edge.ports.1, edge.points[last]),
            ] {
                if end == endpoint && side == seam.side {
                    let neighbor = travel.across(point);
                    restrict_lane(
                        &mut limits,
                        lane,
                        neighbor,
                        neighbor,
                        spacing.min_port_pitch,
                    );
                }
            }
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
    ctx: &Pass<'_>,
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
    let start = inboard_track(points, left, 0, travel, stub, work)?.unwrap_or(left - 1);
    let end = inboard_track(points, right, terminal, travel, stub, work)?.unwrap_or(right + 1);
    if start < 2 || end + 1 >= terminal {
        return Ok(None);
    }
    let before = points[start - 1];
    let after = points[end + 1];
    let first_track = travel.across(points[start]);
    let last_track = travel.across(points[end]);
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
        || travel.along(before) >= travel.along(points[start])
        || travel.along(points[start]) >= travel.along(points[left])
        || travel.along(points[right]) >= travel.along(points[end])
        || travel.along(points[end]) >= travel.along(after)
        || points[start].axis != travel.axis
        || after.axis != travel.axis
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
        if run <= 0.0 || shift > 2.0 * run {
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

fn route_length(route: &[RoutePoint]) -> f64 {
    route
        .windows(2)
        .map(|leg| (leg[1].x - leg[0].x).hypot(leg[1].y - leg[0].y))
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
    work.charge(points.len())?;
    let mut route = reserved(points.len())?;
    route.extend_from_slice(&points[..candidate.start]);
    if candidate.start < first.point {
        route.push(RoutePoint {
            axis: travel.axis,
            ..at(candidate.start)
        });
    }
    let point = route.len();
    route.extend([at(first.point), at(last.point)]);
    if candidate.end > last.point {
        route.push(RoutePoint {
            axis: travel.axis,
            ..at(candidate.end)
        });
    }
    route.extend_from_slice(&points[candidate.end + 1..]);
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
    ctx: &Pass<'_>,
    output: &LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<Vec<Edit>> {
    let mut edits = Vec::new();
    let mut obstacles = None;
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
            obstacles = Some(Obstacles::new(ctx.graph, output, work)?);
        }
        let edge = first.edge.0 as usize;
        if !obstacles.as_ref().unwrap().clear(
            candidate.footprint,
            edge,
            [first.group, last.group],
            output,
            edits.iter(),
            work,
        )? {
            continue;
        }
        push(&mut edits, make_edit(pair, stitch, candidate, work)?)?;
    }
    Ok(edits)
}

fn slide_edits(
    ctx: &Pass<'_>,
    output: &LayoutOutput,
    fixed: &[Edit],
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
    })?;
    work.charge(output.edges.len())?;
    let mut faces = FaceLanes {
        corridors: filled(output.edges.len(), None)?,
        accepted: filled(output.edges.len(), None)?,
    };
    for slide in &slides {
        work.visit(1)?;
        faces.corridors[slide.edge()] = Some(slide.corridor);
    }
    let mut lanes = reserved(slides.len())?;
    let mut planned = reserved(slides.len())?;
    let mut edits = reserved(slides.len())?;
    let mut obstacles = None;
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
            let Some(candidate) = sliding_candidate(output, slide, lane.target, work)? else {
                continue 'corridors;
            };
            if obstacles.is_none() {
                obstacles = Some(Obstacles::new(ctx.graph, output, work)?);
            }
            let edge = slide.edge();
            let groups = slide.pair.seams.map(|seam| seam.group);
            if !obstacles.as_ref().unwrap().clear(
                candidate.footprint,
                edge,
                groups,
                output,
                fixed.iter().chain(&edits).chain(&planned),
                work,
            )? {
                continue 'corridors;
            }
            let edit = make_edit(slide.pair, slide.stitch, candidate, work)?;
            let previous = fixed
                .binary_search_by_key(&edge, |fixed| fixed.edge)
                .ok()
                .map_or(slide.pair.points, |index| fixed[index].route.as_slice());
            if route_length(&edit.route) > route_length(previous) {
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
    options.routing == RoutingStyle::Curved
        && output.truncated_routes.is_empty()
        && output.groups.len() >= 2
        && (2..=caps.max_ordered_compound_ports).contains(&output.border_stitches.len())
        && output.edges.len() <= caps.max_ordered_compound_edges
}

pub(super) fn relax(
    ctx: &Pass<'_>,
    output: &mut LayoutOutput,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if !eligible(ctx.options, output) {
        return Ok(());
    }
    let fixed = match work.compare(|trial| fixed_edits(ctx, output, trial)) {
        Ok(Some(edits)) => edits,
        Ok(None) | Err(LayoutError::AllocationLimit) => return Ok(()),
        Err(error) => return Err(error),
    };
    let slid = match work.compare(|trial| slide_edits(ctx, output, &fixed, trial)) {
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
