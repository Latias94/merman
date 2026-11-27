//! Shared Mermaid graph emission for Flowchart, Agentflow, and Swimlane.

mod emit;
mod plan;

pub use emit::{FlowchartClusterPass, FlowchartGraph, FlowchartGraphPaint, FlowchartGraphTimings};
pub use plan::{
    FlowchartHierarchyInputs, FlowchartHierarchyPlan, FlowchartNodeRootOrder,
    FlowchartNodeRootSchedule, FlowchartRootOffsets, flowchart_checked_sort_work,
};
