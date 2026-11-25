//! Syntax shared by Mermaid Flowchart, Swimlane, and Agentflow presentation statements.

#![forbid(unsafe_code)]

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub mod flowchart;
