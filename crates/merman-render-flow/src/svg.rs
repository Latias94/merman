//! Complete SVG emission for admitted Flow-family layouts.

#[cfg(any(feature = "canonical-edge-paint", test))]
use merman_model::families::flowchart::FlowEdge;
use merman_model::runtime::OperationContext;
#[cfg(any(feature = "canonical-edge-paint", test))]
use merman_render_core::svg_edge_parity::edge_path;
use merman_render_core::{
    Bounds, Error, LayoutCluster, LayoutNode, Result,
    icon_registry::IconRegistry,
    math::MathRenderer,
    resources::OperationWorkMeter,
    root_svg::{self, RootViewportContext},
    svg_css::SvgId,
    svg_edge_parity::trace::FlowchartEdgeTraceCollector,
    svg_output::{
        SvgOutput,
        timing::{self, RenderTiming},
    },
    text::{TextMeasurer, TextStyle, WrapMode},
    xml::escape_attr_display,
};
use merman_theme_contract::v2::{Canvas, Color, FamilyTheme};
use rustc_hash::{FxHashMap, FxHashSet};

#[cfg(any(feature = "canonical-edge-paint", test))]
use crate::markers::{flowchart_edge_marker_end_base, flowchart_edge_marker_start_base};
#[cfg(any(feature = "canonical-edge-paint", test))]
use crate::projection::FlowchartEdgeKey;
use crate::{
    document as document_ids, edge_svg::flowchart_neo_mask_uses_dash_pairs,
    layout_config as render_config, style::FlowchartEdgeStylePlan,
};

mod agentflow;
mod cluster_geometry;
#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
mod compact;
mod defs;
pub(crate) mod edge_fill;
mod edge_geom;
pub(crate) mod edge_label_cutout;
mod hierarchy;
mod label;
mod layout;
mod paint;
mod render;
mod render_input;
mod step;
mod svg_emit;
mod swimlane;
mod swimlane_layout;
mod types;
mod util;
mod viewbox;
mod viewbox_node_bounds;

#[cfg(feature = "test-support")]
#[doc(hidden)]
pub mod test_support {
    pub use super::paint::{
        FlowchartEdgeLabels, prepare_edge_routes, prepare_query_rough_paths,
        with_flowchart_paint_context,
    };
    pub use super::render::cluster::{
        advance_rough_geometry as advance_cluster_rough_geometry, render_flowchart_cluster,
    };
    pub use super::render::node::{
        advance_rough_geometry as advance_node_rough_geometry, render_flowchart_node,
    };
    pub use super::types::{
        FlowchartEdgeDataPointsScratch, FlowchartLayoutEdgeRef, FlowchartRenderCtx,
        FlowchartRenderDetails,
    };

    #[cfg(all(feature = "diagram-flowchart", feature = "layout-dagre"))]
    pub use super::paint::prepare_dagre_linear_edge_route;
}

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    any(
        feature = "layout-dagre",
        feature = "layout-elk",
        feature = "layout-shoal"
    )
))]
pub use compact::render_native_compact;
#[cfg(feature = "layout-grid")]
pub use layout::{FlowchartGridPaint, GridLineHops};
pub use layout::{FlowchartLayoutDom, FlowchartSvgLayout, FlowchartSvgRoute, SwimlaneSvgLayout};
pub use paint::edge_geometry;
pub use svg_emit::{FlowchartSvgModelRequest, render_parity};
pub use swimlane_layout::project_swimlane_layout;

use edge_geom::{FlowchartEdgePathGeomRequest, flowchart_compute_edge_path_geom};
use hierarchy::*;
use label::*;
use merman_render_core::number_format::canonical_number as fmt;
use merman_render_core::number_format::canonical_number as fmt_display;
use path_bounds::{svg_path_bounds_from_d, svg_path_length_from_d};
pub(crate) use render::FlowchartRootPaint;
use render::{render_flowchart_edge_label, render_flowchart_edge_path, render_flowchart_node};
pub(crate) use types::FlowchartEdgeDataPointsScratch;
use types::*;

type SvgDiagramId<'a> = SvgId<'a>;

mod path_bounds {
    pub(super) use merman_render_core::rough_svg::paths::svg_path_bounds_from_rough_ops;
    pub(super) use merman_render_core::svg_bounds::{SvgPathBounds, svg_path_bounds_from_d};
    pub(super) use merman_render_core::svg_edge_parity::svg_path_length_from_d;
}

#[inline]
fn escape_attr(text: &str) -> merman_render_core::xml::EscapeAttrDisplay<&str> {
    escape_attr_display(text)
}

/// The classic marker geometry, for callers that do not have a prepared edge style.
#[cfg(any(feature = "canonical-edge-paint", test))]
pub fn flowchart_marker_metrics(
    edge: &FlowEdge,
) -> (
    merman_render_core::route_types::Marker,
    merman_render_core::route_types::Marker,
) {
    flowchart_marker_variant_metrics(edge, false)
}

#[cfg(any(feature = "canonical-edge-paint", test))]
fn flowchart_marker_variant_metrics(
    edge: &FlowEdge,
    margin: bool,
) -> (
    merman_render_core::route_types::Marker,
    merman_render_core::route_types::Marker,
) {
    let (start, end) = edge_geom::arrow_types_for_edge(edge.edge_type.as_deref());
    (
        defs::measure_marker(
            flowchart_edge_marker_start_base(edge),
            edge_path::marker_offset_for(start),
            margin,
        ),
        defs::measure_marker(
            flowchart_edge_marker_end_base(edge),
            edge_path::marker_offset_for(end),
            margin,
        ),
    )
}

#[cfg(any(feature = "canonical-edge-paint", test))]
impl FlowchartEdgeStylePlan {
    #[doc(hidden)]
    pub fn marker_metrics_for(
        &self,
        key: FlowchartEdgeKey,
        edge: &FlowEdge,
        neo: bool,
    ) -> Result<(
        merman_render_core::route_types::Marker,
        merman_render_core::route_types::Marker,
    )> {
        Ok(flowchart_marker_variant_metrics(
            edge,
            self.marker_uses_margin_for(key, neo)?,
        ))
    }

    #[cfg(feature = "layout-shoal")]
    #[doc(hidden)]
    pub fn reserve_marker_stroke(
        &self,
        key: FlowchartEdgeKey,
        markers: (
            merman_render_core::route_types::Marker,
            merman_render_core::route_types::Marker,
        ),
    ) -> Result<(
        merman_render_core::route_types::Marker,
        merman_render_core::route_types::Marker,
    )> {
        let stroke_width = 2.0 * self.stroke_outset_for(key)?.unwrap_or_default();
        let reserve = |mut marker: merman_render_core::route_types::Marker| {
            if marker.width > 0.0 {
                // The viewport already includes the marker stroke. The shaft and marker
                // share a centerline, so either footprint can set the transverse demand.
                marker.width = marker.width.max(stroke_width);
            }
            marker
        };
        Ok((reserve(markers.0), reserve(markers.1)))
    }
}

/// Services borrowed from the same admitted render operation as the layout and label sidecars.
pub struct FlowchartSvgServices<'a> {
    pub diagram_id: SvgId<'a>,
    pub root_viewport: RootViewportContext<'a>,
    pub operation: &'a OperationContext,
    pub work_meter: &'a OperationWorkMeter,
    pub render_seed: u64,
    pub timing: RenderTiming,
    pub measurer: &'a dyn TextMeasurer,
    pub math_renderer: Option<&'a (dyn MathRenderer + Send + Sync)>,
    pub icon_registry: Option<&'a IconRegistry>,
    pub defer_palette_stylesheet: bool,
    pub native_canvas: Option<&'a Canvas>,
    pub canvas_background: Option<Color>,
    pub native_family_theme: Option<&'a FamilyTheme>,
    pub edge_trace: Option<(&'a str, &'a FlowchartEdgeTraceCollector)>,
}
