//! Flowchart v2 console shape.

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::{escape_xml_display, fmt, fmt_display};

use super::super::geom::{path_from_points, rounded_rect_points};
use super::super::roughjs::roughjs_paths_for_hand_drawn_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_console(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = super::super::helpers::compute_node_label_metrics(
        ctx,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );
    let p = ctx.node_padding.max(0.0);
    let glyph_band = 20.0;
    let radius = 12.0;
    let w = (metrics.width + 2.0 * p)
        .max(common.layout_node.width.max(0.0))
        .max(90.0);
    let h = (metrics.height + 2.0 * p + glyph_band).max(common.layout_node.height.max(0.0));
    let top = -h / 2.0;
    let body_path = path_from_points(&rounded_rect_points(-w / 2.0, top, w, h, radius));

    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(common.rough_group_style)
        );
    } else {
        out.push_str(r#"<g class="basic label-container">"#);
    }
    if common.look_is_hand_drawn() {
        if let Some((fill_d, stroke_d)) =
            super::super::helpers::timed_node_roughjs(common.timing, details, || {
                roughjs_paths_for_hand_drawn_svg_path(
                    &body_path,
                    common.fill_color,
                    common.stroke_color,
                    common.stroke_width,
                    common.stroke_dasharray,
                    common.work_meter,
                    common.hand_drawn_seed,
                )
            })
        {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_attr(&fill_d),
                escape_attr(common.fill_color),
                escape_attr(&stroke_d),
                escape_attr(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_attr(common.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" style="{}"/>"#,
                escape_attr(&body_path),
                escape_attr(common.style),
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
        r#"<text x="{}" y="{}" class="console-glyph" style="font-family:monospace;font-weight:bold;font-size:14px;fill:{}">{}</text>"#,
        fmt(-w / 2.0 + 12.0),
        fmt(top + 16.0),
        escape_attr(ctx.node_border_color.as_str()),
        escape_xml_display(">_"),
    );
    out.push_str("</g>");

    label.dy = top + glyph_band + (h - glyph_band) / 2.0;
}
