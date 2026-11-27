//! Flowchart v2 datastore shape.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::paths::{
        RoughGeometryTarget, roughjs_hand_drawn_line_path, roughjs_paths_for_hand_drawn_rect,
        roughjs_prepare_hand_drawn_line_path, roughjs_prepare_paths_for_hand_drawn_rect,
    },
    svg_output::SvgOutput,
};

use super::{NodeBodyGeometry, NodeBodySvg, escape_attr};

pub fn write_datastore(out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
    let w = body.geometry.width.max(1.0);
    let h = body.geometry.height.max(1.0);

    if body.geometry.hand_drawn {
        let rough_paths = body.timed_rough(|geometry| {
            let (fill_d, _) = roughjs_paths_for_hand_drawn_rect(
                -w / 2.0,
                -h / 2.0,
                w,
                h,
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )?;
            let top_d = roughjs_hand_drawn_line_path(
                (-w / 2.0, -h / 2.0),
                (w / 2.0, -h / 2.0),
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )?;
            let bottom_d = roughjs_hand_drawn_line_path(
                (-w / 2.0, h / 2.0),
                (w / 2.0, h / 2.0),
                geometry.stroke_width,
                geometry.stroke_dasharray,
                geometry.work_meter,
                geometry.randomness,
            )?;
            Some((fill_d, top_d, bottom_d))
        });

        if let Some((fill_d, top_d, bottom_d)) = rough_paths {
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}"><path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
                escape_attr(body.rough_group_style),
                escape_attr(&fill_d),
                escape_attr(body.fill_color),
                escape_attr(&top_d),
                escape_attr(body.stroke_color),
                body.geometry.stroke_width,
                escape_attr(body.geometry.stroke_dasharray),
                escape_attr(&bottom_d),
                escape_attr(body.stroke_color),
                body.geometry.stroke_width,
                escape_attr(body.geometry.stroke_dasharray),
            );
            return;
        }
    }

    let _ = write!(
        out,
        r#"<rect class="basic label-container" style="{}" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}" stroke-dasharray="{} {}"/>"#,
        escape_attr(body.style),
        fmt(-w / 2.0),
        fmt(-h / 2.0),
        fmt(w),
        fmt(h),
        fmt(w),
        fmt(h)
    );
}

pub fn advance_datastore(geometry: NodeBodyGeometry<'_>) {
    if !geometry.hand_drawn {
        return;
    }
    let w = geometry.width.max(1.0);
    let h = geometry.height.max(1.0);
    let Some(_) = roughjs_prepare_paths_for_hand_drawn_rect(
        RoughGeometryTarget::Query,
        -w / 2.0,
        -h / 2.0,
        w,
        h,
        geometry.stroke_width,
        geometry.stroke_dasharray,
        geometry.work_meter,
        geometry.randomness,
    ) else {
        return;
    };
    let Some(_) = roughjs_prepare_hand_drawn_line_path(
        RoughGeometryTarget::Query,
        (-w / 2.0, -h / 2.0),
        (w / 2.0, -h / 2.0),
        geometry.stroke_width,
        geometry.stroke_dasharray,
        geometry.work_meter,
        geometry.randomness,
    ) else {
        return;
    };
    let _ = roughjs_prepare_hand_drawn_line_path(
        RoughGeometryTarget::Query,
        (-w / 2.0, h / 2.0),
        (w / 2.0, h / 2.0),
        geometry.stroke_width,
        geometry.stroke_dasharray,
        geometry.work_meter,
        geometry.randomness,
    );
}
