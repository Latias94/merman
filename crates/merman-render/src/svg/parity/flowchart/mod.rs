use super::*;
use merman_render_flow::{document as document_ids, layout_config as render_config};
use rustc_hash::{FxHashMap, FxHashSet};

mod agentflow;
mod cluster_geometry;
#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
mod compact;
mod defs;
mod edge;
mod edge_fill;
mod edge_geom;
mod hierarchy;
mod label;
mod node_inventory;
mod paint;
pub(crate) use node_inventory::{FlowchartNodeLayoutView, FlowchartPreparedNodes};
mod render;
mod render_input;
mod step;
mod swimlane;
mod types;
mod util;
mod viewbox;
mod viewbox_node_bounds;

#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
pub(super) use compact::render_native_compact_flowchart_artifact;
use edge::*;
pub(in crate::svg::parity::flowchart) use edge_geom::{
    FlowchartEdgePathGeomRequest, flowchart_compute_edge_path_geom,
};
use hierarchy::*;
pub(super) use label::*;
pub(crate) use merman_render_flow::style::FlowchartEdgeStylePlan;
pub(crate) use render_config::{FlowchartRenderConfig, prepare_flowchart_render_config};

/// Measure the same marker descriptors used by SVG emission in drawing units.
#[cfg(any(feature = "layout-shoal", test))]
pub(crate) fn flowchart_marker_metrics(
    edge: &crate::flowchart::FlowEdge,
) -> (crate::graph_layout::Marker, crate::graph_layout::Marker) {
    let (start, end) = edge_geom::arrow_types_for_edge(edge.edge_type.as_deref());
    (
        defs::measure_marker(
            flowchart_edge_marker_start_base(edge),
            super::edge_path::marker_offset_for(start),
        ),
        defs::measure_marker(
            flowchart_edge_marker_end_base(edge),
            super::edge_path::marker_offset_for(end),
        ),
    )
}

use render::{render_flowchart_edge_label, render_flowchart_edge_path, render_flowchart_node};
use types::*;

// Flowchart SVG renderer implementation (split from parity.rs).

// In flowchart SVG emission, many attribute payloads are known to be short-lived (colors, inline
// `d` strings, etc). Avoid allocating an owned `String` for attribute escaping by default.
#[inline]
fn escape_attr(text: &str) -> super::util::EscapeAttrDisplay<&str> {
    escape_attr_display(text)
}

// Entry points (split from parity.rs).

mod svg_emit;
pub(super) use svg_emit::{flowchart_edge_geometry, render_flowchart_svg_artifact};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) use swimlane::{render_swimlane_svg_artifact, swimlane_edge_geometry};

#[cfg(all(test, feature = "layout-elk"))]
mod browser_measurement_tests;
