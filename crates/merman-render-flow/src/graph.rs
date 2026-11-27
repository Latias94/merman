//! Shared Mermaid graph emission for Flowchart, Agentflow, and Swimlane.

mod emit;
mod plan;
mod step_badges;

pub use emit::{FlowchartClusterPass, FlowchartGraph, FlowchartGraphPaint, FlowchartGraphTimings};
pub use plan::{
    FlowchartHierarchyInputs, FlowchartHierarchyPlan, FlowchartNodeRootOrder,
    FlowchartNodeRootSchedule, FlowchartRootOffsets, flowchart_checked_sort_work,
};
pub use step_badges::{StepBadgePlacement, StepBadges};
