//! Railroad layout, presentation binding, and SVG emission.

use merman_render_core::text::TextStyle;

mod layout;
mod model;
mod style;
mod svg;

pub use layout::layout_railroad_diagram_typed;
pub use model::{
    RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadRuleLayout,
};
pub use style::{RailroadStyle, resolve_railroad_style};
pub use svg::write_svg_contents;

type MeasureRawSvgBBox<'a> = dyn Fn(&str, &TextStyle) -> (f64, f64) + 'a;
