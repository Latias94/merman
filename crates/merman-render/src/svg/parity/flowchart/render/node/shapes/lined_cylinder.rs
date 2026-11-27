//! Flowchart v2 lined cylinder (Disk storage).

use crate::svg::parity::flowchart::types::{FlowchartRenderCtx, FlowchartRenderDetails};
use crate::svg::parity::{SvgOutput, escape_attr, fmt};

use super::super::{FlowchartNodeLabelState, FlowchartNodeRenderCommon};
use super::cylinders::{CylinderGeometry, render_hand_drawn_vertical_cylinder};

pub(in crate::svg::parity::flowchart::render::node) fn render_lined_cylinder(
    out: &mut impl SvgOutput,
    ctx: &FlowchartRenderCtx<'_>,
    common: &FlowchartNodeRenderCommon<'_>,
    label: &mut FlowchartNodeLabelState<'_>,
    details: &mut FlowchartRenderDetails,
) {
    let geometry = CylinderGeometry::vertical(common, 0.0);
    let CylinderGeometry { w, h, rx, ry } = geometry;
    let outer_offset = h * 0.1;

    // Mermaid moves the label down by `ry`.
    label.dy = ry;
    if render_hand_drawn_vertical_cylinder(
        out,
        ctx,
        common,
        details,
        &geometry,
        Some(ry + outer_offset),
    ) {
        return;
    }

    // Mirror Mermaid `linedCylinder.ts` for classic and Neo.
    let path_data = format!(
        "M0,{ry} a{rx},{ry} 0,0,0 {w},0 a{rx},{ry} 0,0,0 -{w},0 l0,{h} a{rx},{ry} 0,0,0 {w},0 l0,-{h} M0,{y2} a{rx},{ry} 0,0,0 {w},0",
        ry = fmt(ry),
        rx = fmt(rx),
        w = fmt(w),
        h = fmt(h),
        y2 = fmt(ry + outer_offset),
    );
    let _ = write!(
        out,
        r#"<path d="{}" class="basic label-container outer-path" style="{}""#,
        escape_attr(&path_data),
        escape_attr(common.style),
    );
    if common.look_is_hand_drawn() {
        let _ = write!(
            out,
            r#" fill="{}" stroke="{}" stroke-width="{}""#,
            escape_attr(common.fill_color),
            escape_attr(common.stroke_color),
            crate::svg::parity::fmt_display(common.stroke_width as f64),
        );
    }
    if ctx.security_level_loose {
        let _ = write!(out, r#" label-offset-y="{}""#, fmt(ry));
    }
    let _ = write!(
        out,
        r#" transform="translate({},{})"/>"#,
        fmt(-w / 2.0),
        fmt(-(h / 2.0 + ry))
    );
}
