use super::super::markers::er_marker_paint_bounds;
use crate::graph_layout::{canonical::CanonicalRoutes, route_paint::RouteStyle};
use crate::model::{LayoutEdge, LayoutPoint};
use crate::resources::OperationWorkMeter;
use crate::svg::parity::{canonical_rough::constrained_rough_edge_path, svg_path_bounds_from_d};
use crate::{Error, Result};
use kurbo::{Rect, Vec2};
use roughr::core::RoughRandomness;

pub(super) struct PaintedEdge {
    pub(super) d: String,
    pub(super) points: Vec<LayoutPoint>,
    pub(super) bounds: Rect,
}

pub(super) fn paint_edge(
    routes: &CanonicalRoutes,
    edge: &LayoutEdge,
    translation: Vec2,
    look: &str,
    neo_stroke_width: f64,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
) -> Result<PaintedEdge> {
    let neo = look == "neo";
    let stroke_outset = if neo { neo_stroke_width } else { 1.0 } / 2.0;
    work_meter.charge(edge.points.len())?;
    let geometry = routes.paint(edge, translation, stroke_outset, RouteStyle::Curved)?;
    let path = geometry.path;
    let (d, path_bounds) = if look == "handDrawn" {
        work_meter.charge(path.d.len().max(1))?;
        let d = constrained_rough_edge_path(&path.d, geometry.markers, work_meter, randomness)?;
        let bounds = svg_path_bounds_from_d(&d).ok_or_else(|| Error::InvalidModel {
            message: format!("ER edge `{}` has no hand-drawn paint bounds", edge.id),
        })?;
        (d, bounds)
    } else {
        let bounds = path.bounds;
        (
            path.d,
            crate::svg::parity::path_bounds::SvgPathBounds {
                min_x: bounds.min_x,
                min_y: bounds.min_y,
                max_x: bounds.max_x,
                max_y: bounds.max_y,
            },
        )
    };
    let outset = stroke_outset * std::f64::consts::SQRT_2;
    let mut bounds = Rect::new(
        path_bounds.min_x,
        path_bounds.min_y,
        path_bounds.max_x,
        path_bounds.max_y,
    )
    .inflate(outset, outset);
    let [start_ref, start_shoulder, end_shoulder, end_ref] = &geometry.marker_points;
    for (marker, reference, shoulder, start) in [
        (
            edge.start_marker.as_deref(),
            start_ref,
            start_shoulder,
            true,
        ),
        (edge.end_marker.as_deref(), end_ref, end_shoulder, false),
    ] {
        if let Some(marker) = marker
            && let Some(footprint) = er_marker_paint_bounds(marker, neo, reference, shoulder, start)
        {
            bounds = bounds.union(footprint);
        }
    }
    Ok(PaintedEdge {
        d,
        points: geometry.data_points,
        bounds,
    })
}
