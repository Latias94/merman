//! Flowchart v2 curved trapezoid (Display).

use crate::svg::parity::flowchart::escape_attr;
use crate::svg::parity::util;

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_curved_trapezoid(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );

    let geometry = crate::flowchart::DisplayGeometry::from_label(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        crate::config::mermaid_config_diagram_look(ctx.config).is_neo(),
    );
    let path_data = path_from_points(&geometry.points);
    if let Some((fill_d, stroke_d)) = helpers::hand_drawn_path_pair(common, details, &path_data) {
        let _ = write!(
            out,
            r##"<g class="basic label-container outer-path" transform="translate({},{})" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"##,
            util::fmt(geometry.tx),
            util::fmt(geometry.ty),
            escape_attr(common.rough_group_style),
            escape_attr(&fill_d),
            escape_attr(common.fill_color),
            escape_attr(&stroke_d),
            escape_attr(common.stroke_color),
            util::fmt_display(common.stroke_width as f64),
            escape_attr(common.stroke_dasharray),
        );
        return;
    }
    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate({}, {})">"#,
            util::fmt(geometry.tx),
            util::fmt(geometry.ty),
        );
        helpers::write_raw_filled_stroked_path(
            out,
            &path_data,
            common.fill_color,
            common.stroke_color,
            common.stroke_width,
            common.style,
        );
        out.push_str("</g>");
        return;
    }

    let (fill_d, stroke_d) =
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &path_data,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
        .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
    let _ = write!(
        out,
        r##"<g class="basic label-container outer-path" transform="translate({},{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
        util::fmt(geometry.tx),
        util::fmt(geometry.ty),
        escape_attr(&fill_d),
        escape_attr(common.fill_color),
        escape_attr(common.style),
        escape_attr(&stroke_d),
        escape_attr(common.stroke_color),
        util::fmt_display(common.stroke_width as f64),
        escape_attr(common.stroke_dasharray),
        escape_attr(common.style),
    );
}
