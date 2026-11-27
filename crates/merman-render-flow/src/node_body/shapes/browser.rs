//! Flowchart v2 browser shape.

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use super::rounded_rect_points;
use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{closed_path_d_from_points as path_from_points, paths::RoughGeometryTarget},
    svg_output::SvgOutput,
    text::TextMetrics,
};

pub(super) fn render_browser(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
    accent: &str,
) -> f64 {
    let (bar_height, radius, w, h, top, body_path) =
        prepare_browser_geometry(&svg.geometry, metrics, padding);
    if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(svg.hand_drawn_group_style())
        );
    } else {
        out.push_str(r#"<g class="basic label-container">"#);
    }
    if svg.geometry.hand_drawn {
        if let Some((fill_d, stroke_d)) = svg.hand_drawn_path_pair(&body_path) {
            svg.write_hand_drawn_path_pair(out, &body_path, &fill_d, &stroke_d, 4.0);
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body_path),
                escape_attr(&svg.grouped_child_style()),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" style="{}"/>"#,
            fmt(-w / 2.0),
            fmt(top),
            fmt(w),
            fmt(h),
            fmt(radius),
            fmt(radius),
            escape_attr(svg.style),
        );
    }

    let _ = write!(
        out,
        r#"<line x1="{}" y1="{}" x2="{}" y2="{}" style="stroke:{};stroke-width:1px"/>"#,
        fmt(-w / 2.0),
        fmt(top + bar_height),
        fmt(w / 2.0),
        fmt(top + bar_height),
        escape_attr(accent),
    );
    for i in 0..3 {
        let _ = write!(
            out,
            r#"<circle cx="{}" cy="{}" r="2.5" style="fill:{};stroke:none"/>"#,
            fmt(-w / 2.0 + 12.0 + i as f64 * 9.0),
            fmt(top + bar_height / 2.0),
            escape_attr(accent),
        );
    }
    let _ = write!(
        out,
        r#"<rect class="browser-address-bar" x="{}" y="{}" width="{}" height="{}" rx="3" ry="3" style="fill:none;stroke:{};stroke-width:1px;opacity:0.6"/>"#,
        fmt(-w / 2.0 + 44.0),
        fmt(top + 4.0),
        fmt((w - 56.0).max(10.0)),
        fmt(bar_height - 8.0),
        escape_attr(accent),
    );
    out.push_str("</g>");

    top + bar_height + (h - bar_height) / 2.0
}

fn prepare_browser_geometry(
    geometry: &NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> (f64, f64, f64, f64, f64, String) {
    let p = padding.max(0.0);
    let bar_height = 18.0;
    let radius = 12.0;
    let w = (metrics.width + 2.0 * p)
        .max(geometry.width.max(0.0))
        .max(90.0);
    let h = (metrics.height + 2.0 * p + bar_height).max(geometry.height.max(0.0));
    let top = -h / 2.0;
    let body_path = path_from_points(&rounded_rect_points(-w / 2.0, top, w, h, radius));

    (bar_height, radius, w, h, top, body_path)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) {
    let (_, _, _, _, _, body_path) = prepare_browser_geometry(&geometry, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &body_path, target);
}
