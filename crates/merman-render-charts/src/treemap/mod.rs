//! Typed Treemap layout and leaf SVG elements.

mod config;
mod layout;
mod model;
mod svg;
mod value_format;
mod viewport;

pub use layout::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX,
    layout_treemap_diagram_typed_with_work_meter,
};
pub use model::{TreemapDiagramLayout, TreemapLeafLayout, TreemapSectionLayout};
pub use svg::{
    TreemapLeafRectPaint, write_treemap_leaf_group_open, write_treemap_leaf_rect_and_clip,
};
pub use value_format::format_treemap_value;
pub use viewport::treemap_view_box;
