//! Flowchart v2 bucket shape.

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, roughjs_paths_for_hand_drawn_svg_path},
    },
    svg_output::SvgOutput,
    text::TextMetrics,
};
use std::fmt::Write as _;

fn bucket_points(w: f64, top_y: f64, bottom_y: f64, bottom_w: f64, rim_ry: f64) -> Vec<(f64, f64)> {
    const ARC_SEGMENTS: usize = 12;

    let mut points = vec![(-w / 2.0, top_y), (-bottom_w / 2.0, bottom_y)];
    for step in 1..=ARC_SEGMENTS {
        // The SVG arc travels from the left rim to the right rim with sweep=0. In the
        // y-down coordinate system this is the lower half of the ellipse, so sample the
        // angle from PI down to zero to keep x monotonic and avoid a self-folding polygon.
        let angle = std::f64::consts::PI - std::f64::consts::PI * step as f64 / ARC_SEGMENTS as f64;
        points.push((
            bottom_w / 2.0 * angle.cos(),
            bottom_y + rim_ry * angle.sin(),
        ));
    }
    points.push((w / 2.0, top_y));
    for step in 1..=ARC_SEGMENTS {
        let angle = std::f64::consts::PI * step as f64 / ARC_SEGMENTS as f64;
        points.push((w / 2.0 * angle.cos(), top_y - rim_ry * angle.sin()));
    }
    points
}
pub(super) fn render_bucket(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
    accent: &str,
) -> f64 {
    let (w, rim_ry, top_y, bottom_y, hand_drawn_body) =
        prepare_bucket_geometry(&svg.geometry, metrics, padding);
    let bottom_w = w * 0.72;
    let mut body = String::new();
    let _ = write!(
        &mut body,
        "M{},{} L{},{} A {} {} 0 0 0 {},{} L{},{} A {} {} 0 0 0 {},{} Z",
        fmt(-w / 2.0),
        fmt(top_y),
        fmt(-bottom_w / 2.0),
        fmt(bottom_y),
        fmt(bottom_w / 2.0),
        fmt(rim_ry),
        fmt(bottom_w / 2.0),
        fmt(bottom_y),
        fmt(w / 2.0),
        fmt(top_y),
        fmt(w / 2.0),
        fmt(rim_ry),
        fmt(-w / 2.0),
        fmt(top_y),
    );
    if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(svg.rough_group_style)
        );
    } else {
        out.push_str(r#"<g class="basic label-container">"#);
    }
    if svg.geometry.hand_drawn {
        if let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
            roughjs_paths_for_hand_drawn_svg_path(
                &hand_drawn_body,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )
        }) {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_attr(&fill_d),
                escape_attr(svg.fill_color),
                escape_attr(&stroke_d),
                escape_attr(svg.stroke_color),
                fmt(svg.geometry.stroke_width as f64),
                escape_attr(svg.geometry.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body),
                escape_attr(svg.style),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" style="{}"/>"#,
            escape_attr(&body),
            escape_attr(svg.style),
        );
    }

    let _ = write!(
        out,
        r#"<ellipse cx="0" cy="{}" rx="{}" ry="{}" style="fill:none;stroke:{};stroke-width:1px"/>"#,
        fmt(top_y),
        fmt(w / 2.0),
        fmt(rim_ry),
        escape_attr(accent),
    );
    out.push_str("</g>");

    top_y + (bottom_y - top_y) / 2.0
}

fn prepare_bucket_geometry(
    geometry: &NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> (f64, f64, f64, f64, String) {
    let p = padding.max(0.0);
    let w = (metrics.width + 2.0 * p)
        .max(geometry.width.max(0.0))
        .max(80.0);
    let rim_ry = (w * 0.08).clamp(5.0, 12.0);
    let total_height = (metrics.height + 2.0 * p + rim_ry).max(geometry.height.max(0.0));
    let top_y = -total_height / 2.0 + rim_ry;
    let bottom_y = total_height / 2.0;
    let bottom_w = w * 0.72;

    let hand_drawn_body = path_from_points(&bucket_points(w, top_y, bottom_y, bottom_w, rim_ry));

    (w, rim_ry, top_y, bottom_y, hand_drawn_body)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) {
    let (_, _, _, _, hand_drawn_body) = prepare_bucket_geometry(&geometry, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &hand_drawn_body, target);
}

#[cfg(test)]
mod tests {
    use super::bucket_points;

    #[test]
    fn hand_drawn_bucket_points_follow_both_rims_without_backtracking() {
        let points = bucket_points(100.0, -30.0, 30.0, 72.0, 8.0);

        assert_eq!(points.len(), 27);
        assert_eq!(points[0], (-50.0, -30.0));
        assert_eq!(points[1], (-36.0, 30.0));
        assert_eq!(points[13], (36.0, 30.0));
        assert_eq!(points[14], (50.0, -30.0));
        assert_eq!(points[26], (-50.0, -30.0));

        assert!(points[1..=13].windows(2).all(|pair| pair[0].0 <= pair[1].0));
        assert!(
            points[14..=26]
                .windows(2)
                .all(|pair| pair[0].0 >= pair[1].0)
        );
    }
}
