use super::super::measure::enters;
use super::{label_rect, node_endpoint, normal, point};
use crate::Result;
use crate::graph_layout::ir::{Axis, MeasuredGraph, Point, Rect, RoutePoint, SolvedGraph};
use crate::graph_layout::route_paint::LEAD;
use crate::layout_work::OperationLayoutWorkControl;

const CLEARANCE: f64 = 3.0;

pub(super) fn bbox(rect: Rect) -> kurbo::Rect {
    kurbo::Rect::new(rect.x, rect.y, rect.x + rect.width, rect.y + rect.height)
}

pub(super) fn contains(outer: kurbo::Rect, inner: kurbo::Rect) -> bool {
    outer.x0 <= inner.x0 && outer.y0 <= inner.y0 && outer.x1 >= inner.x1 && outer.y1 >= inner.y1
}

pub(super) fn overlaps(a: kurbo::Rect, b: kurbo::Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

type RouteChanges = Vec<(usize, Vec<RoutePoint>)>;

pub(super) fn reconnect(
    scope: Option<u32>,
    graph: &MeasuredGraph,
    solved: &SolvedGraph,
    rect: &impl Fn(usize) -> Rect,
    changed: &impl Fn(usize) -> bool,
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<RouteChanges>> {
    let mut walls: Vec<_> = (0..solved.nodes.len())
        .map(|index| bbox(rect(index)).inflate(CLEARANCE, CLEARANCE))
        .collect();
    let moved: Vec<_> = (0..solved.nodes.len())
        .filter(|&index| changed(index))
        .map(|index| walls[index])
        .collect();
    for group in &solved.groups {
        if group.parent == scope {
            walls.push(bbox(group.rect).inflate(CLEARANCE, CLEARANCE));
        }
        if let Some(label) = &group.title {
            walls.push(label_rect(label).inflate(CLEARANCE, CLEARANCE));
        }
    }
    walls.extend(
        solved
            .edges
            .iter()
            .filter_map(|edge| edge.label.as_ref())
            .map(label_rect),
    );
    let bounds = scope.map_or_else(
        || kurbo::Rect::new(0.0, 0.0, f64::INFINITY, f64::INFINITY),
        |scope| bbox(solved.groups[scope as usize].rect),
    );
    let mut replacements = Vec::new();
    for (index, (edge, spec)) in solved.edges.iter().zip(&graph.edges).enumerate() {
        let ends = (node_endpoint(edge.source), node_endpoint(edge.target));
        if !(ends.0.is_some_and(changed) || ends.1.is_some_and(changed)) {
            work.charge_adapter(work.checked_mul(edge.points.len(), moved.len())?)?;
            if edge
                .points
                .windows(2)
                .any(|leg| hits(&moved, point(leg[0]), point(leg[1]), None))
            {
                return Ok(None);
            }
            continue;
        }
        let (Some(source), Some(target)) = ends else {
            return Ok(None);
        };
        if graph.nodes[source].parent != scope
            || graph.nodes[target].parent != scope
            || edge.label.is_some()
            || solved
                .border_stitches
                .binary_search_by_key(&index, |stitch| stitch.edge)
                .is_ok()
        {
            return Ok(None);
        }
        let [first, .., last] = edge.points.as_slice() else {
            return Ok(None);
        };
        let translate = |port: Point, index: usize| {
            let old = solved.nodes[index];
            let new = rect(index);
            Point {
                x: port.x + new.x - old.x,
                y: port.y + new.y - old.y,
            }
        };
        let start = translate(point(*first), source);
        let end = translate(point(*last), target);
        let lead = |p: Point, side, trim| {
            let (x, y) = normal(side);
            let distance = trim + LEAD + CLEARANCE;
            Point {
                x: p.x + distance * x,
                y: p.y + distance * y,
            }
        };
        let from = lead(start, edge.ports.0, spec.start_marker.trim);
        let to = lead(end, edge.ports.1, spec.end_marker.trim);
        work.charge_adapter(work.checked_mul(walls.len(), 2)?)?;
        if hits(&walls, start, from, Some(source)) || hits(&walls, to, end, Some(target)) {
            return Ok(None);
        }
        let Some(path) = find_path(from, to, bounds, &walls, work)? else {
            return Ok(None);
        };
        let points: Vec<_> = std::iter::once(start)
            .chain(path)
            .chain(std::iter::once(end))
            .collect();
        let mut knots = Vec::with_capacity(points.len());
        for leg in points.windows(2) {
            let (a, b) = (leg[0], leg[1]);
            if a != b {
                let axis = if a.x == b.x {
                    Axis::Vertical
                } else {
                    Axis::Horizontal
                };
                knots.push(RoutePoint::new(a.x, a.y, axis));
            }
        }
        knots.push(RoutePoint::new(end.x, end.y, edge.ports.1.axis()));
        replacements.push((index, knots));
    }
    Ok(Some(replacements))
}

fn hits(walls: &[kurbo::Rect], from: Point, to: Point, except: Option<usize>) -> bool {
    walls
        .iter()
        .enumerate()
        .any(|(i, &wall)| Some(i) != except && enters(wall, from, to))
}

fn find_path(
    from: Point,
    to: Point,
    bounds: kurbo::Rect,
    walls: &[kurbo::Rect],
    work: &mut OperationLayoutWorkControl,
) -> Result<Option<[Point; 4]>> {
    let mut best = None;
    let mut consider = |path: [Point; 4], work: &mut OperationLayoutWorkControl| -> Result<()> {
        work.charge_adapter(work.checked_mul(walls.len(), 3)?)?;
        let legs = path.windows(2).filter(|leg| leg[0] != leg[1]);
        let length = legs
            .clone()
            .map(|leg| (leg[1].x - leg[0].x).abs() + (leg[1].y - leg[0].y).abs())
            .sum::<f64>();
        let score = (length, legs.clone().count());
        if best.as_ref().is_none_or(|(best, _)| score < *best)
            && path.iter().all(|p| {
                p.x >= bounds.x0 && p.y >= bounds.y0 && p.x <= bounds.x1 && p.y <= bounds.y1
            })
            && !legs.clone().any(|leg| hits(walls, leg[0], leg[1], None))
        {
            best = Some((score, path));
        }
        Ok(())
    };
    let via_x = |x| [from, Point { x, y: from.y }, Point { x, y: to.y }, to];
    let via_y = |y| [from, Point { x: from.x, y }, Point { x: to.x, y }, to];
    for x in [from.x, to.x, from.x.midpoint(to.x)] {
        consider(via_x(x), work)?;
    }
    for y in [from.y.midpoint(to.y), to.y, from.y] {
        consider(via_y(y), work)?;
    }
    work.charge_adapter(walls.len())?;
    let span = kurbo::Rect::new(from.x, from.y, to.x, to.y).abs();
    let occupied = walls
        .iter()
        .filter(|&&wall| overlaps(wall, span))
        .copied()
        .reduce(|a, b| a.union(b));
    if let Some(wall) = occupied {
        consider(via_x(wall.x0), work)?;
        consider(via_x(wall.x1), work)?;
        consider(via_y(wall.y0), work)?;
        consider(via_y(wall.y1), work)?;
    }
    Ok(best.map(|(_, path)| path))
}
