//! A two-way orthogonal decision may exit from its side vertices into clear lanes.

use super::super::{Component, Drawing, Expanded, canonical_size, contour_inset, diamond_offset};
use super::LEAD;
use crate::geom::{Bounds, Bvh, BvhEntry};
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{
    Axis, Direction, Graph, MarkerMetrics, Options, Point, PortSideHint, RoutePoint, RoutingStyle,
    Side, Size,
};
use std::ops::ControlFlow::{Break, Continue};

#[derive(Clone, Copy)]
enum Owner {
    Node {
        index: usize,
        diamond: Option<Bounds>,
    },
    Edge(usize),
}

type Wall = (Bounds, Owner);

fn point(knot: RoutePoint) -> Point {
    Point {
        x: knot.x,
        y: knot.y,
    }
}

fn contact(
    drawing: &Drawing,
    component: &Component,
    direction: Direction,
    node: usize,
    port: Side,
    knot: RoutePoint,
) -> Point {
    let mut at = point(knot);
    let center = drawing.nodes[node].center();
    let offset = match port.axis() {
        Axis::Horizontal => at.y - center.y,
        Axis::Vertical => at.x - center.x,
    };
    let inset = contour_inset(component.nodes[node], direction, port, offset);
    match port {
        Side::Top => at.y += inset,
        Side::Bottom => at.y -= inset,
        Side::Left => at.x += inset,
        Side::Right => at.x -= inset,
    }
    at
}

fn hits_diamond(label: Bounds, diamond: Bounds) -> bool {
    let distance = |[a, b]: [f64; 2], [low, high]: [f64; 2]| {
        let center = a.midpoint(b);
        (center - center.clamp(low, high)).abs() / ((b - a) * 0.5)
    };
    distance(diamond.x, label.x) + distance(diamond.y, label.y) < 1.0
}

fn turn(previous: Point, bend: Point, next: Point, max_radius: f64) -> Option<Bounds> {
    let (mut horizontal, mut vertical) = (previous, next);
    if previous.x == bend.x && next.y == bend.y {
        (horizontal, vertical) = (next, previous);
    }
    if horizontal.y != bend.y || vertical.x != bend.x {
        return None;
    }
    let dx = horizontal.x - bend.x;
    let dy = vertical.y - bend.y;
    let radius = dx.abs().min(dy.abs()).min(max_radius);
    (radius > 0.0).then(|| {
        Bounds::between(
            bend,
            Point {
                x: bend.x + radius.copysign(dx),
                y: bend.y + radius.copysign(dy),
            },
        )
    })
}

struct Dock {
    edge: usize,
    target: usize,
    side: Side,
    points: [RoutePoint; 3],
    start: Point,
    end: Point,
    markers: [MarkerMetrics; 2],
    label: Option<Size>,
}

#[derive(Clone, Copy)]
struct Label {
    at: Point,
    padded: Bounds,
    distance: f64,
}

impl Dock {
    fn legs(&self) -> [f64; 2] {
        let bend = point(self.points[1]);
        [(bend.x - self.start.x).abs(), self.end.y - bend.y]
    }

    fn bend_radius(&self) -> f64 {
        let [horizontal, vertical] = self.legs();
        // The painter rounds within the straight arms left between the terminal leads.
        let [start, end] = self.markers.map(|marker| marker.length.max(LEAD));
        (horizontal - start).min(vertical - end)
    }

    fn marker_boxes(&self) -> impl Iterator<Item = Bounds> {
        [
            Bounds::marker_footprint(self.start, self.side, self.markers[0]),
            Bounds::marker_footprint(self.end, Side::Top, self.markers[1]),
        ]
        .into_iter()
        .flatten()
    }

    fn ink(&self) -> impl Iterator<Item = Bounds> {
        let bend = point(self.points[1]);
        [
            Some(Bounds::between(self.start, bend)),
            Some(Bounds::between(bend, self.end)),
            turn(self.start, bend, self.end, self.bend_radius()),
        ]
        .into_iter()
        .flatten()
        .chain(self.marker_boxes())
    }

    fn labels(
        &self,
        sibling: &Self,
        tree: &Bvh<Wall>,
        pair: [usize; 2],
        pad: f64,
        work: &mut Work<'_>,
    ) -> LayoutResult<impl Iterator<Item = Option<Label>> + Clone + use<>> {
        let mut labels = [None; 8];
        // Inline labels without a center already have a later placement pass for the new route.
        if let Some(size) = self.label {
            let (start, bend) = (self.start, point(self.points[1]));
            let [horizontal, vertical] = self.legs();
            let first = self.markers[0].length;
            let last = horizontal + vertical - self.markers[1].length;
            let middle = first.midpoint(last);
            let radius = self.bend_radius();
            let extents = [size.width * 0.5 + pad, size.height * 0.5 + pad];
            for (leg, ([a, b], half)) in [[first, horizontal - radius], [horizontal + radius, last]]
                .into_iter()
                .zip(extents)
                .enumerate()
            {
                let (low, high) = (a + half, b - half);
                if low > high || !low.is_finite() || !high.is_finite() {
                    continue;
                }
                for (slot, d) in labels[leg * 4..leg * 4 + 4].iter_mut().zip([
                    middle.clamp(low, high),
                    low.midpoint(high),
                    low,
                    high,
                ]) {
                    let at = if leg == 0 {
                        Point {
                            x: start.x + d.copysign(bend.x - start.x),
                            y: start.y,
                        }
                    } else {
                        Point {
                            x: bend.x,
                            y: bend.y + (d - horizontal),
                        }
                    };
                    let padded = Bounds::from_half_extents(at, extents[0], extents[1]);
                    if padded.finite()
                        && padded.has_area()
                        && self
                            .marker_boxes()
                            .chain(sibling.ink())
                            .all(|ink| !padded.overlaps_strictly(ink))
                        && clear(tree, padded, pair, None, work)?
                    {
                        *slot = Some(Label {
                            at,
                            padded,
                            distance: (d - middle).abs(),
                        });
                    }
                }
            }
        }
        Ok(labels
            .into_iter()
            .flatten()
            .map(Some)
            .chain(self.label.is_none().then_some(None)))
    }
}

pub(in crate::flat) fn dock(
    drawing: &mut Drawing,
    component: &Component,
    expanded: &Expanded,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if options.routing != RoutingStyle::Orthogonal
        || component.boundaries
        || component.group_proxies
    {
        return Ok(());
    }
    let direction = graph.direction;
    work.visit(component.nodes.len())?;
    if !component
        .nodes
        .iter()
        .any(|node| diamond_offset(node.contour, direction).is_some())
    {
        return Ok(());
    }
    work.charge(component.nodes.len())?;
    let mut branches = filled(component.nodes.len(), ([0; 2], 0usize))?;
    for (i, link) in component.links.iter().enumerate() {
        work.visit(1)?;
        let (pair, count) = &mut branches[link.from as usize];
        if *count < 2 {
            pair[*count] = i;
        }
        *count = count.saturating_add(1);
    }
    for edge in &component.loops {
        work.visit(1)?;
        branches[edge.node as usize].1 = usize::MAX;
    }
    let mut index = None;
    'decision: for (node, (mut pair, degree)) in branches.into_iter().enumerate() {
        work.visit(1)?;
        if degree != 2 {
            continue;
        }
        let Some(offset) = diamond_offset(component.nodes[node].contour, direction) else {
            continue;
        };
        let source = drawing.nodes[node];
        let box_bottom = source.y + source.height;
        let y = source.center().y + offset.y;
        // The final box-face snap must preserve the vertex's transverse coordinate.
        if !(source.y..=box_bottom).contains(&y) {
            continue;
        }
        let bottom = box_bottom + offset.y.max(0.0);
        let target = |i: usize| component.links[i].to as usize;
        let ports = pair.map(|i| drawing.edges[i].1.points.last().copied());
        let [Some(a), Some(b)] = ports else { continue };
        if a.x > b.x {
            pair.swap(0, 1);
        }
        let candidates = |i: usize, side: Side| {
            let target = target(i);
            let (id, edge) = &drawing.edges[i];
            let spec = &graph.edges[*id as usize];
            if expanded.direct[i]
                || expanded.rank[target] <= expanded.rank[node]
                || spec.source_port != PortSideHint::Auto
                || spec.target_port != PortSideHint::Auto
                || edge.ports != (Side::Bottom, Side::Top)
                || edge.reversed_for_ranking
                || spec.start_label.is_some()
                || spec.end_label.is_some()
            {
                return None;
            }
            let mut end = *edge.points.last()?;
            let (x, outward) = if side == Side::Left {
                (source.x, -1.0)
            } else {
                (source.x + source.width, 1.0)
            };
            let first = RoutePoint::new(x, y, Axis::Horizontal);
            let start = contact(drawing, component, direction, node, side, first);
            if !start.x.is_finite() {
                return None;
            }
            let lead = spec.start_marker.length + LEAD;
            // Leave room beyond the painted vertex and keep the box-face leg outward too.
            let min_reach = lead.max(outward * (x - start.x));
            let has_room = |lane| outward * (lane - start.x) > min_reach;
            let target_rect = drawing.nodes[target];
            if !has_room(end.x) {
                let inset = spec.end_marker.width.max(options.spacing.edge) * 0.5;
                let lane = (min_reach + options.spacing.edge * 0.5).mul_add(outward, start.x);
                if (lane - end.x).abs() > options.spacing.edge
                    || lane < target_rect.x + inset
                    || lane > target_rect.x + target_rect.width - inset
                {
                    return None;
                }
                // A wide diamond may leave a short arm at a centered arrival. Use a
                // nearby contact on the same face, subject to the normal obstacle checks.
                end.x = lane;
            }
            if let Some(offset) = diamond_offset(component.nodes[target].contour, direction)
                && (end.x - target_rect.center().x - offset.x).abs() > target_rect.width * 0.5
            {
                return None;
            }
            let painted_end = contact(drawing, component, direction, target, Side::Top, end);
            let arrival_y = end.y.min(painted_end.y);
            if !has_room(end.x)
                || !painted_end.y.is_finite()
                || arrival_y <= bottom + spec.end_marker.length
                || arrival_y - y <= spec.end_marker.length + LEAD
            {
                return None;
            }
            Some(Dock {
                edge: i,
                target,
                side,
                points: [first, RoutePoint::new(end.x, y, Axis::Horizontal), end],
                start,
                end: painted_end,
                markers: [spec.start_marker, spec.end_marker],
                label: edge
                    .label_center
                    .and(spec.label)
                    .map(|size| canonical_size(direction, size)),
            })
        };
        let (Some(left), Some(right)) = (
            candidates(pair[0], Side::Left),
            candidates(pair[1], Side::Right),
        ) else {
            continue;
        };
        // Use the current route inventory each time a pair is committed. Two new pairs
        // sharing a corridor must also prove clearance against one another.
        if index.is_none() {
            index = Some(walls(drawing, component, graph, work)?);
        }
        let tree = index.as_ref().expect("decision clearance index");
        for branch in [&left, &right] {
            for bounds in branch.ink() {
                if !clear(tree, bounds, pair, Some((node, branch.target)), work)? {
                    continue 'decision;
                }
            }
        }
        if left.ink().any(|a| right.ink().any(|b| a.overlaps(b))) {
            continue;
        }
        let pad = (options.spacing.edge * 0.35).max(2.0);
        let right_labels = right.labels(&left, tree, pair, pad, work)?;
        let mut best = None;
        for a in left.labels(&right, tree, pair, pad, work)? {
            for b in right_labels.clone() {
                work.visit(1)?;
                if a.zip(b).is_some_and(|(a, b)| a.padded.overlaps(b.padded)) {
                    continue;
                }
                let score =
                    a.map_or(0.0, |label| label.distance) + b.map_or(0.0, |label| label.distance);
                if best.is_none_or(|(previous, _)| score < previous) {
                    best = Some((score, [a, b]));
                }
            }
        }
        let Some((_, labels)) = best else { continue };
        for (branch, label) in [left, right].into_iter().zip(labels) {
            let route = &mut drawing.edges[branch.edge].1;
            route.points.clear();
            route.points.extend(branch.points);
            route.ports.0 = branch.side;
            route.label_center = label.map(|label| label.at);
            route.corridor_clearance = 0.0;
        }
        index = None;
    }
    Ok(())
}

fn clear(
    tree: &Bvh<Wall>,
    bounds: Bounds,
    pair: [usize; 2],
    endpoints: Option<(usize, usize)>,
    work: &mut Work<'_>,
) -> LayoutResult<bool> {
    Ok(tree
        .visit(|entry| -> LayoutResult<_> {
            work.visit(1)?;
            Ok(match entry {
                BvhEntry::Node(wall) => Continue(wall.overlaps(bounds)),
                BvhEntry::Item((wall, owner)) => {
                    let check = match owner {
                        Owner::Node { index, .. } => endpoints
                            .is_none_or(|(source, target)| *index != source && *index != target),
                        Owner::Edge(i) => !pair.contains(i),
                    };
                    let overlaps = if endpoints.is_some() {
                        wall.overlaps(bounds)
                    } else {
                        wall.overlaps_strictly(bounds)
                            && match owner {
                                Owner::Node {
                                    diamond: Some(diamond),
                                    ..
                                } => hits_diamond(bounds, *diamond),
                                _ => true,
                            }
                    };
                    if check && overlaps {
                        Break(())
                    } else {
                        Continue(true)
                    }
                }
            })
        })?
        .is_continue())
}

fn walls(
    drawing: &Drawing,
    component: &Component,
    graph: &Graph,
    work: &mut Work<'_>,
) -> LayoutResult<Bvh<Wall>> {
    work.charge(drawing.nodes.len())?;
    let mut walls = reserved(drawing.nodes.len())?;
    walls.extend(drawing.nodes.iter().enumerate().map(|(index, &node)| {
        let bounds = Bounds::of(node);
        let diamond =
            diamond_offset(component.nodes[index].contour, graph.direction).map(|offset| Bounds {
                x: bounds.x.map(|x| x + offset.x),
                y: bounds.y.map(|y| y + offset.y),
            });
        // Shaft checks use a conservative envelope. Labels can occupy empty diamond corners.
        let envelope = diamond.map_or(bounds, |diamond| bounds.union(diamond));
        let diamond = diamond.filter(|diamond| diamond.finite() && diamond.has_area());
        (envelope, Owner::Node { index, diamond })
    }));
    let mut knots: Vec<Point> = Vec::new();
    for (index, (id, route)) in drawing.edges.iter().enumerate() {
        let spec = &graph.edges[*id as usize];
        let owner = Owner::Edge(index);
        let [source, target] = if let Some(link) = component.links.get(index) {
            [link.from as usize, link.to as usize]
        } else {
            [component.loops[index - component.links.len()].node as usize; 2]
        };
        let [first, middle @ .., last] = route.points.as_slice() else {
            continue;
        };
        let start = contact(
            drawing,
            component,
            graph.direction,
            source,
            route.ports.0,
            *first,
        );
        let end = contact(
            drawing,
            component,
            graph.direction,
            target,
            route.ports.1,
            *last,
        );
        work.visit(route.points.len())?;
        knots.clear();
        for at in std::iter::once(start)
            .chain(middle.iter().copied().map(point))
            .chain(std::iter::once(end))
        {
            if knots.last() == Some(&at) {
                continue;
            }
            if let &[.., a, b] = knots.as_slice()
                && (a.x == b.x && b.x == at.x && (a.y.min(at.y)..=a.y.max(at.y)).contains(&b.y)
                    || a.y == b.y && b.y == at.y && (a.x.min(at.x)..=a.x.max(at.x)).contains(&b.x))
            {
                knots.pop();
            }
            push(&mut knots, at)?;
        }
        for leg in knots.windows(2) {
            work.visit(1)?;
            push(&mut walls, (Bounds::between(leg[0], leg[1]), owner))?;
        }
        for bend in knots.windows(3) {
            work.visit(1)?;
            if let Some(bounds) = turn(bend[0], bend[1], bend[2], f64::INFINITY) {
                push(&mut walls, (bounds, owner))?;
            }
        }
        for bounds in [
            Bounds::marker_footprint(start, route.ports.0, spec.start_marker),
            Bounds::marker_footprint(end, route.ports.1, spec.end_marker),
        ]
        .into_iter()
        .flatten()
        {
            work.visit(1)?;
            push(&mut walls, (bounds, owner))?;
        }
        for (center, size) in [
            (route.label_center, spec.label),
            (route.start_label_center, spec.start_label),
            (route.end_label_center, spec.end_label),
        ] {
            if let (Some(center), Some(size)) = (center, size) {
                work.visit(1)?;
                push(
                    &mut walls,
                    (
                        Bounds::centered(center, canonical_size(graph.direction, size)),
                        owner,
                    ),
                )?;
            }
        }
    }
    Bvh::build_with_work(walls, |wall| wall.0, work)
}

#[cfg(test)]
mod tests;
