#[cfg(any(
    feature = "diagram-mindmap",
    feature = "diagram-radar",
    feature = "diagram-treemap"
))]
pub(crate) use merman_render_core::svg_css::default_c_scale;

#[cfg(feature = "diagram-kanban")]
pub(crate) use merman_render_core::svg_css::default_section_colors as kanban_section_defaults;

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
