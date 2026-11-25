//! Shared layout geometry, SVG emission, and render resource admission.

#![forbid(unsafe_code)]

#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod config;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod entities;
mod geometry;
mod layout;
#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod mermaid_style;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod number_format;
#[cfg(feature = "svg-output")]
mod render_error;
#[cfg(feature = "resources")]
pub mod resources;
pub mod route_types;

#[cfg(feature = "route-paint")]
pub mod route_paint;

#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_css;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_output;
#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod text;
#[cfg(feature = "theme-svg")]
#[doc(hidden)]
pub mod theme_svg;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod xml;

pub use geometry::{Bounds, LayoutPoint};
pub use layout::{LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode};
#[cfg(feature = "svg-output")]
pub use render_error::{Error, RenderCapability, RenderCapabilityPolicy, Result};
