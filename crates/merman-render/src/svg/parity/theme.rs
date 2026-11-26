mod helpers;

use helpers::*;

#[cfg(any(
    feature = "diagram-mindmap",
    feature = "diagram-radar",
    feature = "diagram-treemap"
))]
pub(crate) use helpers::default_c_scale;

#[cfg(feature = "diagram-timeline")]
pub(crate) fn timeline_default_section_colors(index: usize) -> [&'static str; 3] {
    [
        default_c_scale(index),
        default_c_scale_label(index),
        default_c_scale_inv(index),
    ]
}

#[cfg(feature = "diagram-kanban")]
pub(crate) fn kanban_section_defaults(index: usize) -> [&'static str; 3] {
    [
        default_c_scale(index),
        default_c_scale_label(index),
        default_c_scale_inv(index),
    ]
}

#[cfg(all(test, feature = "all-diagrams"))]
mod tests;
