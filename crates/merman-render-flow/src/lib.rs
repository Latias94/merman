//! Layout configuration, shape vocabulary, and geometry for flow diagrams.

#![forbid(unsafe_code)]

#[doc(hidden)]
pub mod bpmn;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod class_style;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod cluster_shell;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod document;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod edge_geometry;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod edge_labels;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod edge_svg;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod graph;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod hierarchy;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod source_style;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod style;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod stylesheet;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod svg;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod svg_label;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod typography;

#[cfg(feature = "layout-dagre")]
#[doc(hidden)]
pub mod dagre;
#[cfg(all(
    feature = "layout-elk",
    any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    )
))]
#[doc(hidden)]
pub mod elk;
#[cfg(all(
    feature = "layout-grid",
    any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    )
))]
#[doc(hidden)]
pub mod grid;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod lane_title;
#[cfg(all(
    feature = "layout-layered",
    any(
        feature = "diagram-flowchart",
        feature = "diagram-agentflow",
        feature = "diagram-swimlane"
    )
))]
#[doc(hidden)]
pub mod layered;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod layout_config;
#[cfg(all(
    any(
        feature = "layout-elk",
        feature = "layout-grid",
        feature = "layout-layered",
        feature = "layout-shoal"
    ),
    any(
        feature = "diagram-agentflow",
        feature = "diagram-flowchart",
        feature = "diagram-swimlane"
    )
))]
#[doc(hidden)]
pub mod measured;
#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod projection;
#[doc(hidden)]
pub mod shapes;
#[cfg(feature = "layout-swimlane")]
#[doc(hidden)]
pub mod swimlane;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod markers;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod node_body;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod node_geometry;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod node_inventory;

#[cfg(any(
    feature = "diagram-agentflow",
    feature = "diagram-flowchart",
    feature = "diagram-swimlane"
))]
#[doc(hidden)]
pub mod theme;
