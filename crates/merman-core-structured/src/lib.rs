//! Syntax parsers for structured Mermaid diagram families.

#![forbid(unsafe_code)]

#[cfg(feature = "diagram-cynefin")]
#[doc(hidden)]
pub mod cynefin;

#[cfg(feature = "diagram-gantt")]
#[doc(hidden)]
pub mod gantt;

#[cfg(feature = "diagram-info")]
#[doc(hidden)]
pub mod info;

#[cfg(feature = "diagram-ishikawa")]
#[doc(hidden)]
pub mod ishikawa;

#[cfg(feature = "diagram-journey")]
#[doc(hidden)]
pub mod journey;

#[cfg(feature = "diagram-packet")]
pub mod packet;

#[cfg(feature = "diagram-pie")]
#[doc(hidden)]
pub mod pie;

#[cfg(feature = "diagram-quadrant-chart")]
#[doc(hidden)]
pub mod quadrant_chart;

#[cfg(feature = "diagram-radar")]
#[doc(hidden)]
pub mod radar;

#[cfg(feature = "diagram-railroad")]
pub mod railroad;

#[cfg(feature = "diagram-sankey")]
#[doc(hidden)]
pub mod sankey;

#[cfg(feature = "diagram-timeline")]
#[doc(hidden)]
pub mod timeline;

#[cfg(feature = "diagram-tree-view")]
#[doc(hidden)]
pub mod tree_view;

#[cfg(feature = "diagram-treemap")]
#[doc(hidden)]
pub mod treemap;

#[cfg(feature = "diagram-venn")]
#[doc(hidden)]
pub mod venn;

#[cfg(feature = "diagram-xychart")]
#[doc(hidden)]
pub mod xychart;
