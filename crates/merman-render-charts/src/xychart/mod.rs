//! XYChart preparation, layout, presentation, and SVG emission.

mod css_binding;
mod labels;
mod layout;
mod model;
mod palette;
mod preparation;
mod series;
mod svg;
mod theme;
mod typography;

#[cfg(test)]
mod tests;

pub use labels::{
    XY_CHART_DATA_LABEL_INSET_PX, XyChartRectDataLabel, XyChartRectDataLabels, rect_data_labels,
};
pub use layout::{XyChartLayoutTypography, layout_xychart_diagram_typed};
pub use model::{
    XyChartDiagramLayout, XyChartDrawableElem, XyChartPathData, XyChartRectData, XyChartTextData,
};
pub use preparation::{XyChartStyle, prepare_xychart_diagram};
pub use series::XyChartSeriesPaintPlan;
pub use svg::{XyChartSvgContext, render_xychart_svg};
pub use typography::{XyChartRoleFonts, XyChartTextRole};
