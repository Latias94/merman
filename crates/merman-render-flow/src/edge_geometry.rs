//! Flow edge paint geometry after the layout endpoints have been resolved.

use merman_render_core::{
    LayoutPoint,
    svg_edge_parity::{
        curve::CurveGeometry,
        edge_path::{
            line_with_offset_points, maybe_fix_corners, rounded_line_with_marker_offsets_points,
        },
    },
};

use crate::edge_svg::arrow_types_for_edge;

mod basis;
mod curve_path;
mod endpoint;
mod rect_clip;

pub use endpoint::{
    FlowchartEndpointContext, force_intersect_for_layout_shape, intersect_for_layout_shape,
};
pub use rect_clip::EdgeClipRect;

/// The provider's original points and the exact root-local translation for the singleton fallback.
pub struct LegacyEdgePointProjection<'a> {
    pub points: &'a [LayoutPoint],
    pub translation: (f64, f64),
    pub origin: (f64, f64),
}

/// The line data after Flow's corner preparation and marker offsets.
pub struct LegacyEdgeCurve<'a> {
    line_data: Vec<LayoutPoint>,
    interpolate: &'a str,
}

impl<'a> LegacyEdgeCurve<'a> {
    /// Prepares a clipped route once the caller has retained the original and clipped label paths.
    pub fn prepare(
        points_for_render: &mut Vec<LayoutPoint>,
        interpolate: &'a str,
        is_cluster_edge: bool,
        singleton_fallback: Option<LegacyEdgePointProjection<'_>>,
        edge_type: Option<&str>,
    ) -> Self {
        let is_rounded = interpolate == "rounded";
        let is_basis = !matches!(
            interpolate,
            "linear"
                | "natural"
                | "bumpX"
                | "bumpY"
                | "catmullRom"
                | "step"
                | "stepAfter"
                | "stepBefore"
                | "cardinal"
                | "monotoneX"
                | "monotoneY"
                | "rounded"
        );
        if is_basis && is_cluster_edge {
            basis::maybe_remove_redundant_cluster_run_point(points_for_render);
        }

        if let Some(fallback) = singleton_fallback
            && points_for_render.len() == 1
        {
            // Avoid a degenerate move for clipped cluster-adjacent edges.
            points_for_render.clear();
            points_for_render.extend(fallback.points.iter().map(|point| LayoutPoint {
                x: point.x + fallback.translation.0 - fallback.origin.0,
                y: point.y + fallback.translation.1 - fallback.origin.1,
            }));
        }

        let mut line_data: Vec<LayoutPoint> = points_for_render
            .iter()
            .filter(|p| !p.y.is_nan())
            .cloned()
            .collect();

        // Mermaid's rendering-elements/edges.js applies fixCorners before the D3 line
        // generator. The rounded generator has its own corner preparation.
        if !is_rounded {
            maybe_fix_corners(&mut line_data);
        }

        // See Mermaid's utils/lineWithOffset.ts for the marker setback rules.
        let (start_marker, end_marker) = arrow_types_for_edge(edge_type);
        let line_data = if is_rounded {
            rounded_line_with_marker_offsets_points(&line_data, start_marker, end_marker)
        } else {
            line_with_offset_points(&line_data, start_marker, end_marker)
        };
        Self {
            line_data,
            interpolate,
        }
    }

    /// Emits the curve and measures its serialized bounds, with optional Neo centerline length.
    pub fn into_geometry(self, rounded_radius: f64, measure_length: bool) -> CurveGeometry {
        curve_path::curve_path_geometry(
            &self.line_data,
            self.interpolate,
            rounded_radius,
            measure_length,
        )
    }

    /// Gives the compact Dagre caller the same post-offset points without serializing a path.
    #[cfg(all(feature = "diagram-flowchart", feature = "layout-dagre"))]
    pub fn into_linear_points(self) -> Option<Vec<LayoutPoint>> {
        (self.interpolate == "linear").then_some(self.line_data)
    }
}
