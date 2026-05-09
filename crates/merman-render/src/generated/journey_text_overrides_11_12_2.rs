// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Journey fixed text/face geometry constants in one
// place, so layout and SVG parity code do not duplicate the same diagram-specific literals.

pub fn journey_legend_circle_r_px() -> f64 {
    7.0
}

pub fn journey_viewbox_top_pad_px() -> f64 {
    25.0
}

pub fn journey_title_extra_height_px() -> f64 {
    70.0
}

pub fn journey_face_radius_px() -> f64 {
    15.0
}

pub fn journey_face_base_y_px() -> f64 {
    300.0
}

pub fn journey_face_score_step_y_px() -> f64 {
    30.0
}
