//! Shared layout geometry, SVG route painting, and render resource admission.

#![forbid(unsafe_code)]

mod geometry;
#[cfg(feature = "resources")]
pub mod resources;
pub mod route_types;

#[cfg(feature = "route-paint")]
pub mod route_paint;

pub use geometry::{Bounds, LayoutPoint};
