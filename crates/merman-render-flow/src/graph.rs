//! Shared Mermaid graph emission for Flowchart, Agentflow, and Swimlane.

#[cfg(feature = "canonical-edge-paint")]
mod axis_routes;
mod emit;
mod plan;
mod step_badges;

#[cfg(feature = "layout-shoal")]
pub use axis_routes::FlowchartSvgEdgeGeometry;
#[cfg(feature = "canonical-edge-paint")]
pub use axis_routes::{AxisRoutes, RouteTopology};
pub use emit::{FlowchartClusterPass, FlowchartGraph, FlowchartGraphPaint, FlowchartGraphTimings};
pub use plan::{
    FlowchartHierarchyInputs, FlowchartHierarchyPlan, FlowchartNodeRootOrder,
    FlowchartNodeRootSchedule, FlowchartRootOffsets, flowchart_checked_sort_work,
};
pub use step_badges::{StepBadgePlacement, StepBadges};
