// This file is intentionally small and hand-curated.
//
// We use these overrides to close the remaining C4 SVG text-measurement parity gaps where
// Mermaid@11.12.2 rounds browser `getBBox().height` to diagram-specific per-line constants.

pub fn lookup_c4_svg_bbox_line_height_px(font_size_px: i64) -> Option<f64> {
    match font_size_px {
        12 => Some(14.0),
        14 => Some(16.0),
        16 => Some(17.0),
        _ => None,
    }
}
