//! Layout configuration, shape vocabulary, and geometry for flow diagrams.

#![forbid(unsafe_code)]

#[doc(hidden)]
pub mod bpmn;
#[cfg(feature = "layout-dagre")]
#[doc(hidden)]
pub mod dagre;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod layout_config;
#[doc(hidden)]
pub mod shapes;
