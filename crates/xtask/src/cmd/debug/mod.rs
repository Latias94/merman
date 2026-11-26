//! General SVG debug utilities.

mod svg;

#[cfg(feature = "render-parity")]
pub(crate) use svg::debug_svg_bbox;
pub(crate) use svg::debug_svg_data_points;
