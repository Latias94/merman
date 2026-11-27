//! Shared Mermaid-compatible path geometry and after-paint overlays.
//!
//! The SVG facade exposes trace data with `svg-output` alone.

#[cfg(feature = "svg-edge-parity")]
pub mod curve;
#[cfg(feature = "svg-edge-parity")]
pub mod edge_label_geometry;
#[cfg(feature = "svg-edge-parity")]
pub mod edge_path;
#[cfg(all(feature = "svg-edge-parity", feature = "layout-layered"))]
pub mod layered_route;
#[cfg(feature = "svg-edge-parity")]
pub mod line_hops;
#[cfg(feature = "svg-edge-parity")]
pub mod markers;
#[cfg(feature = "svg-edge-parity")]
pub mod path_numbers;
pub mod trace;

#[cfg(feature = "svg-edge-parity")]
mod path_length;
#[cfg(feature = "svg-edge-parity")]
pub use path_length::svg_path_length_from_d;
