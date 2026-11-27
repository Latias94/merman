//! Flowchart v2 window pane shape.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::{RoughGeometryTarget, roughjs_paths_for_svg_path},
    svg_output::SvgOutput,
    xml::escape_attr,
};

use super::super::{
    NodeBodyGeometry, NodeBodySvg, prepare_node_path, write_raw_filled_stroked_path,
};

pub(super) fn write(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    label_dx: &mut f64,
    label_dy: &mut f64,
) {
    let (rect_offset, path_data) = prepare_window_pane_geometry(&svg.geometry, label_dx, label_dy);
    if let Some((fill_d, stroke_d)) = svg.hand_drawn_path_pair(&path_data) {
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})" class="basic label-container outer-path" style="{}">"#,
            fmt(rect_offset / 2.0),
            fmt(rect_offset / 2.0),
            escape_attr(svg.hand_drawn_group_style()),
        );
        svg.write_hand_drawn_path_pair(out, &path_data, &fill_d, &stroke_d, 4.0);
        out.push_str("</g>");
        return;
    }

    if let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
        roughjs_paths_for_svg_path(
            &path_data,
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.work_meter,
            geometry.randomness,
        )
    }) {
        let _ = write!(
            out,
            r#"<g transform="translate({},{})" class="basic label-container outer-path">"#,
            fmt(rect_offset / 2.0),
            fmt(rect_offset / 2.0)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(svg.fill_color),
            escape_attr(svg.style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(svg.stroke_color),
            fmt(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
            escape_attr(svg.style)
        );
        out.push_str("</g>");
    } else if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})" class="basic label-container outer-path">"#,
            fmt(rect_offset / 2.0),
            fmt(rect_offset / 2.0),
        );
        write_raw_filled_stroked_path(out, &path_data, &svg);
        out.push_str("</g>");
    }
}

fn prepare_window_pane_geometry(
    geometry: &NodeBodyGeometry<'_>,
    label_dx: &mut f64,
    label_dy: &mut f64,
) -> (f64, String) {
    // Mermaid `windowPane.ts` (non-handDrawn): RoughJS multi-subpath with `roughness=0` + a
    // fixed `rectOffset=10` and a translation of `(+5, +5)`.
    let rect_offset = 10.0;
    let out_w = geometry.width.max(1.0);
    let out_h = geometry.height.max(1.0);
    let w = (out_w - rect_offset).max(1.0);
    let h = (out_h - rect_offset).max(1.0);
    let x = -w / 2.0;
    let y = -h / 2.0;

    // Label transform includes the same `rectOffset/2` shift as the container.
    *label_dx = rect_offset / 2.0;
    *label_dy = rect_offset / 2.0;

    let path_data = format!(
        "M{},{} L{},{} L{},{} L{},{} L{},{} M{},{} L{},{} M{},{} L{},{}",
        fmt(x - rect_offset),
        fmt(y - rect_offset),
        fmt(x + w),
        fmt(y - rect_offset),
        fmt(x + w),
        fmt(y + h),
        fmt(x - rect_offset),
        fmt(y + h),
        fmt(x - rect_offset),
        fmt(y - rect_offset),
        fmt(x - rect_offset),
        fmt(y),
        fmt(x + w),
        fmt(y),
        fmt(x),
        fmt(y - rect_offset),
        fmt(x),
        fmt(y + h),
    );

    (rect_offset, path_data)
}

pub(super) fn advance_rough_geometry(
    geometry: NodeBodyGeometry<'_>,
    label_dx: &mut f64,
    label_dy: &mut f64,
) {
    let (_, path_data) = prepare_window_pane_geometry(&geometry, label_dx, label_dy);
    let target = RoughGeometryTarget::Query;
    let _ = prepare_node_path(&geometry, &path_data, target);
}
