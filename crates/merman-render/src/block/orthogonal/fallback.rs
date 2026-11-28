use super::{
    BlockDiagramLayout, BlockDiagramRenderModel, BlockShapeGeometry, EdgeStyle,
    OperationLayoutWorkControl, Point, Rect, Result, RoutePoint, Side, SolvedRequest, Wall,
    block_render_edge_points, contact_work, edge_marker_geometry, intersects, knot_point,
    marker_bounds, rect_bounds, to_point,
};
use merman_layout_shoal::geom::{Bounds, Bvh, BvhEntry};
use std::{collections::HashMap, ops::ControlFlow};

/// Compute the grid marker geometry once, independently of the selected ports.
pub(super) fn markers(
    model: &BlockDiagramRenderModel,
    layout: &BlockDiagramLayout,
    edge_indices: &HashMap<&str, usize>,
    geometries: &HashMap<&str, &BlockShapeGeometry>,
    bodies: &HashMap<&str, u32>,
    style: Option<EdgeStyle>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<[Option<Wall>; 2]>> {
    work.charge_adapter(model.edges.len())?;
    let mut fallback = vec![[None; 2]; model.edges.len()];
    for (occurrence, edge) in model.edges.iter().enumerate() {
        let [start, end] = [true, false].map(|start| edge_marker_geometry(edge, start));
        let (Some(&index), Some(&from), Some(&to)) = (
            edge_indices.get(edge.id.as_str()),
            bodies.get(edge.start.as_str()),
            bodies.get(edge.end.as_str()),
        ) else {
            continue;
        };
        if start.is_none() && end.is_none() {
            continue;
        }
        let edge_layout = &layout.edges[index];
        work.charge_adapter(edge_layout.points.len())?;
        for body in [from, to] {
            work.charge_adapter(contact_work(&layout.shape_geometries[body as usize]))?;
        }
        let points = block_render_edge_points(edge, edge_layout, geometries, style).rendered_points;
        for (slot, (marker, first, next, body)) in [
            (start, points.first(), points.get(1), from),
            (end, points.last(), points.iter().rev().nth(1), to),
        ]
        .into_iter()
        .enumerate()
        {
            let (Some(marker), Some(first), Some(next)) = (marker, first, next) else {
                continue;
            };
            let first = to_point(first);
            let delta = to_point(next) - first;
            let distance = delta.hypot();
            if distance.is_finite() && distance > 0.0 {
                let outward = delta / distance;
                let rect = marker_bounds(marker, first, outward);
                if rect.is_finite() {
                    // The grid path can approach diagonally, so it has no orthogonal face.
                    let contact = first - outward * marker.routing.trim;
                    fallback[occurrence][slot] = Some(Wall::marker(
                        rect,
                        occurrence,
                        body,
                        RoutePoint::new(contact.x, contact.y, Side::Top.axis()),
                    ));
                }
            }
        }
    }
    Ok(fallback)
}

enum Footprint {
    Shaft(Point, Point),
    Marker(Rect),
}

impl Footprint {
    fn bounds(&self) -> Rect {
        match *self {
            Self::Shaft(a, b) => Rect::from_points(a, b),
            Self::Marker(rect) => rect,
        }
    }

    fn enters(&self, rect: Rect) -> bool {
        if !intersects(self.bounds(), rect) {
            return false;
        }
        let Self::Shaft(a, b) = *self else {
            return true;
        };
        if a.x == b.x || a.y == b.y {
            return true;
        }
        // Orthogonal spans may use a direct diagonal in polyline mode.
        let [x0, x1] = [rect.x0, rect.x1].map(|x| (x - a.x) / (b.x - a.x));
        let [y0, y1] = [rect.y0, rect.y1].map(|y| (y - a.y) / (b.y - a.y));
        x0.min(x1).max(y0.min(y1)).max(0.0) < x0.max(x1).min(y0.max(y1)).min(1.0)
    }
}

/// A route that could not be solved restores its grid markers. Remove only solved
/// paths that would intersect the restored ink, then examine their grid markers too.
/// Each edge can fall back once, and BVH construction and visits share the layout budget.
pub(super) fn restore(
    solved: &mut [Option<SolvedRequest>],
    fallback: &[[Option<Wall>; 2]],
    margin: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(solved.len())?;
    let mut pending: Vec<_> = solved
        .iter()
        .zip(fallback)
        .enumerate()
        .filter(|(_, (route, markers))| route.is_none() && markers.iter().any(Option::is_some))
        .map(|(edge, _)| edge)
        .collect();
    if pending.is_empty() {
        return Ok(());
    }
    let mut footprints = Vec::new();
    for route in solved.iter().flatten() {
        work.charge_adapter(work.checked_add(route.points.len(), 2)?)?;
        let edge = route.request.placement.occurrence;
        footprints.extend(route.points.windows(2).map(|pair| {
            (
                edge,
                Footprint::Shaft(knot_point(pair[0]), knot_point(pair[1])),
            )
        }));
        footprints.extend(
            route
                .request
                .marker_walls()
                .into_iter()
                .flatten()
                .map(|wall| (edge, Footprint::Marker(wall.rect))),
        );
    }
    let footprints =
        Bvh::build_controlled(footprints, |(_, item)| rect_bounds(item.bounds()), work)
            .map_err(|error| work.map_shoal_error(error))?;
    while let Some(edge) = pending.pop() {
        for &marker in fallback[edge].iter().flatten() {
            work.charge_adapter(1)?;
            let halo = marker.rect.inflate(margin, margin);
            let _ = footprints.visit(|entry| -> Result<ControlFlow<(), bool>> {
                work.charge_adapter(1)?;
                match entry {
                    BvhEntry::Node(Bounds {
                        x: [x0, x1],
                        y: [y0, y1],
                    }) => Ok(ControlFlow::Continue(intersects(
                        Rect::new(x0, y0, x1, y1),
                        halo,
                    ))),
                    BvhEntry::Item((edge, footprint)) => {
                        if solved[*edge].is_some() && footprint.enters(halo) {
                            solved[*edge] = None;
                            pending.push(*edge);
                        }
                        Ok(ControlFlow::Continue(true))
                    }
                }
            })?;
        }
    }
    Ok(())
}
