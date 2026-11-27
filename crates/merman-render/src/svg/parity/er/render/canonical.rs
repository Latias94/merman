use crate::er::ErCssBinding;
use crate::graph_layout::{
    canonical::{CanonicalPaintScratch, CanonicalRoutes},
    route_paint::RouteStyle,
};
use crate::model::{LayoutEdge, LayoutPoint};
use crate::resources::OperationWorkMeter;
use crate::svg::parity::canonical_rough::constrained_rough_edge_path;
use crate::{Error, Result};
use kurbo::{Rect, Vec2};
use merman_render_core::svg_bounds::{SvgPathBounds, svg_path_bounds_from_d};
use merman_render_graph::er::er_marker_paint_bounds;
use roughr::core::RoughRandomness;

pub(super) struct PaintedEdge<'points> {
    pub(super) d: String,
    pub(super) points: &'points [LayoutPoint],
    pub(super) bounds: Rect,
}

pub(super) fn paint_edge<'points>(
    routes: &CanonicalRoutes,
    edge: &LayoutEdge,
    translation: Vec2,
    binding: &ErCssBinding,
    work_meter: &OperationWorkMeter,
    randomness: &RoughRandomness,
    scratch: &'points mut CanonicalPaintScratch,
) -> Result<PaintedEdge<'points>> {
    let look = binding.render_settings.diagram_look.as_str();
    let neo = look == "neo";
    let native = binding.native.as_ref();
    let stroke_outset = native.map_or(
        if neo {
            binding.neo_marker_stroke_width
        } else {
            1.0
        },
        |style| style.connector_width,
    ) / 2.0;
    work_meter.charge(edge.points.len())?;
    use merman_theme_contract::v2::EdgeStyle;
    let style = match native.map(|style| style.edge_style) {
        Some(EdgeStyle::Orthogonal) => RouteStyle::Step,
        Some(EdgeStyle::Straight | EdgeStyle::Sketch) => RouteStyle::Linear,
        _ => RouteStyle::Curved,
    };
    let geometry = routes.paint_with_borrowed_points(
        edge,
        translation,
        stroke_outset,
        style,
        work_meter,
        scratch,
    )?;
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
            SvgPathBounds {
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
