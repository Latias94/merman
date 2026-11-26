//! ER relationship marker geometry and SVG definitions.

mod markers;

pub use markers::{ErMarkerDefs, er_marker_id, er_unified_marker_id, write_er_marker_definitions};
#[cfg(feature = "layout-shoal")]
pub use markers::{er_canonical_marker, er_marker_paint_bounds};
