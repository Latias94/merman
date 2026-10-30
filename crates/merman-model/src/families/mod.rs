//! Canonical family data, independently selected from source parsers and output handlers.

#[cfg(feature = "diagram-architecture")]
pub mod architecture;
#[cfg(feature = "diagram-c4")]
pub mod c4;
#[cfg(feature = "diagram-class")]
pub mod class;
#[cfg(feature = "diagram-cynefin")]
pub mod cynefin;
#[cfg(feature = "diagram-er")]
pub mod er;
pub mod error;
#[cfg(feature = "diagram-event-modeling")]
pub mod event_modeling;
#[cfg(any(
    feature = "diagram-flowchart",
    feature = "diagram-swimlane",
    feature = "diagram-agentflow"
))]
pub mod flowchart;
#[cfg(feature = "diagram-gantt")]
pub mod gantt;
#[cfg(feature = "diagram-git-graph")]
pub mod git_graph;
#[cfg(feature = "diagram-quadrant-chart")]
pub mod quadrant_chart;
#[cfg(feature = "diagram-requirement")]
pub mod requirement;
#[cfg(feature = "diagram-sankey")]
pub mod sankey;
#[cfg(feature = "diagram-sequence")]
pub mod sequence;
#[cfg(feature = "diagram-state")]
pub mod state;
#[cfg(feature = "diagram-tree-view")]
pub mod tree_view;
#[cfg(feature = "diagram-venn")]
pub mod venn;
#[cfg(feature = "diagram-wardley")]
pub mod wardley;
#[cfg(feature = "diagram-zenuml")]
pub mod zenuml;

#[cfg(feature = "diagram-mindmap")]
pub mod mindmap;
#[cfg(feature = "diagram-xychart")]
pub mod xy_chart;
