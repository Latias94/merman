//! Flowchart v2 subroutine shape.

use std::fmt::Write as _;

use super::super::FlowchartNodeGeometry;
use super::super::helpers::timed_node_roughjs;
use super::super::roughjs::{
    RoughGeometryTarget, RoughPathPair, roughjs_prepare_hachure_paths_for_rect,
    roughjs_prepare_hand_drawn_line_path,
};
use crate::svg::parity::flowchart::types::FlowchartRenderDetails;
use crate::svg::parity::flowchart::{OptionalStyleAttr, escape_attr};
use crate::svg::parity::fmt_display;
use crate::svg::parity::roughjs_common::ops_to_svg_path_d;

const FRAME_WIDTH: f64 = 8.0;
const HAND_DRAWN_FILL_WEIGHT: f32 = 1.5;
const HAND_DRAWN_HACHURE_GAP: f32 = 1.5;
const HAND_DRAWN_ROUGHNESS: f32 = 0.7;

pub(in crate::svg::parity::flowchart::render::node) fn render_subroutine(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let total_w = common.layout_node.width.max(1.0);
    let h = common.layout_node.height.max(1.0);
    if common.look_is_hand_drawn()
        && let Some(((fill, outline), side_strokes)) =
            timed_node_roughjs(common.timing, details, || {
                let target = RoughGeometryTarget::Svg;
                let frame =
                    prepare_frame(common, target, total_w, h)?.into_svg_paths(common.work_meter)?;
                let sides = prepare_side_strokes(common, target, total_w, h)?;
                Some((frame, sides))
            })
    {
        let _ = write!(
            out,
            r#"<g class="basic label-container" style="{}">"#,
            escape_attr(common.rough_group_style),
        );
        let frame = if common.solid_rough_fill {
            use kurbo::Shape;
            kurbo::Rect::from_center_size((0.0, 0.0), (total_w, h))
                .to_path(0.05)
                .to_svg()
        } else {
            String::new()
        };
        super::super::helpers::write_hand_drawn_hachure(
            out,
            common,
            &frame,
            &fill,
            f64::from(HAND_DRAWN_FILL_WEIGHT),
        );
        write_outline(out, common, &outline);
        out.push_str("</g>");
        // Mermaid generates the left rule first, then inserts the right rule ahead of it.
        for stroke in side_strokes.iter().rev() {
            out.push_str("<g>");
            write_outline(out, common, &ops_to_svg_path_d(stroke));
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
        OptionalStyleAttr(common.style)
    );
}

fn write_outline(
    out: &mut impl crate::svg::parity::SvgOutput,
    common: &super::super::FlowchartNodeRenderCommon<'_>,
    path: &str,
) {
    let _ = write!(
        out,
        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
        escape_attr(path),
        escape_attr(common.stroke_color),
        common.stroke_width,
        escape_attr(common.stroke_dasharray),
    );
}

pub(super) fn advance_rough_geometry(common: &FlowchartNodeGeometry<'_>) {
    if common.look_is_hand_drawn() {
        let width = common.layout_node.width.max(1.0);
        let height = common.layout_node.height.max(1.0);
        let target = RoughGeometryTarget::Query;
        if prepare_frame(common, target, width, height).is_some() {
            let _ = prepare_side_strokes(common, target, width, height);
        }
    }
}

fn prepare_frame(
    common: &FlowchartNodeGeometry<'_>,
    target: RoughGeometryTarget,
    width: f64,
    height: f64,
) -> Option<RoughPathPair> {
    roughjs_prepare_hachure_paths_for_rect(
        target,
        -width / 2.0,
        -height / 2.0,
        width,
        height,
        common.stroke_width,
        common.stroke_dasharray,
        HAND_DRAWN_FILL_WEIGHT,
        HAND_DRAWN_HACHURE_GAP,
        HAND_DRAWN_ROUGHNESS,
        common.work_meter,
        common.hand_drawn_seed,
    )
}

fn prepare_side_strokes(
    common: &FlowchartNodeGeometry<'_>,
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
            common.stroke_width,
            common.stroke_dasharray,
            common.work_meter,
            common.hand_drawn_seed,
        )
    };
    Some([side(-inset)?, side(inset)?])
}
