use super::{ArchitectureShoalTitle, TEXT_CLEARANCE};
use crate::Result;
use crate::graph_layout::canonical::{PaintedNodeBounds, RouteContours};
use crate::graph_layout::contour::ShapePortProfile;
use crate::graph_layout::ir::{
    Axis, Endpoint, MeasuredGraph, Point as LayoutPoint, Rect, Side, SolvedGraph,
};
use crate::graph_layout::route_paint::marker_bounds;
use crate::layout_work::OperationLayoutWorkControl;
use crate::model::LayoutLabel;
use kurbo::{Point, Rect as InkRect};
use merman_layout_shoal::RoutePoint;
use std::ops::Range;

fn rect(body: Rect) -> InkRect {
    InkRect::new(body.x, body.y, body.x + body.width, body.y + body.height)
}

fn label_rect(label: &LayoutLabel) -> InkRect {
    InkRect::from_center_size((label.x, label.y), (label.width, label.height))
}

fn point(point: RoutePoint) -> Point {
    Point::new(point.x, point.y)
}

fn shaft_bounds(leg: &[RoutePoint]) -> InkRect {
    InkRect::from_points(point(leg[0]), point(leg[1]))
}

fn overlaps_x(a: InkRect, b: InkRect) -> bool {
    a.x0 < b.x1 && b.x0 < a.x1
}

fn overlaps(a: InkRect, b: InkRect) -> bool {
    overlaps_x(a, b) && a.y0 < b.y1 && b.y0 < a.y1
}

/// A taller caption enlarges both halves of a grid card's routing box. When that
/// makes a top departure step up briefly, the emitted icon can still clear the
/// straight approach to the first border stitch.
fn top_jog(points: &[RoutePoint], reverse: bool, max_step: f64) -> Option<Range<usize>> {
    if points.len() < 6 {
        return None;
    }
    let at = |index| {
        points[if reverse {
            points.len() - 1 - index
        } else {
            index
        }]
    };
    let (contact, lead, turn) = (at(0), at(1), at(2));
    let step = lead.y - turn.y;
    if contact.x != lead.x
        || lead.x != turn.x
        || contact.y <= lead.y
        || step <= 0.0
        || step > max_step
    {
        return None;
    }
    let direction = (at(3).x - turn.x).signum();
    let mut last = 2;
    while last + 2 < points.len() && at(last + 1).y == turn.y {
        if (at(last + 1).x - at(last).x) * direction <= 0.0 {
            return None;
        }
        last += 1;
    }
    let drop = at(last + 1);
    if last == 2 || last + 2 >= points.len() || drop.y != lead.y || drop.x != at(last).x {
        return None;
    }
    Some(if reverse {
        points.len() - 1 - last..points.len() - 2
    } else {
        2..last + 1
    })
}

fn visible_clearance(
    solved: &SolvedGraph,
    nodes: &[PaintedNodeBounds],
    sweep: InkRect,
    work: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    for node in nodes {
        work.charge_adapter(1)?;
        if overlaps(sweep, node.shape) || node.title.is_some_and(|title| overlaps(sweep, title)) {
            return Ok(false);
        }
    }
    for group in &solved.groups {
        work.charge_adapter(1)?;
        let body = rect(group.rect);
        if (overlaps(sweep, body)
            && !(body.x0 <= sweep.x0
                && sweep.x1 <= body.x1
                && body.y0 <= sweep.y0
                && sweep.y1 <= body.y1))
            || group
                .title
                .as_ref()
                .is_some_and(|title| overlaps(sweep, label_rect(title)))
        {
            return Ok(false);
        }
    }
    Ok(true)
}

fn top_lane_is_clear(
    solved: &SolvedGraph,
    edge: usize,
    jog: &Range<usize>,
    sweep: InkRect,
    lane: InkRect,
    work: &mut OperationLayoutWorkControl,
) -> Result<bool> {
    let local_legs = jog.start.saturating_sub(2)..=jog.end;
    for (index, other) in solved.edges.iter().enumerate() {
        work.charge_adapter(other.points.len())?;
        if other
            .label
            .as_ref()
            .is_some_and(|label| overlaps(sweep, label_rect(label)))
        {
            return Ok(false);
        }
        for (leg_index, leg) in other.points.windows(2).enumerate() {
            if index == edge && local_legs.contains(&leg_index) {
                continue;
            }
            // A vertical leg spanning the sweep already crosses the departing route.
            let leg = shaft_bounds(leg);
            if overlaps(lane, leg)
                && !(index != edge && leg.x0 == leg.x1 && leg.y0 <= sweep.y0 && leg.y1 >= sweep.y1)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

fn marker_envelopes(
    graph: &MeasuredGraph,
    contours: RouteContours<'_>,
    group_radius: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<(usize, InkRect)>> {
    let mut envelopes = Vec::new();
    for (i, (edge, measured)) in contours.solved.edges.iter().zip(&graph.edges).enumerate() {
        work.charge_adapter(2)?;
        let [first, .., last] = edge.points.as_slice() else {
            continue;
        };
        let neighbor_at_end = edge.points[edge.points.len() - 2];
        for (endpoint, side, tip, neighbor, mut marker) in [
            (
                edge.source,
                edge.ports.0,
                *first,
                edge.points[1],
                measured.start_marker,
            ),
            (
                edge.target,
                edge.ports.1,
                *last,
                neighbor_at_end,
                measured.end_marker,
            ),
        ] {
            if marker.width == 0.0 {
                continue;
            }
            let (tip, lead) = contours.terminal(endpoint, side, tip, neighbor, |_| group_radius);
            // Architecture draws the arrow from the visible tip with a square extent equal to
            // its width. The shaft trim differs by marker style.
            marker.trim = 0.0;
            marker.depth = marker.width;
            if let Some(bounds) = marker_bounds(tip, lead, marker) {
                envelopes.push((i, bounds));
            }
        }
    }
    Ok(envelopes)
}

pub(super) fn flatten_top_corridors(
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    nodes: &[PaintedNodeBounds],
    (profiles, group_radius): (&[ShapePortProfile], f64),
    max_step: f64,
    stroke_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    let clearance = stroke_outset.max(TEXT_CLEARANCE);
    let mut arrows = None;
    for index in 0..solved.edges.len() {
        work.charge_adapter(1)?;
        for reverse in [false, true] {
            let edge = &solved.edges[index];
            let (endpoint, side) = if reverse {
                (edge.target, edge.ports.1)
            } else {
                (edge.source, edge.ports.0)
            };
            if side != Side::Top || !matches!(endpoint, Endpoint::Node(_)) || edge.label.is_some() {
                continue;
            }
            work.charge_adapter(edge.points.len())?;
            let Some(jog) = top_jog(&edge.points, reverse, max_step) else {
                continue;
            };
            let start = solved
                .border_stitches
                .partition_point(|stitch| stitch.edge < index);
            let end = solved
                .border_stitches
                .partition_point(|stitch| stitch.edge <= index);
            let stitches = &solved.border_stitches[start..end];
            if stitches.is_empty() || stitches.iter().any(|stitch| jog.contains(&stitch.point)) {
                continue;
            }
            let a = edge.points[jog.start - 1];
            let b = edge.points[jog.end];
            let top = edge.points[jog.start].y;
            let sweep = InkRect::new(a.x.min(b.x), top, a.x.max(b.x), a.y);
            let lane = InkRect::from_points(point(a), point(b)).inflate(clearance, clearance);
            if !visible_clearance(solved, nodes, sweep.inflate(clearance, clearance), work)?
                || !top_lane_is_clear(solved, index, &jog, sweep, lane, work)?
            {
                continue;
            }
            if arrows.is_none() {
                arrows = Some(marker_envelopes(
                    graph,
                    RouteContours { solved, profiles },
                    group_radius,
                    work,
                )?);
            }
            let arrows = arrows.as_deref().unwrap_or_default();
            work.charge_adapter(arrows.len())?;
            if arrows
                .iter()
                .any(|&(edge, bounds)| edge != index && overlaps(lane, bounds))
            {
                continue;
            }
            let edge = &mut solved.edges[index];
            edge.points[jog.end].axis = Axis::Horizontal;
            let count = jog.len();
            for stitch in &mut solved.border_stitches[start..end] {
                if stitch.point >= jog.end {
                    stitch.point -= count;
                }
            }
            edge.points.drain(jog);
        }
    }
    Ok(())
}

pub(super) fn node_ink(
    body: Rect,
    title: Option<&ArchitectureShoalTitle>,
    painted_size: f64,
) -> PaintedNodeBounds {
    let center = body.center();
    PaintedNodeBounds {
        shape: InkRect::from_center_size((center.x, center.y), (painted_size, painted_size)),
        title: title.map(|title| {
            let ink = title.text.bounds_at(LayoutPoint {
                x: center.x + title.anchor.x,
                y: center.y + title.anchor.y,
            });
            InkRect::new(ink.min_x, ink.min_y, ink.max_x, ink.max_y)
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_layout::Marker;
    use crate::graph_layout::ir::{
        BorderStitch, Direction, MeasuredEdge, RoutingStyle, SolvedEdge,
    };
    use crate::resources::{OperationWorkMeter, RenderResourcePolicy};
    use std::sync::Arc;

    #[test]
    fn top_corridor_keeps_a_jog_when_only_an_arrow_wing_blocks_the_new_lane() {
        let knot = |x, y| RoutePoint {
            x,
            y,
            axis: Axis::Vertical,
            kind: Default::default(),
        };
        let make_edge = |source, target, ports, points| SolvedEdge {
            source: Endpoint::Node(source),
            target: Endpoint::Node(target),
            ports,
            points,
            label: None,
            start_label: None,
            end_label: None,
            corridor_clearance: 0.0,
            reversed_for_ranking: false,
        };
        let make_scene = || SolvedGraph {
            direction: Direction::Down,
            bounds: Default::default(),
            nodes: vec![Rect::default(); 4],
            node_ranks: Vec::new(),
            groups: Vec::new(),
            edges: vec![
                make_edge(
                    0,
                    1,
                    (Side::Top, Side::Top),
                    vec![
                        knot(30., 30.),
                        knot(30., 10.),
                        knot(30., 0.),
                        knot(100., 0.),
                        knot(100., 10.),
                        knot(100., 60.),
                    ],
                ),
                make_edge(
                    2,
                    3,
                    (Side::Left, Side::Right),
                    vec![knot(85., 18.), knot(60., 18.)],
                ),
            ],
            border_stitches: vec![BorderStitch {
                edge: 0,
                point: 4,
                group: 0,
                side: Side::Right,
            }],
        };
        for (marker, count) in [
            (
                Marker {
                    trim: 12.0,
                    depth: 0.0,
                    width: 12.0,
                },
                6,
            ),
            (Marker::default(), 4),
        ] {
            let mut solved = make_scene();
            let graph = MeasuredGraph {
                direction: solved.direction,
                routing: RoutingStyle::Orthogonal,
                node_spacing: 0.0,
                rank_spacing: 0.0,
                component_spacing: 0.0,
                inherit_crossed_group_direction: false,
                nodes: Vec::new(),
                groups: Vec::new(),
                edges: solved
                    .edges
                    .iter()
                    .enumerate()
                    .map(|(index, edge)| MeasuredEdge {
                        source: edge.source,
                        target: edge.target,
                        min_rank_span: 1,
                        weight: 1.0,
                        label: None,
                        start_label: None,
                        end_label: None,
                        start_marker: if index == 1 {
                            marker
                        } else {
                            Marker::default()
                        },
                        end_marker: Marker::default(),
                    })
                    .collect(),
            };
            let profiles = std::array::from_fn::<_, 4, _>(|_| ShapePortProfile::rectangle(0., 0.));
            let mut work = OperationLayoutWorkControl::new(Arc::new(OperationWorkMeter::new(
                RenderResourcePolicy::constrained(),
            )));
            flatten_top_corridors(
                &graph,
                &mut solved,
                &[],
                (&profiles, 0.0),
                12.0,
                0.0,
                &mut work,
            )
            .unwrap();
            assert_eq!(solved.edges[0].points.len(), count);
        }
    }
}

#[derive(Default)]
struct ScopeMembers {
    nodes: Vec<usize>,
    groups: Vec<usize>,
}

impl ScopeMembers {
    fn boxes<'a>(&'a self, solved: &'a SolvedGraph) -> impl Iterator<Item = InkRect> + 'a {
        self.nodes
            .iter()
            .map(|&i| rect(solved.nodes[i]))
            .chain(self.groups.iter().map(|&i| rect(solved.groups[i].rect)))
    }
}

pub(super) fn compact_empty_groups(
    graph: &MeasuredGraph,
    solved: &mut SolvedGraph,
    gap: f64,
    route_outset: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    if !solved.groups.iter().any(|group| group.parent.is_some()) {
        return Ok(());
    }
    let mut members: Vec<_> = (0..solved.groups.len())
        .map(|_| ScopeMembers::default())
        .collect();
    for (index, node) in graph.nodes.iter().enumerate() {
        work.charge_adapter(1)?;
        if let Some(parent) = node.parent {
            members[parent as usize].nodes.push(index);
        }
    }
    let mut empty = Vec::new();
    for (index, group) in solved.groups.iter().enumerate() {
        work.charge_adapter(1)?;
        if let Some(parent) = group.parent {
            members[parent as usize].groups.push(index);
        }
    }
    for (index, scope) in members.iter().enumerate() {
        if scope.nodes.is_empty()
            && scope.groups.is_empty()
            && let Some(parent) = solved.groups[index].parent
        {
            empty.push((index, parent as usize));
        }
    }
    if empty.is_empty() {
        return Ok(());
    }
    work.charge_adapter(work.checked_mul(empty.len(), empty.len().ilog2() as usize + 1)?)?;
    empty.sort_unstable_by(|&(a, _), &(b, _)| {
        solved.groups[a]
            .rect
            .y
            .total_cmp(&solved.groups[b].rect.y)
            .then(a.cmp(&b))
    });

    let mut routes = Vec::new();
    let mut route_bottom = 0.0_f64;
    for edge in &solved.edges {
        work.charge_adapter(edge.points.len())?;
        for bounds in edge
            .points
            .windows(2)
            .map(shaft_bounds)
            .chain(edge.label.as_ref().map(label_rect))
        {
            route_bottom = route_bottom.max(bounds.y1);
            routes.push(bounds.inflate(route_outset, route_outset));
        }
    }
    // Empty leaves inherit global row heights even when the tall item is outside their scope.
    // Slide them into the available space above, keeping their header and child-scope gap.
    let mut original_bottom = vec![None; members.len()];
    for (index, parent) in empty {
        work.charge_adapter(
            work.checked_add(members[parent].nodes.len(), members[parent].groups.len())?,
        )?;
        let group = rect(solved.groups[index].rect);
        let outer = &solved.groups[parent];
        let mut top = outer.rect.y + graph.groups[parent].padding.y0;
        if let Some(title) = &outer.title {
            top = top.max(label_rect(title).y1 + TEXT_CLEARANCE);
        }
        let mut content_bottom = group.y1;
        let sweep = group.inflate(TEXT_CLEARANCE, 0.0);
        for sibling in members[parent].boxes(solved) {
            content_bottom = content_bottom.max(sibling.y1);
            if sibling != group && sibling.y0 < group.y0 && overlaps_x(sweep, sibling) {
                top = top.max(sibling.y1 + gap);
            }
        }
        if top >= group.y0 {
            continue;
        }
        work.charge_adapter(routes.len())?;
        for &route in &routes {
            if route.y0 < group.y1 && route.y1 > top && overlaps_x(sweep, route) {
                top = top.max(route.y1 + TEXT_CLEARANCE);
            }
        }
        if top < group.y0 {
            original_bottom[parent].get_or_insert(content_bottom);
            solved.groups[index].rect.y = top;
            if let Some(title) = &mut solved.groups[index].title {
                title.y += top - group.y0;
            }
        }
    }
    if !original_bottom.iter().any(Option::is_some) {
        return Ok(());
    }

    for (index, old_content_bottom) in original_bottom.into_iter().enumerate() {
        let Some(old_content_bottom) = old_content_bottom else {
            continue;
        };
        work.charge_adapter(
            work.checked_add(members[index].nodes.len(), members[index].groups.len())?,
        )?;
        let group = rect(solved.groups[index].rect);
        let content_bottom = members[index]
            .boxes(solved)
            .map(|body| body.y1)
            .fold(group.y0, f64::max);
        let mut bottom = (group.y1 - (old_content_bottom - content_bottom))
            .max(group.y0 + graph.groups[index].min_size.height)
            .min(group.y1);
        if bottom >= group.y1 {
            continue;
        }
        // Preserve any shaft, caption, or border contact that uses the bottom of the scope.
        work.charge_adapter(routes.len())?;
        for &route in &routes {
            if route.y0 <= group.y1 && route.y1 > bottom && overlaps_x(group, route) {
                bottom = bottom.max(route.y1.min(group.y1));
            }
        }
        solved.groups[index].rect.height = bottom - group.y0;
    }
    work.charge_adapter(work.checked_add(solved.nodes.len(), solved.groups.len())?)?;
    solved.bounds.height = solved
        .nodes
        .iter()
        .copied()
        .chain(solved.groups.iter().map(|group| group.rect))
        .map(|body| body.y + body.height)
        .fold(route_bottom, f64::max);
    Ok(())
}
