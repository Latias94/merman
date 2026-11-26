//! Pie layout, presentation bindings, and SVG contents.

mod config;
mod geometry;
mod layout;
mod model;
mod native_theme;
mod svg;
mod theme;

pub use config::{PieConfigView, PieLegendPosition, PieRenderSettings};
pub use geometry::{native_pie_slice_path, pie_group_offset, pie_slice_class, pie_slice_path};
pub use layout::{layout_mermaid_pie, native_legend_baseline_y, pie_legend_text};
pub use model::{PieDiagramLayout, PieLegendItemLayout, PieSliceLayout};
pub use native_theme::PieNativeStyle;
pub use svg::{
    PieNativeSvgContext, write_mermaid_css_rules, write_mermaid_svg_body, write_native_svg_body,
    write_svg_accessibility,
};
pub use theme::{PieMermaidCss, PieMermaidStyle, PieTextRole};

pub const PIE_LEGEND_RECT_SIZE_PX: f64 = 18.0;
pub const PIE_LEGEND_SPACING_PX: f64 = 4.0;
pub const PIE_TITLE_BASELINE_Y_PX: f64 = -200.0;
pub const PIE_TITLE_CLASS: &str = "pieTitleText";

const PIE_MARGIN_PX: f64 = 40.0;
