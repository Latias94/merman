//! Canonical family data, independently selected from source parsers and output handlers.

#[cfg(feature = "diagram-architecture")]
pub mod architecture;
#[cfg(feature = "diagram-c4")]
pub mod c4;
#[cfg(feature = "diagram-cynefin")]
pub mod cynefin;
pub mod error;
#[cfg(feature = "diagram-event-modeling")]
pub mod event_modeling;
#[cfg(feature = "diagram-quadrant-chart")]
pub mod quadrant_chart;
#[cfg(feature = "diagram-requirement")]
pub mod requirement;
#[cfg(feature = "diagram-sankey")]
pub mod sankey;
#[cfg(feature = "diagram-venn")]
pub mod venn;
#[cfg(feature = "diagram-wardley")]
pub mod wardley;
