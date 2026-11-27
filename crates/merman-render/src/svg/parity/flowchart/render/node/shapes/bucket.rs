//! Flowchart v2 bucket shape.

use std::fmt::Write as _;

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt;

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_hand_drawn_svg_path;

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

pub(in crate::svg::parity::flowchart::render::node) fn render_bucket(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let (w, rim_ry, top_y, bottom_y, hand_drawn_body) = prepare_bucket_geometry(ctx, common, label);
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
    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(helpers::hand_drawn_group_style(common))
        );
    } else {
        out.push_str(r#"<g class="basic label-container">"#);
    }
    if common.look_is_hand_drawn() {
        if let Some((fill_d, stroke_d)) =
            helpers::timed_node_roughjs(common.timing, details, || {
                roughjs_paths_for_hand_drawn_svg_path(
                    &hand_drawn_body,
                    common.stroke_width,
                    common.stroke_dasharray,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            helpers::write_hand_drawn_path_pair(out, common, &hand_drawn_body, &fill_d, &stroke_d);
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body),
                escape_attr(&helpers::grouped_shape_child_style(common)),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" style="{}"/>"#,
            escape_attr(&body),
            escape_attr(common.style),
        );
    }

    let _ = write!(
        out,
        r#"<ellipse cx="0" cy="{}" rx="{}" ry="{}" style="fill:none;stroke:{};stroke-width:1px"/>"#,
        fmt(top_y),
        fmt(w / 2.0),
        fmt(rim_ry),
        escape_attr(ctx.node_border_color.as_str()),
    );
    out.push_str("</g>");

    let body_center_y = top_y + (bottom_y - top_y) / 2.0;
    label.dy = body_center_y;
}

fn prepare_bucket_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
) -> (f64, f64, f64, f64, String) {
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let p = ctx.node_padding.max(0.0);
    let w = (metrics.width + 2.0 * p)
        .max(common.layout_node.width.max(0.0))
        .max(80.0);
    let rim_ry = (w * 0.08).clamp(5.0, 12.0);
    let total_height = (metrics.height + 2.0 * p + rim_ry).max(common.layout_node.height.max(0.0));
    let top_y = -total_height / 2.0 + rim_ry;
    let bottom_y = total_height / 2.0;
    let bottom_w = w * 0.72;

    let hand_drawn_body = path_from_points(&bucket_points(w, top_y, bottom_y, bottom_w, rim_ry));

    (w, rim_ry, top_y, bottom_y, hand_drawn_body)
}

pub(super) fn advance_bucket_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
) {
    if !common.look_is_hand_drawn() {
        return;
    }
    let (_, _, _, _, hand_drawn_body) = prepare_bucket_geometry(ctx, common, label);
    let target = super::super::roughjs::RoughGeometryTarget::Query;
    let _ = helpers::prepare_node_path(common, &hand_drawn_body, target);
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
