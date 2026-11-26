//! Treemap presentation bindings.

mod css_binding;
mod theme;

pub(crate) use merman_render_charts::treemap::{
    TREEMAP_SECTION_HEADER_HEIGHT_PX, TREEMAP_SECTION_INNER_PADDING_PX,
    layout_treemap_diagram_typed_with_work_meter,
};
pub(crate) use theme::TreemapStyle;

pub(crate) const TREEMAP_TITLE_CLASS: &str = "treemapTitle";
