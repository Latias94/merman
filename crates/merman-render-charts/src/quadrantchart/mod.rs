//! Quadrant Chart layout, presentation binding, and SVG emission.

mod config;
mod css_binding;
mod layout;
mod model;
mod native_theme;
mod svg;
mod theme;

pub use config::QuadrantChartLayoutSettings;
pub use css_binding::QuadrantChartCssBinding;
pub use layout::layout_quadrantchart_diagram_typed;
pub use model::{
    QuadrantChartAxisLabelData, QuadrantChartBorderLineData, QuadrantChartDiagramLayout,
    QuadrantChartPointData, QuadrantChartQuadrantData, QuadrantChartTextData,
};
pub use svg::{QuadrantChartSvgContext, render_quadrantchart_svg, write_svg_body};
pub use theme::{QuadrantChartPointThemePlan, prepare_quadrantchart_diagram};
