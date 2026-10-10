//! Flowchart v2 stacked document shape.

use crate::svg::parity::{escape_xml_display, fmt_display};

use super::super::geom::path_from_points;
use super::super::helpers;
use super::super::roughjs::roughjs_paths_for_svg_path;

pub(in crate::svg::parity::flowchart::render::node) fn render_stacked_document(
    out: &mut impl crate::svg::parity::SvgOutput,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    label: &mut super::super::FlowchartNodeLabelState<'_>,
    details: &mut crate::svg::parity::flowchart::types::FlowchartRenderDetails,
) {
    let metrics = helpers::compute_node_label_metrics(
        ctx,
        common.node_id,
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );

    let geometry = crate::flowchart::flowchart_stacked_document_geometry(
        metrics.width,
        metrics.height,
        ctx.node_padding,
        common.look_is_neo(),
    );
    label.dx = geometry.label_dx;
    label.dy = geometry.label_dy;
    let outer_path = path_from_points(&geometry.outer_points);
    let inner_path = path_from_points(&geometry.inner_points);

    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate(0,{})" style="{}">"#,
            fmt_display(geometry.group_dy),
            escape_xml_display(common.rough_group_style),
        );
    } else {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate(0,{})">"#,
            fmt_display(geometry.group_dy)
        );
    }
    let outer_paths = if common.look_is_hand_drawn() {
        helpers::hand_drawn_path_pair(common, details, &outer_path)
    } else {
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &outer_path,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
    };
    if let Some((fill_d, stroke_d)) = outer_paths {
        if common.look_is_hand_drawn() {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(common.fill_color),
                escape_xml_display(&stroke_d),
                escape_xml_display(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_xml_display(common.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(common.fill_color),
                escape_xml_display(common.style),
                escape_xml_display(&stroke_d),
                escape_xml_display(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_xml_display(common.stroke_dasharray),
                escape_xml_display(common.style),
            );
        }
    } else if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/>"#,
            escape_xml_display(&outer_path),
            escape_xml_display(common.fill_color),
            escape_xml_display(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_xml_display(common.style),
        );
    }
    out.push_str("<g>");
    let inner_paths = if common.look_is_hand_drawn() {
        helpers::hand_drawn_path_pair(common, details, &inner_path)
    } else {
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &inner_path,
                common.stroke_width,
                common.stroke_dasharray,
                common.hand_drawn_seed,
            )
        })
    };
    if let Some((fill_d, stroke_d)) = inner_paths {
        if common.look_is_hand_drawn() {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(common.fill_color),
                escape_xml_display(&stroke_d),
                escape_xml_display(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_xml_display(common.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                escape_xml_display(&fill_d),
                escape_xml_display(common.fill_color),
                escape_xml_display(common.style),
                escape_xml_display(&stroke_d),
                escape_xml_display(common.stroke_color),
                fmt_display(common.stroke_width as f64),
                escape_xml_display(common.stroke_dasharray),
                escape_xml_display(common.style),
            );
        }
    } else if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/>"#,
            escape_xml_display(&inner_path),
            escape_xml_display(common.fill_color),
            escape_xml_display(common.stroke_color),
            fmt_display(common.stroke_width as f64),
            escape_xml_display(common.style),
        );
    }
    out.push_str("</g></g>");
}
