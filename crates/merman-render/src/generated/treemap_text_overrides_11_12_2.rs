// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Treemap section-header text/layout constants in
// one place, so layout and SVG parity code do not duplicate the same diagram-specific literals.

pub fn treemap_section_inner_padding_px() -> f64 {
    10.0
}

pub fn treemap_section_header_height_px() -> f64 {
    25.0
}

pub fn treemap_section_header_center_y_px() -> f64 {
    treemap_section_header_height_px() / 2.0
}

pub fn treemap_section_header_label_inset_x_px() -> f64 {
    6.0
}

pub fn treemap_section_label_font_size_px() -> f64 {
    12.0
}

pub fn treemap_section_value_font_size_px() -> f64 {
    10.0
}

pub fn treemap_section_value_right_inset_px() -> f64 {
    10.0
}

pub fn treemap_section_label_reserved_value_width_px() -> f64 {
    30.0
}

pub fn treemap_section_label_value_gap_px() -> f64 {
    10.0
}

pub fn treemap_section_label_min_visible_width_px() -> f64 {
    15.0
}
