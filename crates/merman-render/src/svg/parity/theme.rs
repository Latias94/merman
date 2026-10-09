mod helpers;

use helpers::*;

#[cfg(feature = "diagram-radar")]
pub(crate) fn radar_default_series_color(index: usize) -> &'static str {
    default_c_scale(index)
}

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
