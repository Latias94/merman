//! Shared layout geometry, SVG emission, and render resource admission.

#![forbid(unsafe_code)]

#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod config;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod entities;
mod geometry;
#[cfg(feature = "graph-layout")]
#[doc(hidden)]
pub mod graph_layout;
#[cfg(feature = "icon-registry")]
#[doc(hidden)]
pub mod icon_registry;
mod layout;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod layout_work;
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
#[cfg(feature = "root-svg")]
#[doc(hidden)]
pub mod root_svg;
#[cfg(feature = "rough-svg")]
#[doc(hidden)]
pub mod rough_svg;
pub mod route_types;

#[cfg(feature = "route-paint")]
pub mod route_paint;

#[cfg(feature = "svg-animation")]
#[doc(hidden)]
pub mod step_animation;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_css;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_output;
#[cfg(feature = "svg-parse")]
#[doc(hidden)]
pub mod svg_scan;
#[cfg(feature = "svg-parse")]
#[doc(hidden)]
pub mod svg_xml;
#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod text;
#[cfg(feature = "theme-svg")]
#[doc(hidden)]
pub mod theme_svg;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod xml;
#[cfg(any(feature = "icon-registry", feature = "svg-output"))]
mod xml_chars;

pub use geometry::{Bounds, LayoutPoint};
pub use layout::{LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode};
#[cfg(feature = "svg-output")]
pub use render_error::{Error, RenderCapability, RenderCapabilityPolicy, Result};
