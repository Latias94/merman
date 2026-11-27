//! Mermaid 11.17 folder/directory flowchart shape.

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use merman_render_core::{
    number_format::canonical_number as fmt_display,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{RoughGeometryTarget, roughjs_paths_for_hand_drawn_svg_path},
    },
    svg_output::SvgOutput,
    text::TextMetrics,
};

fn folder_geometry(label_width: f64, label_height: f64, padding: f64) -> (f64, f64, f64) {
    let padding = padding.max(0.0);
    let width = (label_width + 2.0 * padding).max(90.0);
    let content_height = label_height + 2.0 * padding;
    let tab_height = (content_height * 0.16).clamp(8.0, 14.0);
    let total_height = content_height + tab_height;
    (width, total_height, tab_height)
}

fn folder_points(width: f64, total_height: f64, tab_height: f64) -> Vec<(f64, f64)> {
    let tab_width = (width * 0.38).max(28.0);
    let top = -total_height / 2.0;
    vec![
        (-width / 2.0, top),
        (-width / 2.0 + tab_width, top),
        (-width / 2.0 + tab_width, top + tab_height),
        (width / 2.0, top + tab_height),
        (width / 2.0, total_height / 2.0),
        (-width / 2.0, total_height / 2.0),
    ]
}

pub(super) fn render_folder(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> f64 {
    let (total_height, tab_height, path_data) =
        prepare_folder_geometry(&svg.geometry, metrics, padding);
    if svg.geometry.hand_drawn {
        let rough_paths = svg.timed_rough(|geometry| {
            roughjs_paths_for_hand_drawn_svg_path(
                &path_data,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )
        });
        if let Some((fill_d, stroke_d)) = rough_paths {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr(svg.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(svg.fill_color),
                escape_attr(&stroke_d),
                escape_attr(svg.stroke_color),
                fmt_display(svg.geometry.stroke_width as f64),
                escape_attr(svg.geometry.stroke_dasharray),
            );
        } else {
            let _ = write!(
                out,
                r#"<path d="{}" class="basic label-container" style="{}"/>"#,
                escape_attr(&path_data),
                escape_attr(svg.style),
            );
        }
    } else {
        let _ = write!(
            out,
            r#"<path d="{}" class="basic label-container" style="{}"/>"#,
            escape_attr(&path_data),
            escape_attr(svg.style),
        );
    }

    let body_height = total_height - tab_height.min(total_height);
    -total_height / 2.0 + tab_height.min(total_height) + body_height / 2.0
}

fn prepare_folder_geometry(
    geometry: &NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) -> (f64, f64, String) {
    let (measured_width, measured_height, tab_height) =
        folder_geometry(metrics.width, metrics.height, padding);
    // Mermaid takes the larger of the rendered label box and any pre-existing node dimensions.
    let width = measured_width.max(geometry.width.max(0.0));
    let total_height = measured_height.max(geometry.height.max(0.0));
    let points = folder_points(width, total_height, tab_height.min(total_height));
    let path_data = path_from_points(&points);

    (total_height, tab_height, path_data)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
) {
    let (_, _, path_data) = prepare_folder_geometry(&geometry, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &path_data, target);
}
