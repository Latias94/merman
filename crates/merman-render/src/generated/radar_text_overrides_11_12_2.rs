// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Radar legend geometry constants in one place, so
// layout and SVG parity code do not duplicate the same diagram-specific literals.

pub fn radar_legend_line_step_y_px() -> f64 {
    20.0
}

pub fn radar_legend_box_size_px() -> f64 {
    12.0
}

pub fn radar_legend_label_x_px() -> f64 {
    16.0
}

pub fn radar_legend_label_baseline_y_px() -> f64 {
    0.0
}
