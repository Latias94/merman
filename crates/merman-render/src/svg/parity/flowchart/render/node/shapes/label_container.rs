//! Flowchart v2 shapes that emit a label container and continue into label rendering.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::path_from_points;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_hourglass_collate(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    fill_color: &str,
    stroke_color: &str,
    hand_drawn_seed: u64,
    timing_enabled: bool,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
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

    let w = layout_node.width.max(30.0);
    let h = layout_node.height.max(30.0);
    let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)];
    let path_data = path_from_points(&pts);
    let (fill_d, stroke_d) = rough_timed(timing_enabled, details, || {
        roughjs_paths_for_svg_path(
            &path_data,
            fill_color,
            stroke_color,
            1.3,
            "0 0",
            hand_drawn_seed,
        )
    })
    .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
    let _ = write!(
        out,
        r##"<g class="basic label-container" transform="translate({}, {})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
        util::fmt(-w / 2.0),
        util::fmt(-h / 2.0),
        escape_attr(&fill_d),
        escape_attr(fill_color),
        escape_attr(&stroke_d),
        escape_attr(stroke_color),
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_notched_rectangle(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
) {
    let w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let notch = 12.0;
    let pts: Vec<(f64, f64)> = vec![
        (notch, -h),
        (w, -h),
        (w, 0.0),
        (0.0, 0.0),
        (0.0, -h + notch),
        (notch, -h),
    ];
    let mut points_attr = String::new();
    for (idx, (px, py)) in pts.iter().copied().enumerate() {
        if idx > 0 {
            points_attr.push(' ');
        }
        let _ = write!(&mut points_attr, "{},{}", util::fmt(px), util::fmt(py));
    }
    let _ = write!(
        out,
        r#"<polygon points="{}" class="label-container" transform="translate({},{})"/>"#,
        points_attr,
        util::fmt(-w / 2.0),
        util::fmt(h / 2.0)
    );
}
