//! Font-agnostic SVG text bbox helpers.

use super::TextStyle;

const SVG_DEFAULT_FIRST_LINE_BBOX_EM: f64 = 1.1875;
const SVG_EDGE_LABEL_BASELINE_BBOX_EM: f64 = 1.125;
const SVG_DEFAULT_TITLE_ASCENT_EM: f64 = 0.9444444444;
const SVG_DEFAULT_TITLE_DESCENT_EM: f64 = 0.262;

pub(crate) fn svg_bbox_round_px_ties_to_even(v: f64) -> f64 {
    if !v.is_finite() {
        return 0.0;
    }
    let floor = v.floor();
    let frac = v - floor;
    if frac < 0.5 {
        floor
    } else if frac > 0.5 {
        floor + 1.0
    } else if (floor as i64) % 2 == 0 {
        floor
    } else {
        floor + 1.0
    }
}

pub(crate) fn svg_wrapped_first_line_bbox_height_px(style: &TextStyle) -> f64 {
    svg_bbox_round_px_ties_to_even(style.font_size.max(1.0) * SVG_DEFAULT_FIRST_LINE_BBOX_EM)
}

pub(crate) fn flowchart_svg_edge_label_background_y_px(style: &TextStyle) -> f64 {
    let baseline_box_h =
        svg_bbox_round_px_ties_to_even(style.font_size.max(1.0) * SVG_EDGE_LABEL_BASELINE_BBOX_EM);
    baseline_box_h - svg_wrapped_first_line_bbox_height_px(style)
}

pub(crate) fn svg_title_bbox_vertical_extents_px(style: &TextStyle) -> (f64, f64) {
    let font_size = style.font_size.max(1.0);
    (
        font_size * SVG_DEFAULT_TITLE_ASCENT_EM,
        font_size * SVG_DEFAULT_TITLE_DESCENT_EM,
    )
}
