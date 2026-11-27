//! Shared layout geometry, SVG emission, and render resource admission.

#![forbid(unsafe_code)]

#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod config;
#[cfg(all(feature = "euclid-geometry", feature = "layout-dagre"))]
#[doc(hidden)]
pub mod dagre_geometry;
#[cfg(feature = "layout-elk")]
#[doc(hidden)]
pub mod elk_adapter;
#[cfg(any(feature = "layout-elk", feature = "svg-output"))]
#[doc(hidden)]
pub mod elk_edge_geometry;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod entities;
#[cfg(feature = "error-diagram")]
#[doc(hidden)]
pub mod error_diagram;
#[cfg(feature = "euclid-geometry")]
#[doc(hidden)]
pub mod euclid_box;
mod geometry;
#[cfg(feature = "graph-layout")]
#[doc(hidden)]
pub mod graph_layout;
#[cfg(feature = "html")]
#[doc(hidden)]
pub mod html;
#[cfg(feature = "icon-registry")]
#[doc(hidden)]
pub mod icon_registry;
mod layout;
#[cfg(feature = "resources")]
#[doc(hidden)]
pub mod layout_work;
#[cfg(feature = "math-labels")]
#[doc(hidden)]
pub mod math;
#[cfg(feature = "css-style")]
#[doc(hidden)]
pub mod mermaid_style;
#[cfg(any(feature = "svg-output", feature = "route-paint"))]
#[doc(hidden)]
pub mod number_format;
#[cfg(feature = "paint-scene")]
#[doc(hidden)]
pub mod paint_scene;
#[cfg(feature = "resources")]
mod render_error;
#[cfg(feature = "resources")]
pub mod resources;
#[cfg(feature = "root-svg")]
#[doc(hidden)]
pub mod root_svg;
#[cfg(feature = "rough-svg")]
#[doc(hidden)]
pub mod rough_svg;
#[doc(hidden)]
pub mod route_types;

#[cfg(feature = "route-paint")]
#[doc(hidden)]
pub mod route_paint;

#[cfg(any(
    feature = "layout-dagre",
    feature = "layout-elk",
    feature = "svg-output"
))]
#[doc(hidden)]
pub mod shape_intersection;
#[cfg(feature = "svg-animation")]
#[doc(hidden)]
pub mod step_animation;
#[cfg(feature = "svg-bounds")]
#[doc(hidden)]
pub mod svg_bounds;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_css;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_edge_parity;
#[cfg(feature = "svg-fallback")]
#[doc(hidden)]
pub mod svg_fallback;
#[cfg(feature = "svg-output")]
#[doc(hidden)]
pub mod svg_output;
#[cfg(feature = "svg-safety")]
#[doc(hidden)]
pub mod svg_safety;
#[cfg(any(feature = "svg-parse", feature = "svg-bounds"))]
#[doc(hidden)]
pub mod svg_scan;
#[cfg(feature = "html")]
#[doc(hidden)]
pub mod svg_security;
#[cfg(feature = "svg-validation")]
#[doc(hidden)]
pub mod svg_validation;
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
#[doc(hidden)]
pub use layout::measure_layout_bounds;
pub use layout::{EdgePaintGeometry, LayoutCluster, LayoutEdge, LayoutLabel, LayoutNode};
#[doc(hidden)]
pub use layout::{
    SwimlaneDirection, SwimlaneEdgeLayout, SwimlaneLaneLayout, SwimlaneNodeLayout,
    SwimlaneTitleRect,
};
#[cfg(feature = "resources")]
pub use render_error::{Error, RenderCapability, RenderCapabilityPolicy, Result};
