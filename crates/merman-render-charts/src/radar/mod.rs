//! Radar layout and geometry projection.

mod config;
mod layout;
mod model;
mod point_list;

pub use layout::layout_radar_diagram_typed_with_work_meter;
pub use model::{
    RadarAxisLayout, RadarCurveLayout, RadarDiagramLayout, RadarGraticuleShapeLayout,
    RadarLegendItemLayout,
};
