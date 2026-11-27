//! Canonical route storage for Flowchart, Agentflow, and Swimlane.

use merman_render_core::graph_layout::{
    Axis, BorderStitch, Endpoint, KnotKind, Side, axis_clearance::AxisClearance,
    contour::ShapePortProfile,
};
#[cfg(feature = "layout-shoal")]
use merman_render_core::{LayoutLabel, LayoutPoint};

/// Render-only metadata for coordinates in occurrence order.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteTopology {
    pub endpoints: (Endpoint, Endpoint),
    pub ports: (Side, Side),
    pub knots: Vec<(Axis, KnotKind)>,
    pub corridor_clearance: f64,
}

#[cfg(feature = "layout-shoal")]
#[derive(Debug)]
pub struct FlowchartSvgEdgeGeometry {
    pub points: Vec<LayoutPoint>,
    pub label: Option<LayoutLabel>,
}

/// Routes indexed by semantic occurrence, with holes for hidden edges.
#[derive(Debug)]
pub struct AxisRoutes {
    pub nodes: Vec<ShapePortProfile>,
    pub group_parents: Vec<Option<u32>>,
    pub struck_group_titles: rustc_hash::FxHashSet<String>,
    pub border_stitches: Vec<BorderStitch>,
    pub clearance: AxisClearance,
    pub routes: Vec<Option<RouteTopology>>,
    /// Geometry in Flowchart edge-owner order when preparing SVG without owned layout edges.
    #[cfg(feature = "layout-shoal")]
    pub svg_edges: Option<Vec<FlowchartSvgEdgeGeometry>>,
}
