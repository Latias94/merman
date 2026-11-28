use super::{
    BlockDiagramLayout, BlockDiagramRenderModel, BlockShapeGeometry, EdgePlacement, EdgeStyle,
    LayoutLabel, OperationLayoutWorkControl, Point, Rect, Result, RoutePoint, Side, SolvedRequest,
    Wall, block_render_edge_points, contact_work, edge_marker_geometry, intersects, knot_point,
    marker_bounds, rect_bounds, to_point,
};
use kurbo::{BezPath, ParamCurve, PathSeg, Shape};
use merman_layout_shoal::geom::{Bounds, Bvh, BvhEntry};
use merman_render_core::route_paint::ObstacleOwner;
use merman_render_core::svg_edge_parity::curve::curve_basis_path_d;
use std::{collections::HashMap, ops::ControlFlow};

#[derive(Clone, Copy, Default)]
pub(super) struct GridEdge {
    pub(super) blocked: bool,
    pub(super) markers: [Option<Wall>; 2],
    pub(super) caption: Option<Wall>,
}

impl GridEdge {
    pub(super) fn walls(&self) -> impl Iterator<Item = Wall> {
        self.markers.into_iter().chain([self.caption]).flatten()
    }
}

/// Inspect the unmodified grid path, including the legacy painter's marker insets.
pub(super) fn grid_edges(
    model: &BlockDiagramRenderModel,
    layout: &BlockDiagramLayout,
    placements: &[EdgePlacement],
    fixed: &[Wall],
    captions: &[Option<LayoutLabel>],
    style: Option<EdgeStyle>,
    work: &mut OperationLayoutWorkControl,
) -> Result<Vec<GridEdge>> {
    work.charge_adapter(work.checked_add(model.edges.len(), layout.shape_geometries.len())?)?;
    let mut fallback = vec![GridEdge::default(); model.edges.len()];
    let geometries: HashMap<&str, &BlockShapeGeometry> = layout
        .shape_geometries
        .iter()
        .map(|shape| (shape.id.as_str(), shape))
        .collect();
    work.charge_adapter(fixed.len())?;
    let fixed = Bvh::build_controlled(fixed.to_vec(), |wall| rect_bounds(wall.rect), work)
        .map_err(|error| work.map_shoal_error(error))?;
    for &EdgePlacement {
        occurrence,
        index,
        bodies,
    } in placements
    {
        let edge = &model.edges[occurrence];
        let [start, end] = [true, false].map(|start| edge_marker_geometry(edge, start));
        let grid = &mut fallback[occurrence];
        let edge_layout = &layout.edges[index];
        work.charge_adapter(edge_layout.points.len())?;
        for body in bodies {
            work.charge_adapter(contact_work(&layout.shape_geometries[body as usize]))?;
        }
        let points =
            block_render_edge_points(edge, edge_layout, &geometries, style).rendered_points;
        if matches!(style, None | Some(EdgeStyle::Curved)) {
            work.charge_adapter(points.len())?;
            let path = BezPath::from_svg(&curve_basis_path_d(&points)).map_err(|error| {
                crate::Error::InvalidModel {
                    message: format!("Block grid edge {}: {error}", edge.id),
                }
            })?;
            for segment in path.segments() {
                if Footprint::Curve(segment).blocked_by(&fixed, bodies, work)? {
                    grid.blocked = true;
                    break;
                }
            }
        } else {
            for leg in points.windows(2) {
                if Footprint::Shaft(to_point(&leg[0]), to_point(&leg[1]))
                    .blocked_by(&fixed, bodies, work)?
                {
                    grid.blocked = true;
                    break;
                }
            }
        }
        if grid.blocked {
            continue;
        }
        for (slot, (marker, body)) in [start, end].into_iter().zip(bodies).enumerate() {
            let Some(marker) = marker else {
                continue;
            };
            work.charge_adapter(points.len())?;
            let terminal = if slot == 0 {
                marker_tangent(points.iter().map(to_point))
            } else {
                marker_tangent(points.iter().rev().map(to_point))
            };
            if let Some((first, outward)) = terminal {
                let rect = marker_bounds(marker, first, outward);
                if rect.is_finite() {
                    if Footprint::Marker(rect).blocked_by(&fixed, bodies, work)? {
                        *grid = GridEdge {
                            blocked: true,
                            ..Default::default()
                        };
                        break;
                    }
                    // The grid path can approach diagonally, so it has no orthogonal face.
                    let contact = first - outward * marker.routing.trim;
                    grid.markers[slot] = Some(Wall::marker(
                        rect,
                        occurrence,
                        body,
                        RoutePoint::new(contact.x, contact.y, Side::Top.axis()),
                    ));
                }
            }
        }
        if !grid.blocked && captions[occurrence].is_some() {
            grid.caption = edge_layout
                .label
                .as_ref()
                .map(|label| Wall::label(label, occurrence));
        }
    }
    Ok(fallback)
}

fn marker_tangent(mut points: impl Iterator<Item = Point>) -> Option<(Point, kurbo::Vec2)> {
    let first = points.next()?;
    // SVG takes the tangent of the first nonzero segment at a repeated endpoint.
    let next = points.find(|&point| point != first)?;
    let delta = next - first;
    let distance = delta.hypot();
    (distance.is_finite() && distance > 0.0).then(|| (first, delta / distance))
}

#[cfg(test)]
#[test]
fn fallback_markers_use_the_first_nonzero_tangent_at_each_end() {
    let start = Point::new(20.0, 4.0);
    let end = Point::new(25.0, 4.0);
    let points = [start, start, end, end];
    let direction = kurbo::Vec2::new(1.0, 0.0);
    assert_eq!(marker_tangent(points.into_iter()), Some((start, direction)));
    assert_eq!(
        marker_tangent(points.into_iter().rev()),
        Some((end, -direction))
    );
}

enum Footprint {
    Shaft(Point, Point),
    Curve(PathSeg),
    Marker(Rect),
}

impl Footprint {
    fn bounds(&self) -> Rect {
        match *self {
            Self::Shaft(a, b) => Rect::from_points(a, b),
            Self::Curve(segment) => segment.bounding_box(),
            Self::Marker(rect) => rect,
        }
    }

    fn enters(&self, rect: Rect) -> bool {
        if !intersects(self.bounds(), rect) {
            return false;
        }
        let (a, b) = match *self {
            Self::Shaft(a, b) => (a, b),
            Self::Curve(PathSeg::Line(line)) => (line.p0, line.p1),
            Self::Curve(segment) => {
                let inside = |point: Point| {
                    rect.x0 < point.x && point.x < rect.x1 && rect.y0 < point.y && point.y < rect.y1
                };
                return inside(segment.start())
                    || inside(segment.end())
                    || kurbo::segments(rect.path_elements(0.0)).any(|side| {
                        let PathSeg::Line(line) = side else {
                            unreachable!()
                        };
                        !segment.intersect_line(line).is_empty()
                    });
            }
            Self::Marker(_) => return true,
        };
        if a.x == b.x || a.y == b.y {
            return true;
        }
        // Orthogonal spans may use a direct diagonal in polyline mode.
        let [x0, x1] = [rect.x0, rect.x1].map(|x| (x - a.x) / (b.x - a.x));
        let [y0, y1] = [rect.y0, rect.y1].map(|y| (y - a.y) / (b.y - a.y));
        x0.min(x1).max(y0.min(y1)).max(0.0) < x0.max(x1).min(y0.max(y1)).min(1.0)
    }

    fn blocked_by(
        &self,
        walls: &Bvh<Wall>,
        endpoints: [u32; 2],
        work: &mut OperationLayoutWorkControl,
    ) -> Result<bool> {
        let bounds = self.bounds();
        work.charge_adapter(1)?;
        let result = walls.visit(|entry| -> Result<ControlFlow<(), bool>> {
            work.charge_adapter(1)?;
            match entry {
                BvhEntry::Node(Bounds {
                    x: [x0, x1],
                    y: [y0, y1],
                }) => Ok(ControlFlow::Continue(intersects(
                    bounds,
                    Rect::new(x0, y0, x1, y1),
                ))),
                BvhEntry::Item(wall) => {
                    let endpoint = match wall.owner {
                        ObstacleOwner::Node(node) | ObstacleOwner::GroupTitle(node) => {
                            endpoints.contains(&node)
                        }
                        _ => false,
                    };
                    if !endpoint && self.enters(wall.rect) {
                        Ok(ControlFlow::Break(()))
                    } else {
                        Ok(ControlFlow::Continue(true))
                    }
                }
            }
        })?;
        Ok(result.is_break())
    }
}

/// A safe grid fallback restores its markers and any displaced caption. Remove
/// solved paths that intersect the restored ink, then examine their grid ink too.
/// Each edge can fall back once, and BVH construction and visits share the layout budget.
pub(super) fn restore(
    solved: &mut [Option<SolvedRequest>],
    fallback: &[GridEdge],
    margin: f64,
    work: &mut OperationLayoutWorkControl,
) -> Result<()> {
    work.charge_adapter(solved.len())?;
    let mut pending: Vec<_> = solved
        .iter()
        .zip(fallback)
        .enumerate()
        .filter(|(_, (route, grid))| route.is_none() && grid.walls().next().is_some())
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
        for wall in fallback[edge].walls() {
            work.charge_adapter(1)?;
            let halo = wall.rect.inflate(margin, margin);
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
