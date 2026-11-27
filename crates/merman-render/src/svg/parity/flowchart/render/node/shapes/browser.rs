//! Flowchart v2 browser shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::fmt;

use super::super::geom::{path_from_points, rounded_rect_points};
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_hand_drawn_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_browser(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let (bar_height, radius, w, h, top, body_path) = prepare_browser_geometry(ctx, common, label);
    let accent = ctx.node_border_color.as_str();
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
                    &body_path,
                    common.stroke_width,
                    common.stroke_dasharray,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            helpers::write_hand_drawn_path_pair(out, common, &body_path, &fill_d, &stroke_d);
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body_path),
                escape_attr(&helpers::grouped_shape_child_style(common)),
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
            escape_attr(common.style),
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

    label.dy = top + bar_height + (h - bar_height) / 2.0;
}

fn prepare_browser_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
) -> (f64, f64, f64, f64, f64, String) {
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
    let bar_height = 18.0;
    let radius = 12.0;
    let w = (metrics.width + 2.0 * p)
        .max(common.layout_node.width.max(0.0))
        .max(90.0);
    let h = (metrics.height + 2.0 * p + bar_height).max(common.layout_node.height.max(0.0));
    let top = -h / 2.0;
    let body_path = path_from_points(&rounded_rect_points(-w / 2.0, top, w, h, radius));

    (bar_height, radius, w, h, top, body_path)
}

pub(super) fn advance_browser_rough_geometry(
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeGeometry<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
) {
    if !common.look_is_hand_drawn() {
        return;
    }
    let (_, _, _, _, _, body_path) = prepare_browser_geometry(ctx, common, label);
    let target = super::super::roughjs::RoughGeometryTarget::Query;
    let _ = helpers::prepare_node_path(common, &body_path, target);
}
