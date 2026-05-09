// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Pie legend rectangle size and spacing in one
// place, so layout and SVG parity code do not duplicate the shared legend geometry literals.

pub fn pie_legend_rect_size_px() -> f64 {
    18.0
}

pub fn pie_legend_spacing_px() -> f64 {
    4.0
}
