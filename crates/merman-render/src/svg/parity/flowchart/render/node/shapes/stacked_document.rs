//! Flowchart v2 stacked document shape.

use crate::svg::parity::{escape_xml_display, fmt_display};

use super::super::geom::{generate_full_sine_wave_points, path_from_points};
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
        Some(common.layout_node),
        label.text,
        label.label_type,
        common.node_classes,
        common.node_styles,
    );

    let p = ctx.node_padding;
    let w = metrics.width + 2.0 * p;
    let h = metrics.height + 3.0 * p;
    let wave_amplitude = h / 8.0;
    let final_h = h + wave_amplitude / 2.0;
    let x = -w / 2.0;
    let y = -final_h / 2.0;
    let rect_offset = 10.0;

    label.dx = -rect_offset;
    label.dy = rect_offset - wave_amplitude / 2.0;

    let wave_points = generate_full_sine_wave_points(
        x - rect_offset,
        y + final_h + rect_offset,
        x + w - rect_offset,
        y + final_h + rect_offset,
        wave_amplitude,
        0.8,
    );
    let (_last_x, last_y) = wave_points[wave_points.len() - 1];

    let mut outer_points: Vec<(f64, f64)> = Vec::new();
    outer_points.push((x - rect_offset, y + rect_offset));
    outer_points.push((x - rect_offset, y + final_h + rect_offset));
    outer_points.extend(wave_points.iter().copied());
    outer_points.push((x + w - rect_offset, last_y - rect_offset));
    outer_points.push((x + w, last_y - rect_offset));
    outer_points.push((x + w, last_y - 2.0 * rect_offset));
    outer_points.push((x + w + rect_offset, last_y - 2.0 * rect_offset));
    outer_points.push((x + w + rect_offset, y - rect_offset));
    outer_points.push((x + rect_offset, y - rect_offset));
    outer_points.push((x + rect_offset, y));
    outer_points.push((x, y));
    outer_points.push((x, y + rect_offset));

    let inner_points = vec![
        (x, y + rect_offset),
        (x + w - rect_offset, y + rect_offset),
        (x + w - rect_offset, last_y - rect_offset),
        (x + w, last_y - rect_offset),
        (x + w, y),
        (x, y),
    ];

    let outer_path = path_from_points(&outer_points);
    let inner_path = path_from_points(&inner_points);

    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate(0,{})" style="{}">"#,
            fmt_display(-wave_amplitude / 2.0),
            escape_xml_display(common.rough_group_style),
        );
    } else {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" transform="translate(0,{})">"#,
            fmt_display(-wave_amplitude / 2.0)
        );
    }
    let outer_paths = if common.look_is_hand_drawn() {
        helpers::hand_drawn_path_pair(common, details, &outer_path)
    } else {
        super::super::helpers::timed_node_roughjs(common.timing, details, || {
            roughjs_paths_for_svg_path(
                &outer_path,
                common.fill_color,
                common.stroke_color,
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
                common.fill_color,
                common.stroke_color,
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
