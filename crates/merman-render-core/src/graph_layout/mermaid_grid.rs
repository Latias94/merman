//! Mermaid Grid's measured layout and geometry contracts.

mod config;
mod forest;
mod geometry;
mod label_positioning;
mod label_spacing;
mod placement;
mod port_plan;
mod prepared;
mod route_plan;
pub mod routing;
mod routing_context;
mod routing_session;
mod sparse_search;
mod sparse_segment;
mod sparse_topology;

pub use config::{GridLineHops, GridPaint};
pub use forest::{GridItemKind, GridMeasuredItem};
pub use geometry::GridGroupMeasure;
pub use placement::GridItem;
pub use prepared::{GridEdgeLabel, GridMeasuredEdge, GridRoutedEdge, PreparedGrid, prepare_grid};
