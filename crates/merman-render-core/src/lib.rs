//! Shared layout geometry and SVG route painting.

#![forbid(unsafe_code)]

mod geometry;
pub mod route_types;

#[cfg(feature = "route-paint")]
pub mod route_paint;

pub use geometry::{Bounds, LayoutPoint};
