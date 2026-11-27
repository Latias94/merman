//! Move straight lanes into free space above the current box in canonical Down coordinates.

use super::super::{Component, Drawing, Expanded, canonical_size};
use super::{LEAD, advance};
use crate::geom::Bounds;
use crate::work::{LayoutResult, Work, filled, push, reserved};
use crate::{Axis, EdgeGeometry, EdgeSpec, Graph, MarkerMetrics, Options, Point, RoutePoint, Side};
#[cfg(test)]
use crate::{Rect, Size};

const MAX_WORK: usize = 64 * 1024;

impl Bounds {
    fn terminal(at: RoutePoint, side: Side, marker: MarkerMetrics) -> Self {
        Self::marker(
            Point { x: at.x, y: at.y },
            side,
            MarkerMetrics {
                length: marker.length.max(LEAD),
                ..marker
            },
        )
    }

    fn contains_x(self, x: f64) -> bool {
        self.x[0] <= x && x <= self.x[1]
    }
}

fn route_bounds(points: &[RoutePoint], sides: (Side, Side), spec: &EdgeSpec) -> Option<Bounds> {
    let [first, middle @ .., last] = points else {
        return None;
    };
    // The routed knots and outward leads enclose the shaft, curve controls, and rounded turns.
    let mut envelope = Bounds::terminal(*first, sides.0, spec.start_marker)
        .union(Bounds::terminal(*last, sides.1, spec.end_marker));
    for knot in middle {
        envelope.include(Point {
            x: knot.x,
            y: knot.y,
        });
    }
    envelope.finite().then_some(envelope)
}

fn admit(remaining: &mut usize, units: usize, work: &mut Work<'_>) -> LayoutResult<bool> {
    let Some(rest) = remaining.checked_sub(units) else {
        return Ok(false);
    };
    work.visit(units)?;
    *remaining = rest;
    Ok(true)
}

struct Scene {
    routes: Vec<Bounds>,
    labels: Vec<Bounds>,
}

impl Scene {
    fn read(
        drawing: &Drawing,
        graph: &Graph,
        remaining: &mut usize,
        work: &mut Work<'_>,
    ) -> LayoutResult<Option<Self>> {
        let mut routes = reserved(drawing.edges.len())?;
        let mut labels = Vec::new();
        for (id, route) in &drawing.edges {
            if !admit(remaining, route.points.len().saturating_add(3), work)? {
                return Ok(None);
            }
            let spec = &graph.edges[*id as usize];
            let Some(bounds) = route_bounds(&route.points, route.ports, spec) else {
                return Ok(None);
            };
            routes.push(bounds);
            for (at, size) in [
                (route.label_center, spec.label),
                (route.start_label_center, spec.start_label),
                (route.end_label_center, spec.end_label),
            ] {
                match (at, size) {
                    (Some(at), Some(size)) => {
                        let bounds = Bounds::centered(at, canonical_size(graph.direction, size));
                        if !bounds.finite() {
                            return Ok(None);
                        }
                        push(&mut labels, bounds)?;
                    }
                    (None, None) => {}
                    _ => return Ok(None),
                }
            }
        }
        Ok(Some(Self { routes, labels }))
    }

    fn floor(
        &self,
        drawing: &Drawing,
        node: usize,
        incoming: usize,
        outgoing: Option<usize>,
        footprint: Bounds,
        gap: f64,
    ) -> Option<f64> {
        let nodes = drawing
            .nodes
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != node)
            .map(|(_, &rect)| Bounds::of(rect));
        let routes = self
            .routes
            .iter()
            .enumerate()
            .filter_map(|(i, &b)| (i != incoming && Some(i) != outgoing).then_some(b));
        // Each obstacle contributes its bottom step over its cross interval. Approach the highest
        // step above the footprint. A step already overlapping the footprint prevents the move.
        let mut floor = f64::NEG_INFINITY;
        for wall in nodes.chain(routes).chain(self.labels.iter().copied()) {
            if wall.x[1] + gap < footprint.x[0]
                || footprint.x[1] < wall.x[0] - gap
                || footprint.y[1] < wall.y[0] - gap
            {
                continue;
            }
            let bottom = wall.y[1] + gap;
            if bottom >= footprint.y[0] {
                return None;
            }
            floor = floor.max(bottom);
        }
        Some(floor)
    }
}

#[derive(Clone, Copy, Default)]
struct Incident {
    incoming: Option<usize>,
    outgoing: Option<usize>,
    held: bool,
}

pub(in crate::flat) fn compact_ranks(
    drawing: &mut Drawing,
    component: &Component,
    expanded: &Expanded,
    graph: &Graph,
    options: &Options,
    work: &mut Work<'_>,
) -> LayoutResult<()> {
    if component.boundaries
        || component.pinned
        || component.group_proxies
        || component.links.is_empty()
    {
        return Ok(());
    }
    let mut remaining = MAX_WORK;
    let setup = drawing
        .nodes
        .len()
        .saturating_add(drawing.edges.len())
        .saturating_add(expanded.rank.len())
        .saturating_add(expanded.layers.len());
    if !admit(&mut remaining, setup, work)? {
        return Ok(());
    }
    let mut incidents = filled(drawing.nodes.len(), Incident::default())?;
    for (i, link) in component.links.iter().enumerate() {
        let (id, route) = &drawing.edges[i];
        let usable = !expanded.direct[i] && straight(route, &graph.edges[*id as usize]);
        for (v, receiving) in [(link.from, false), (link.to, true)] {
            let incident = &mut incidents[v as usize];
            let slot = if receiving {
                &mut incident.incoming
            } else {
                &mut incident.outgoing
            };
            incident.held |= slot.replace(i).is_some() || !usable;
        }
    }
    for loop_edge in &component.loops {
        incidents[loop_edge.node as usize].held = true;
    }
    if !incidents.iter().any(|i| i.incoming.is_some() && !i.held) {
        return Ok(());
    }
    let Some(mut scene) = Scene::read(drawing, graph, &mut remaining, work)? else {
        return Ok(());
    };
    let scan = drawing.nodes.len() + scene.routes.len() + scene.labels.len();
    for &node in expanded.layers.iter().flatten() {
        let node = node as usize;
        let Some(&Incident {
            incoming: Some(incoming),
            outgoing,
            held: false,
        }) = incidents.get(node)
        else {
            continue;
        };
        let parent = component.links[incoming].from as usize;
        let (a, b) = (expanded.rank[parent] as usize, expanded.rank[node] as usize);
        if b != a + 1 {
            continue;
        }
        let ranks = expanded.logical_rank[b] - expanded.logical_rank[a];
        let parent = Bounds::of(drawing.nodes[parent]);
        let Ok(earliest) = advance(parent.y[1], ranks as f64 * options.spacing.rank) else {
            continue;
        };
        if earliest >= drawing.nodes[node].y {
            continue;
        }
        if !admit(&mut remaining, scan, work)? {
            return Ok(());
        }
        slide(
            drawing,
            graph,
            &mut scene,
            node,
            (incoming, outgoing),
            earliest,
            options.spacing.edge,
        );
    }
    Ok(())
}

fn straight(route: &EdgeGeometry, spec: &EdgeSpec) -> bool {
    let [a, b] = route.points.as_slice() else {
        return false;
    };
    !route.reversed_for_ranking
        && route.ports == (Side::Bottom, Side::Top)
        && a.axis == Axis::Vertical
        && b.axis == Axis::Vertical
        && a.x == b.x
        && a.y < b.y
        && spec.label.is_none()
        && spec.start_label.is_none()
        && spec.end_label.is_none()
}

fn slide(
    drawing: &mut Drawing,
    graph: &Graph,
    scene: &mut Scene,
    node: usize,
    (incoming, outgoing): (usize, Option<usize>),
    earliest: f64,
    clearance: f64,
) {
    let rect = drawing.nodes[node];
    let (id, approach) = &drawing.edges[incoming];
    let spec = &graph.edges[*id as usize];
    let [parent, contact] = [approach.points[0], approach.points[1]];
    let node_bounds = Bounds::of(rect);
    if contact.y != node_bounds.y[0] || !node_bounds.contains_x(contact.x) {
        return;
    }
    let up = spec.end_marker.length.max(LEAD);
    // Both opposed markers must fit on the shortened incoming shaft. The outgoing shaft only
    // grows, and its extension lies inside the swept node box along its original port normal.
    let Ok(runway) = advance(parent.y, spec.start_marker.length.max(LEAD) + up) else {
        return;
    };
    let mut footprint = node_bounds.union(Bounds::terminal(contact, Side::Top, spec.end_marker));
    let departure = if let Some(i) = outgoing {
        let (id, route) = &drawing.edges[i];
        let spec = &graph.edges[*id as usize];
        let start = route.points[0];
        if start.y != node_bounds.y[1] || !node_bounds.contains_x(start.x) {
            return;
        }
        footprint = footprint.union(Bounds::terminal(start, Side::Bottom, spec.start_marker));
        Some((i, spec, start, route.points[1]))
    } else {
        None
    };
    if !footprint.finite() {
        return;
    }
    let Some(floor) = scene.floor(drawing, node, incoming, outgoing, footprint, clearance) else {
        return;
    };
    let y = earliest.max(runway).max((floor + up).next_up());
    let bottom = y + rect.height;
    if y >= rect.y
        || y - up <= floor
        || !bottom.is_finite()
        || parent.y >= y
        || (rect.height > 0.0 && bottom <= y)
    {
        return;
    }
    let contact = RoutePoint { y, ..contact };
    let Some(approach) = route_bounds(&[parent, contact], approach.ports, spec) else {
        return;
    };
    if let Some((i, spec, start, target)) = departure {
        if bottom >= target.y {
            return;
        }
        let start = RoutePoint { y: bottom, ..start };
        let route = &mut drawing.edges[i].1;
        let Some(bounds) = route_bounds(&[start, target], route.ports, spec) else {
            return;
        };
        scene.routes[i] = bounds;
        route.points[0] = start;
        route.corridor_clearance = 0.0;
    }
    scene.routes[incoming] = approach;
    drawing.edges[incoming].1.points[1] = contact;
    drawing.edges[incoming].1.corridor_clearance = 0.0;
    drawing.nodes[node].y = y;
}

#[cfg(test)]
mod tests;
