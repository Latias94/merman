// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Pie fixed text/layout geometry constants in one
// place, so layout and SVG parity code do not duplicate the same diagram-specific literals.

pub fn pie_margin_px() -> f64 {
    40.0
}

pub fn pie_legend_rect_size_px() -> f64 {
    18.0
}

pub fn pie_legend_spacing_px() -> f64 {
    4.0
}

pub fn pie_center_x_px() -> f64 {
    225.0
}

pub fn pie_center_y_px() -> f64 {
    225.0
}

pub fn pie_radius_px() -> f64 {
    185.0
}

pub fn pie_outer_radius_px() -> f64 {
    186.0
}

pub fn pie_label_radius_px(radius_px: f64) -> f64 {
    radius_px.max(0.0) * 0.75
}

pub fn pie_legend_x_px() -> f64 {
    12.0 * pie_legend_rect_size_px()
}

pub fn pie_legend_label_font_size_px() -> f64 {
    17.0
}

pub fn pie_title_y_px() -> f64 {
    -200.0
}

pub fn pie_legend_text_x_px() -> f64 {
    pie_legend_rect_size_px() + pie_legend_spacing_px()
}

pub fn pie_legend_text_y_px() -> f64 {
    14.0
}
