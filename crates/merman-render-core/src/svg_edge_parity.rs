//! Shared Mermaid-compatible path geometry and after-paint overlays.

pub mod curve;
pub mod edge_label_geometry;
pub mod edge_path;
pub mod line_hops;
pub mod markers;
pub mod path_numbers;
pub mod trace;

mod path_length;
pub use path_length::svg_path_length_from_d;
