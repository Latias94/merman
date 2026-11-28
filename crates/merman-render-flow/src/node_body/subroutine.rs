//! Flowchart v2 subroutine shape.

use std::fmt::Write as _;

use merman_render_core::{
    number_format::canonical_number as fmt_display,
    rough_svg::{
        ops_to_svg_path_d,
        paths::{
            RoughGeometryTarget, RoughPathPair, prepare_hachure_rect,
            roughjs_prepare_hand_drawn_line_path,
        },
    },
    svg_output::SvgOutput,
};

use super::{NodeBodyGeometry, NodeBodySvg, OptionalStyleAttr, escape_attr};

const FRAME_WIDTH: f64 = 8.0;
const HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const HAND_DRAWN_HACHURE_GAP: f32 = 1.5;
const HAND_DRAWN_ROUGHNESS: f32 = 0.7;

pub fn write_subroutine(out: &mut impl SvgOutput, mut body: NodeBodySvg<'_>) {
    let total_w = body.geometry.width.max(1.0);
    let h = body.geometry.height.max(1.0);
    if body.geometry.hand_drawn
        && let Some(((fill, outline), side_strokes)) = body.timed_rough(|geometry| {
            let target = RoughGeometryTarget::Svg;
            let frame = geometry.write_rough_paths(prepare_frame(geometry, target, total_w, h)?)?;
            let sides = prepare_side_strokes(geometry, target, total_w, h)?;
            Some((frame, sides))
        })
    {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(body.hand_drawn_group_style()),
        );
        let frame = super::rectangle_backing(&body, total_w, h);
        body.write_hachure(out, &frame, &fill, f64::from(HAND_DRAWN_FILL_WEIGHT));
        write_outline(out, &body, &outline);
        if body.rough_fill_opacity.is_none() {
            out.push_str("</g>");
        }
        // Mermaid generates the left rule first, then inserts the right rule ahead of it.
        for stroke in side_strokes.iter().rev() {
            out.push_str("<g>");
            #[cfg(feature = "layout-shoal")]
            let d = if body.geometry.compact_rough {
                merman_render_core::rough_svg::ops_to_compact_svg_path_d(stroke)
            } else {
                ops_to_svg_path_d(stroke)
            };
            #[cfg(not(feature = "layout-shoal"))]
            let d = ops_to_svg_path_d(stroke);
            write_outline(out, &body, &d);
            out.push_str("</g>");
        }
        if body.rough_fill_opacity.is_some() {
            out.push_str("</g>");
        }
        return;
    }

    let w = (total_w - 2.0 * FRAME_WIDTH).max(1.0);

    let pts = [
        (0.0, 0.0),
        (w, 0.0),
        (w, -h),
        (0.0, -h),
        (0.0, 0.0),
        (-FRAME_WIDTH, 0.0),
        (w + FRAME_WIDTH, 0.0),
        (w + FRAME_WIDTH, -h),
        (-FRAME_WIDTH, -h),
        (-FRAME_WIDTH, 0.0),
    ];
    let mut points_attr = String::new();
    for (idx, (px, py)) in pts.iter().copied().enumerate() {
        if idx > 0 {
            points_attr.push(' ');
        }
        let _ = write!(&mut points_attr, "{},{}", fmt_display(px), fmt_display(py));
    }
    let _ = write!(
        out,
        r#"<polygon points="{}" class="label-container" transform="translate({},{})"{} />"#,
        points_attr,
        fmt_display(-w / 2.0),
        fmt_display(h / 2.0),
        OptionalStyleAttr(body.style)
    );
}

fn write_outline(out: &mut impl SvgOutput, body: &NodeBodySvg<'_>, path: &str) {
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
        escape_attr(path),
        escape_attr(body.stroke_color),
        body.geometry.stroke_width,
        escape_attr(body.geometry.stroke_dasharray),
    );
}

pub fn advance_subroutine(geometry: NodeBodyGeometry<'_>) {
    if geometry.hand_drawn {
        let width = geometry.width.max(1.0);
        let height = geometry.height.max(1.0);
        let target = RoughGeometryTarget::Query;
        if prepare_frame(&geometry, target, width, height).is_some() {
            let _ = prepare_side_strokes(&geometry, target, width, height);
        }
    }
}

fn prepare_frame(
    geometry: &NodeBodyGeometry<'_>,
    target: RoughGeometryTarget,
    width: f64,
    height: f64,
) -> Option<RoughPathPair> {
    prepare_hachure_rect(
        target,
        [-width / 2.0, -height / 2.0, width, height],
        geometry.stroke_width,
        geometry.stroke_dasharray,
        geometry.hachure(
            HAND_DRAWN_FILL_WEIGHT,
            HAND_DRAWN_HACHURE_GAP,
            HAND_DRAWN_ROUGHNESS,
        ),
        geometry.work_meter,
        geometry.randomness,
    )
}

fn prepare_side_strokes(
    geometry: &NodeBodyGeometry<'_>,
    target: RoughGeometryTarget,
    width: f64,
    height: f64,
) -> Option<[roughr::core::OpSet<f64>; 2]> {
    let inset = width / 2.0 - FRAME_WIDTH;
    let side = |x| {
        roughjs_prepare_hand_drawn_line_path(
            target,
            (x, -height / 2.0),
            (x, height / 2.0),
            geometry.stroke_width,
            geometry.stroke_dasharray,
            geometry.work_meter,
            geometry.randomness,
        )
    };
    Some([side(-inset)?, side(inset)?])
}
