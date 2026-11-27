use super::*;
use rustc_hash::{FxHashMap, FxHashSet};

mod agentflow;
mod cluster_geometry;
#[cfg(all(
    feature = "native-compact",
    feature = "diagram-flowchart",
    feature = "layout-dagre"
))]
mod compact;
mod css;
mod defs;
mod document_ids;
mod edge;
mod edge_fill;
mod edge_geom;
mod edge_style_plan;
mod hierarchy;
mod label;
mod node_inventory;
mod node_style;
mod paint;
pub(crate) use node_inventory::{FlowchartNodeLayoutView, FlowchartPreparedNodes};
mod render;
mod render_config;
mod render_input;
mod step;
mod style;
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
pub(crate) use edge_style_plan::FlowchartEdgeStylePlan;
use hierarchy::*;
pub(super) use label::*;
pub(crate) use render_config::{FlowchartRenderConfig, prepare_flowchart_render_config};
pub(super) use style::*;

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

pub(super) use render::render_flowchart_cluster;
use render::{
    FlowchartRootRenderSession, render_flowchart_edge_label, render_flowchart_edge_path,
    render_flowchart_elk_root_groups, render_flowchart_node, render_flowchart_root,
};
use types::*;
use util::OptionalStyleAttr;

// Flowchart SVG renderer implementation (split from parity.rs).

// Mermaid's `createText(...)` defaults its `width` argument to 200. Flowchart edge labels call
// `createText(...)` without overriding that width, so keep edge label wrapping/max-width fixed at
// 200px (independent of `flowchart.wrappingWidth`).
pub(in crate::svg::parity::flowchart) const FLOWCHART_EDGE_LABEL_WRAP_WIDTH: f64 = 200.0;

// In flowchart SVG emission, many attribute payloads are known to be short-lived (colors, inline
// `d` strings, etc). Avoid allocating an owned `String` for attribute escaping by default.
#[inline]
fn escape_attr(text: &str) -> super::util::EscapeAttrDisplay<&str> {
    escape_attr_display(text)
}

pub(in crate::svg::parity::flowchart) fn flowchart_config_diagram_look(
    config: &merman_model::presentation::PresentationConfig,
) -> crate::config::DiagramLook<'_> {
    crate::config::mermaid_config_diagram_look(config)
}

// Entry points (split from parity.rs).

mod svg_emit;
pub(super) use svg_emit::{flowchart_edge_geometry, render_flowchart_svg_artifact};
#[cfg(any(feature = "diagram-flowchart", feature = "diagram-swimlane"))]
pub(super) use swimlane::{render_swimlane_svg_artifact, swimlane_edge_geometry};

#[cfg(all(test, feature = "layout-elk"))]
mod browser_measurement_tests;
