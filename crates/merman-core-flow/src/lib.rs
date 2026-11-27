//! Source parsing for Mermaid Flowchart, Swimlane, and Agentflow.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-agentflow")]
#[doc(hidden)]
pub mod agentflow;

#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub mod flowchart;
