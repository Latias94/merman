// This file is intentionally small and hand-curated.
//
// We use these overrides to keep Mermaid@11.12.2 Sankey node geometry and padding in one place,
// so layout code does not duplicate the same diagram-specific literals.

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
