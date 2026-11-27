//! Source parsing for Mermaid Flowchart and Swimlane, with shared Agentflow directive lexing.

#![forbid(unsafe_code)]

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub mod flowchart;
