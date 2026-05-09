// This file is intentionally small and hand-curated.
//
// We use this override to keep the Mermaid@11.12.2 Radar legend row spacing in one place, so
// layout code does not duplicate the same diagram-specific literal.

pub fn radar_legend_line_step_y_px() -> f64 {
    20.0
}
