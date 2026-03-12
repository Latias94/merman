// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Architecture text-bbox constants in one place,
// so layout and SVG parity code do not duplicate the same diagram-specific measurements inline.

pub fn architecture_icon_text_bbox_height_px(font_size_px: f64, line_count: usize) -> f64 {
    (line_count.max(1) as f64) * font_size_px.max(1.0) * 1.1875
}

pub fn architecture_create_text_bbox_height_px(font_size_px: f64, line_count: usize) -> f64 {
    let font_size_px = font_size_px.max(1.0);
    let extra_lines = line_count.max(1).saturating_sub(1) as f64;
    font_size_px * ((19.0 / 16.0) + extra_lines * 1.1)
}

pub fn architecture_create_text_root_label_extra_bottom_px(
    font_size_px: f64,
    line_count: usize,
) -> f64 {
    let font_size_px = font_size_px.max(1.0);
    let extra_lines = line_count.max(1).saturating_sub(1) as f64;
    font_size_px * ((24.1875 / 16.0) + extra_lines * 1.1)
}

pub fn architecture_create_text_compound_label_extra_bottom_px(font_size_px: f64) -> f64 {
    font_size_px.max(1.0) * (17.0 / 16.0)
}

pub fn architecture_cytoscape_canvas_label_width_scale() -> f64 {
    1.055
}

pub fn architecture_service_label_bottom_extension_px() -> f64 {
    18.0
}

pub fn architecture_singleton_icon_text_service_offset_y_px() -> f64 {
    architecture_service_label_bottom_extension_px()
}
