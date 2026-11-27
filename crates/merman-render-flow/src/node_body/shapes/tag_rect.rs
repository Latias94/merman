//! Flowchart v2 tagged rectangle (Tagged process).

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};
use merman_render_core::{
    number_format::canonical_number as fmt_display,
    rough_svg::{
        closed_path_d_from_points as path_from_points,
        paths::{
            RoughGeometryTarget, roughjs_paths_for_hand_drawn_solid_svg_path,
            roughjs_paths_for_svg_path, roughjs_prepare_paths_for_hand_drawn_solid_svg_path,
        },
    },
    svg_output::SvgOutput,
    text::TextMetrics,
};

pub(super) fn render_tag_rect(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    metrics: TextMetrics,
    padding: f64,
    neo: bool,
) {
    let (rect_path, tag_path) = prepare_tag_rect_geometry(neo, metrics, padding);
    if svg.geometry.hand_drawn {
        let rect_paths = svg.hand_drawn_path_pair(&rect_path);
        let tag_paths = svg
            .timed_rough(|geometry| {
                roughjs_paths_for_hand_drawn_solid_svg_path(
                    &tag_path,
                    geometry.stroke_width,
                    geometry.stroke_dasharray,
                    geometry.work_meter,
                    geometry.randomness,
                )
            })
            .filter(|(fill_d, stroke_d)| !fill_d.is_empty() && !stroke_d.is_empty());

        if let (Some((rect_fill_d, rect_stroke_d)), Some((tag_fill_d, tag_stroke_d))) =
            (rect_paths, tag_paths)
        {
            let _ = write!(
                out,
                r##"<g class="basic label-container outer-path" style="{}"><g><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g><path d="{}" stroke="none" stroke-width="0" fill="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"##,
                escape_attr(svg.rough_group_style),
                escape_attr(&rect_fill_d),
                escape_attr(svg.fill_color),
                escape_attr(&rect_stroke_d),
                escape_attr(svg.stroke_color),
                fmt_display(svg.geometry.stroke_width as f64),
                escape_attr(svg.geometry.stroke_dasharray),
                escape_attr(&tag_fill_d),
                escape_attr(svg.fill_color),
                escape_attr(&tag_stroke_d),
                escape_attr(svg.stroke_color),
                fmt_display(svg.geometry.stroke_width as f64),
                escape_attr(svg.geometry.stroke_dasharray),
            );
            return;
        }

        // Hand-drawn RoughJS only accepts a strict color subset. Keep the full tag and body
        // geometry visible when either path cannot be admitted.
        let _ = write!(
            out,
            r##"<g class="basic label-container outer-path"><g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g><path d="{}" fill="{}" stroke="{}" stroke-width="{}" style="{}"/></g>"##,
            escape_attr(&rect_path),
            escape_attr(svg.fill_color),
            escape_attr(svg.stroke_color),
            fmt_display(svg.geometry.stroke_width as f64),
            escape_attr(svg.style),
            escape_attr(&tag_path),
            escape_attr(svg.fill_color),
            escape_attr(svg.stroke_color),
            fmt_display(svg.geometry.stroke_width as f64),
            escape_attr(svg.style),
        );
        return;
    }

    let (rect_fill_d, rect_stroke_d) = svg
        .timed_rough(|geometry| {
            roughjs_paths_for_svg_path(
                &rect_path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )
        })
        .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

    let (tag_fill_d, tag_stroke_d) = svg
        .timed_rough(|geometry| {
            roughjs_paths_for_svg_path(
                &tag_path,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )
        })
        .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

    let _ = write!(
        out,
        r##"<g class="basic label-container outer-path"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/></g>"##,
        escape_attr(&rect_fill_d),
        escape_attr(svg.fill_color),
        escape_attr(svg.style),
        escape_attr(&rect_stroke_d),
        escape_attr(svg.stroke_color),
        fmt_display(svg.geometry.stroke_width as f64),
        escape_attr(svg.geometry.stroke_dasharray),
        escape_attr(svg.style),
        escape_attr(&tag_fill_d),
        escape_attr(svg.fill_color),
        escape_attr(svg.style),
        escape_attr(&tag_stroke_d),
        escape_attr(svg.stroke_color),
        fmt_display(svg.geometry.stroke_width as f64),
        escape_attr(svg.geometry.stroke_dasharray),
        escape_attr(svg.style),
    );
}

fn prepare_tag_rect_geometry(neo: bool, metrics: TextMetrics, padding: f64) -> (String, String) {
    let p = padding;
    let padding_x = if neo { 16.0 } else { p };
    let padding_y = if neo { 12.0 } else { p };
    let w = metrics.width + 2.0 * padding_x;
    let h = metrics.height + 2.0 * padding_y;
    let x = -w / 2.0;
    let y = -h / 2.0;
    let tag_w = 0.2 * h;
    let tag_h = 0.2 * h;

    let rect_points = vec![
        (x - tag_w / 2.0, y),
        (x + w + tag_w / 2.0, y),
        (x + w + tag_w / 2.0, y + h),
        (x - tag_w / 2.0, y + h),
    ];
    let tag_points = vec![
        (x + w - tag_w / 2.0, y + h),
        (x + w + tag_w / 2.0, y + h),
        (x + w + tag_w / 2.0, y + h - tag_h),
    ];

    let rect_path = path_from_points(&rect_points);
    let tag_path = path_from_points(&tag_points);

    (rect_path, tag_path)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    metrics: TextMetrics,
    padding: f64,
    neo: bool,
) {
    let (rect_path, tag_path) = prepare_tag_rect_geometry(neo, metrics, padding);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &rect_path, target);
    if geometry.hand_drawn {
        let _ = roughjs_prepare_paths_for_hand_drawn_solid_svg_path(
            target,
            &tag_path,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.work_meter,
            geometry.randomness,
        );
    } else {
        let _ = prepare_node_path(&geometry, &tag_path, target);
    }
}
