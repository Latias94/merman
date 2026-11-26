//! Typed Treemap layout and layout geometry shared with the SVG writer.

mod config;
mod layout;
mod model;

pub use layout::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX,
    layout_treemap_diagram_typed_with_work_meter,
};
pub use model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
