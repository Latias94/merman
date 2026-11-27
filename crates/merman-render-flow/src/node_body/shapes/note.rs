//! Flowchart v2 note shape.

use merman_render_core::{
    number_format::canonical_number as fmt,
    rough_svg::{
        RoughRectSpec, paths::RoughGeometryTarget, roughjs_paths_for_rect, roughjs_rect_operations,
    },
    svg_output::SvgOutput,
};

use super::super::{NodeBodyGeometry, NodeBodySvg, escape_attr, prepare_node_path};

pub(super) fn write(
    out: &mut impl SvgOutput,
    mut svg: NodeBodySvg<'_>,
    note_fill: &str,
    note_stroke: &str,
) {
    let (w, h, x, y) = note_geometry(&svg.geometry);

    if let Some((fill_d, stroke_d)) =
        svg.hand_drawn_path_pair(&format!("M{} {} H{} V{} H{} Z", x, y, x + w, y + h, x))
    {
        let _ = write!(
            out,
            r#"<g class="basic label-container outer-path" style="{}">"#,
            escape_attr(svg.rough_group_style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="4" fill="none" stroke-dasharray="0 0"/><path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/></g>"#,
            escape_attr(&fill_d),
            escape_attr(note_fill),
            escape_attr(&stroke_d),
            escape_attr(note_stroke),
            fmt(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
        );
    } else if svg.geometry.hand_drawn {
        let _ = write!(
            out,
            r#"<rect class="basic label-container outer-path" style="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}" stroke-width="{}"/>"#,
            escape_attr(svg.style),
            fmt(x),
            fmt(y),
            fmt(w),
            fmt(h),
            escape_attr(note_fill),
            escape_attr(note_stroke),
            fmt(svg.geometry.stroke_width as f64),
        );
    } else if let Some((fill_d, stroke_d)) = svg.timed_rough(|geometry| {
        roughjs_paths_for_rect(RoughRectSpec {
            x,
            y,
            w,
            h,
            stroke_width: geometry.stroke_width,
            work_meter: geometry.work_meter,
            phase: merman_model::OperationPhase::Emit,
            randomness: geometry.randomness,
        })
    }) {
        let _ = write!(out, r#"<g class="basic label-container outer-path">"#);
        let _ = write!(
            out,
            r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
            escape_attr(&fill_d),
            escape_attr(note_fill),
            escape_attr(svg.style)
        );
        let _ = write!(
            out,
            r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
            escape_attr(&stroke_d),
            escape_attr(note_stroke),
            fmt(svg.geometry.stroke_width as f64),
            escape_attr(svg.geometry.stroke_dasharray),
            escape_attr(svg.style)
        );
        out.push_str("</g>");
    } else {
        // Fallback: basic rect.
        let _ = write!(
            out,
            r#"<rect class="basic label-container outer-path" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
            escape_attr(svg.style),
            fmt(x),
            fmt(y),
            fmt(w),
            fmt(h)
        );
    }
}

fn note_geometry(geometry: &NodeBodyGeometry<'_>) -> (f64, f64, f64, f64) {
    let w = geometry.width.max(1.0);
    let h = geometry.height.max(1.0);
    let x = -w / 2.0;
    let y = -h / 2.0;

    (w, h, x, y)
}

pub(super) fn advance_rough_geometry(geometry: NodeBodyGeometry<'_>) {
    let (w, h, x, y) = note_geometry(&geometry);
    if geometry.hand_drawn {
        let path = format!("M{} {} H{} V{} H{} Z", x, y, x + w, y + h, x);
        let _ = prepare_node_path(&geometry, &path, RoughGeometryTarget::Query);
    } else {
        let _ = roughjs_rect_operations(RoughRectSpec {
            x,
            y,
            w,
            h,
            stroke_width: geometry.stroke_width,
            work_meter: geometry.work_meter,
            phase: merman_model::OperationPhase::Layout,
            randomness: geometry.randomness,
        });
    }
}
