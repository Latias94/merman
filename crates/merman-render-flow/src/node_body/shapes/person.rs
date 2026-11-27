//! Flowchart v2 person shape.

use super::super::{NodeBodyGeometry, NodeBodySvg, circle_backing, escape_attr, prepare_node_path};
use super::rounded_rect_points;
use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, roughjs_paths_for_circle, roughjs_prepare_paths_for_circle},
    },
    svg_output::SvgOutput,
    text::TextMetrics,
};

pub(super) fn render_person(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> f64 {
    let (w, head_radius, body_height, body_radius, body_top, body_path, head_center_y) =
        prepare_person_geometry(&svg.geometry, metrics, padding);
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
        if let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
            roughjs_paths_for_circle(
                head_radius * 2.0,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                true,
                geometry.work_meter,
                geometry.randomness,
            )
        }) {
            let _ = write!(
                out,
                r#"<g transform="translate(0,{})">"#,
                fmt(head_center_y),
            );
            let outline = circle_backing(&svg, head_radius);
            svg.write_hand_drawn_path_pair(out, &outline, &fill_d, &stroke_d, 4.0);
            out.push_str("</g>");
        } else {
            let _ = write!(
                out,
                r#"<circle cx="0" cy="{}" r="{}" style="{}"/>"#,
                fmt(head_center_y),
                fmt(head_radius),
                escape_attr(&svg.grouped_child_style()),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<rect x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}" style="{}"/><circle cx="0" cy="{}" r="{}" style="{}"/>"#,
            fmt(-w / 2.0),
            fmt(body_top),
            fmt(w),
            fmt(body_height),
            fmt(body_radius),
            fmt(body_radius),
            escape_attr(svg.style),
            fmt(head_center_y),
            fmt(head_radius),
            escape_attr(svg.style),
        );
    }
    out.push_str("</g>");

    body_top + body_height / 2.0
}

fn prepare_person_geometry(
    geometry: &NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> (f64, f64, f64, f64, f64, String, f64) {
    let p = padding.max(0.0);
    let w = (metrics.width + 2.0 * p)
        .max(geometry.width.max(0.0))
        .max(100.0);
    let head_radius = (w * 0.23).clamp(16.0, 56.0);
    let overlap = head_radius * 0.27;
    let body_height =
        (metrics.height + 2.0 * p).max((geometry.height - (2.0 * head_radius - overlap)).max(0.0));
    let body_radius = (w * 0.177).min(body_height * 0.45);
    let total_height = body_height + 2.0 * head_radius - overlap;
    let top = -total_height / 2.0;
    let body_top = top + 2.0 * head_radius - overlap;
    let body_path = path_from_points(&rounded_rect_points(
        -w / 2.0,
        body_top,
        w,
        body_height,
        body_radius,
    ));
    let head_center_y = top + head_radius;

    (
        w,
        head_radius,
        body_height,
        body_radius,
        body_top,
        body_path,
        head_center_y,
    )
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) {
    let (_, head_radius, _, _, _, body_path, _) =
        prepare_person_geometry(&geometry, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &body_path, target);
    let _ = roughjs_prepare_paths_for_circle(
        target,
        head_radius * 2.0,
        geometry.stroke_width,
        geometry.stroke_dasharray,
        true,
        geometry.work_meter,
        geometry.randomness,
    );
}
