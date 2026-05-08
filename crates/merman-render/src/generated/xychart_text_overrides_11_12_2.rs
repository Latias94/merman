// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 XYChart bar data-label geometry constants in one
// place, so SVG parity code does not duplicate the same diagram-specific literals.

pub fn xychart_bar_data_label_scale_factor() -> f64 {
    0.7
}

pub fn xychart_bar_data_label_inset_px() -> f64 {
    10.0
}
