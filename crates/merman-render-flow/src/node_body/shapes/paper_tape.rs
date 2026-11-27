//! Flowchart v2 paper tape (flag) shape.

use super::super::document_shapes::generate_full_sine_wave_points;
use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use merman_render_core::{
    number_format::canonical_number as fmt_display,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, roughjs_paths_for_svg_path},
    },
    svg_output::SvgOutput,
    text::TextMetrics,
};

pub(super) fn render_paper_tape(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
    neo: bool,
) {
    let (path_data,) = prepare_paper_tape_geometry(neo, metrics, padding);
    if let Some((fill_d, stroke_d)) = svg.hand_drawn_path_pair(&path_data) {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(svg.rough_group_style),
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(svg.fill_color),
            escape_attr(&stroke_d),
            escape_attr(svg.stroke_color),
            fmt_display(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
        );
        out.push_str("</g>");
    } else if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<path d="{}" class="basic label-container" style="{}"/>"#,
            escape_attr(&path_data),
            escape_attr(svg.style),
        );
    } else if let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
        roughjs_paths_for_svg_path(
            &path_data,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.work_meter,
            geometry.randomness,
        )
    }) {
        out.push_str(r#"<g class="basic label-container">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(svg.fill_color),
            escape_attr(svg.style),
            escape_attr(&stroke_d),
            escape_attr(svg.stroke_color),
            fmt_display(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
            escape_attr(svg.style),
        );
        out.push_str("</g>");
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" class="basic label-container" style="{}"/>"#,
            escape_attr(&path_data),
            escape_attr(svg.style),
        );
    }
}

fn prepare_paper_tape_geometry(neo: bool, metrics: TextMetrics, padding: f64) -> (String,) {
    let p = padding;
    let padding_x = if neo { 16.0 } else { p };
    let padding_y = if neo { 20.0 } else { p };
    let w = (metrics.width + 2.0 * padding_x).max(0.0);
    let h = (metrics.height + padding_y).max(0.0);
    let wave_amplitude = h / 8.0;
    let final_h = h + wave_amplitude * 2.0;

    let mut points: Vec<(f64, f64)> = Vec::new();
    points.push((-w / 2.0, final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        -w / 2.0,
        final_h / 2.0,
        w / 2.0,
        final_h / 2.0,
        wave_amplitude,
        1.0,
    ));
    points.push((w / 2.0, -final_h / 2.0));
    points.extend(generate_full_sine_wave_points(
        w / 2.0,
        -final_h / 2.0,
        -w / 2.0,
        -final_h / 2.0,
        wave_amplitude,
        -1.0,
    ));

    let path_data = path_from_points(&points);
    (path_data,)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
    neo: bool,
) {
    let (path_data,) = prepare_paper_tape_geometry(neo, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &path_data, target);
}
