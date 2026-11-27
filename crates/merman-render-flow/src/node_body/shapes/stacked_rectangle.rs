//! Flowchart v2 stacked rectangle shape.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::closed_path_d_from_points as path_from_points,
    rough_svg::paths::{RoughGeometryTarget, roughjs_paths_for_svg_path},
    svg_output::SvgOutput,
};

use super::super::{
    NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, escape_attr, prepare_node_path,
};

fn write_stacked_rectangle_path_group(
    out: &mut impl SvgOutput,
    svg: &NodeBodySvg<'_>,
    fill_d: &str,
    stroke_d: &str,
    hand_drawn: bool,
) {
    if hand_drawn {
        let _ = write!(
            out,
            r#"<g><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
            escape_attr(fill_d),
            escape_attr(svg.fill_color),
            escape_attr(stroke_d),
            escape_attr(svg.stroke_color),
            fmt(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
        );
        return;
    }

    let _ = write!(
        out,
        r#"<g><path d="{} {}" fill="{}" fill-opacity="1" stroke="{}" stroke-opacity="1" stroke-width="{}"{} /></g>"#,
        escape_attr(fill_d),
        escape_attr(stroke_d),
        escape_attr(svg.fill_color),
        escape_attr(svg.stroke_color),
        fmt(svg.geometry.stroke_width as f64),
        OptionalStyleAttr(svg.style),
    );
}

pub(super) fn write(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    neo: bool,
    label_dx: &mut f64,
    label_dy: &mut f64,
) {
    let (outer_path, inner_path) =
        prepare_stacked_rectangle_geometry(&svg.geometry, neo, label_dx, label_dy);
    if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" style="{}">"#,
            escape_attr(svg.rough_group_style),
        );
    } else {
        out.push_str(r#"<g class="basic label-container outer-path">"#);
    }
    for path_data in [&outer_path, &inner_path] {
        let hand_drawn = svg.geometry.hand_drawn;
        let paths = if hand_drawn {
            svg.hand_drawn_path_pair(path_data)
        } else {
            svg.timed_rough(|geometry| {
                roughjs_paths_for_svg_path(
                    path_data,
                    geometry.stroke_width,
                    geometry.stroke_dasharray,
                    geometry.work_meter,
                    geometry.randomness,
                )
            })
        };

        if let Some((fill_d, stroke_d)) = paths {
            write_stacked_rectangle_path_group(out, &svg, &fill_d, &stroke_d, hand_drawn);
        } else if hand_drawn {
            write_stacked_rectangle_path_group(out, &svg, path_data, path_data, false);
        }
    }
    out.push_str("</g>");
}

fn prepare_stacked_rectangle_geometry(
    geometry: &NodeBodyGeometry<'_>,
    neo: bool,
    label_dx: &mut f64,
    label_dy: &mut f64,
) -> (String, String) {
    let rect_offset = if neo { 10.0 } else { 5.0 };
    let total_w = geometry.width.max(1.0);
    let total_h = geometry.height.max(1.0);
    let w = (total_w - 2.0 * rect_offset).max(1.0);
    let h = (total_h - 2.0 * rect_offset).max(1.0);
    let x = -w / 2.0;
    let y = -h / 2.0;

    *label_dx = -rect_offset;
    *label_dy = rect_offset;

    let outer_points = vec![
        (x - rect_offset, y + rect_offset),
        (x - rect_offset, y + h + rect_offset),
        (x + w - rect_offset, y + h + rect_offset),
        (x + w - rect_offset, y + h),
        (x + w, y + h),
        (x + w, y + h - rect_offset),
        (x + w + rect_offset, y + h - rect_offset),
        (x + w + rect_offset, y - rect_offset),
        (x + rect_offset, y - rect_offset),
        (x + rect_offset, y),
        (x, y),
        (x, y + rect_offset),
    ];

    let inner_points = vec![
        (x, y + rect_offset),
        (x + w - rect_offset, y + rect_offset),
        (x + w - rect_offset, y + h),
        (x + w, y + h),
        (x + w, y),
        (x, y),
    ];

    let outer_path = path_from_points(&outer_points);
    let inner_path = path_from_points(&inner_points);

    (outer_path, inner_path)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    neo: bool,
    label_dx: &mut f64,
    label_dy: &mut f64,
) {
    let (outer_path, inner_path) =
        prepare_stacked_rectangle_geometry(&geometry, neo, label_dx, label_dy);
    let target = RoughGeometryTarget::Query;
    for path in [&outer_path, &inner_path] {
        let _ = prepare_node_path(&geometry, path, target);
    }
}
