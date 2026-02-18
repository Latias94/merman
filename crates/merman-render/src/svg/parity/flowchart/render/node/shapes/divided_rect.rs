//! Flowchart v2 divided rectangle (Divided process).

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;

use super::super::roughjs::roughjs_paths_for_polygon;

pub(in crate::svg::parity::flowchart::render::node) fn render_divided_rect(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    fill_color: &str,
    stroke_color: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
    label_dy: &mut f64,
) {
    fn rough_timed<T>(
        timing_enabled: bool,
        details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
        f: impl FnOnce() -> T,
    ) -> T {
        if timing_enabled {
            details.node_roughjs_calls += 1;
            let start = std::time::Instant::now();
            let out = f();
            details.node_roughjs += start.elapsed();
            out
        } else {
            f()
        }
    }

    // Mermaid draws the polygon using `h` and then the rendered bbox expands to
    // `out_h = h + rectOffset` where `rectOffset = h * 0.2`, i.e. `out_h = 1.2*h`.
    let out_w = layout_node.width.max(1.0);
    let out_h = layout_node.height.max(1.0);
    let h = out_h / 1.2;
    let w = out_w;
    let rect_offset = h * 0.2;
    let x = -w / 2.0;
    let y = -h / 2.0 - rect_offset / 2.0;

    // Label is shifted down by `rectOffset/2`.
    *label_dy = rect_offset / 2.0;

    let pts: Vec<(f64, f64)> = vec![
        (x, y + rect_offset),
        (-x, y + rect_offset),
        (-x, -y),
        (x, -y),
        (x, y),
        (-x, y),
        (-x, y + rect_offset),
    ];
    let (fill_d, stroke_d) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_polygon(&pts, fill_color, stroke_color, 1.3, hand_drawn_seed)
    })
    .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
    let _ = write!(
        out,
        r##"<g class="basic label-container" style=""><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        escape_attr(&fill_d),
        escape_attr(fill_color),
        escape_attr(&stroke_d),
        escape_attr(stroke_color),
    );
}
