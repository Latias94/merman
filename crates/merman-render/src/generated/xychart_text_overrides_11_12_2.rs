// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 XYChart bar data-label geometry constants in one
// place, so SVG parity code does not duplicate the same diagram-specific literals.

pub fn xychart_bar_data_label_char_width_factor() -> f64 {
    0.7
}

pub fn xychart_horizontal_bar_data_label_font_height_factor() -> f64 {
    0.7
}

pub fn xychart_horizontal_bar_data_label_right_inset_px() -> f64 {
    10.0
}

pub fn xychart_vertical_bar_data_label_top_inset_px() -> f64 {
    10.0
}
