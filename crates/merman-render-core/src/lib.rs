//! Shared layout geometry, SVG emission, and render resource admission.

#![forbid(unsafe_code)]

mod geometry;
#[cfg(feature = "svg-output")]
mod render_error;
#[cfg(feature = "resources")]
pub mod resources;
pub mod route_types;

#[cfg(feature = "route-paint")]
pub mod route_paint;

#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_output;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod xml;

pub use geometry::{Bounds, LayoutPoint};
#[cfg(feature = "svg-output")]
pub use render_error::{Error, RenderCapability, RenderCapabilityPolicy, Result};
