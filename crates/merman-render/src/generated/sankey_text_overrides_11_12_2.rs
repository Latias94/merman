// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Sankey node/label geometry constants in one
// place, so layout and SVG parity code do not duplicate the same diagram-specific literals.

const SANKEY_NODE_PADDING_BASE_PX: f64 = 10.0;
const SANKEY_NODE_PADDING_SHOW_VALUES_EXTRA_PX: f64 = 15.0;

pub fn sankey_node_width_px() -> f64 {
    10.0
}

pub fn sankey_node_padding_px(show_values: bool) -> f64 {
    SANKEY_NODE_PADDING_BASE_PX
        + if show_values {
            SANKEY_NODE_PADDING_SHOW_VALUES_EXTRA_PX
        } else {
            0.0
        }
}

pub fn sankey_label_font_size_px() -> f64 {
    14.0
}

pub fn sankey_label_gap_x_px() -> f64 {
    6.0
}

pub fn sankey_label_hide_values_dy_em() -> f64 {
    0.35
}
