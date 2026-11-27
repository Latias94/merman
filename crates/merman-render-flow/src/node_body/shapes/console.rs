//! Flowchart v2 console shape.

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use super::rounded_rect_points;
use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{closed_path_d_from_points as path_from_points, paths::RoughGeometryTarget},
    svg_output::SvgOutput,
    text::TextMetrics,
    xml::escape_xml_display,
};

pub(super) fn render_console(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
    accent: &str,
) -> f64 {
    let (glyph_band, radius, w, h, top, body_path) =
        prepare_console_geometry(&svg.geometry, metrics, padding);
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
        r#"<text x="{}" y="{}" class="console-glyph" style="font-family:monospace;font-weight:bold;font-size:14px;fill:{}">{}</text>"#,
        fmt(-w / 2.0 + 12.0),
        fmt(top + 16.0),
        escape_attr(accent),
        escape_xml_display(">_"),
    );
    out.push_str("</g>");

    top + glyph_band + (h - glyph_band) / 2.0
}

fn prepare_console_geometry(
    geometry: &NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> (f64, f64, f64, f64, f64, String) {
    let p = padding.max(0.0);
    let glyph_band = 20.0;
    let radius = 12.0;
    let w = (metrics.width + 2.0 * p)
        .max(geometry.width.max(0.0))
        .max(90.0);
    let h = (metrics.height + 2.0 * p + glyph_band).max(geometry.height.max(0.0));
    let top = -h / 2.0;
    let body_path = path_from_points(&rounded_rect_points(-w / 2.0, top, w, h, radius));

    (glyph_band, radius, w, h, top, body_path)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) {
    let (_, _, _, _, _, body_path) = prepare_console_geometry(&geometry, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &body_path, target);
}
