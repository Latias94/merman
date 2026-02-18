//! Flowchart v2 cylinder shapes.

use std::fmt::Write as _;

use crate::svg::parity::{escape_attr, fmt};

pub(in crate::svg::parity::flowchart::render::node) fn render_cylinder(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    layout_node: &crate::model::LayoutNode,
    style: &str,
    label_dy: &mut f64,
) {
    // Mermaid `cylinder.ts` (non-handDrawn): a single `<path>` with arc commands and a
    // `label-offset-y` attribute.
    let w = layout_node.width.max(1.0);
    let rx = w / 2.0;
    let ry = rx / (2.5 + w / 50.0);
    let total_h = layout_node.height.max(1.0);
    let h = (total_h - 2.0 * ry).max(1.0);
    // Mermaid applies an extra downward label shift of `node.padding / 1.5`.
    *label_dy = ctx.node_padding / 1.5;

    let path_data = format!(
        "M0,{ry} a{rx},{ry} 0,0,0 {w},0 a{rx},{ry} 0,0,0 {mw},0 l0,{h} a{rx},{ry} 0,0,0 {w},0 l0,{mh}",
        ry = fmt(ry),
        rx = fmt(rx),
        w = fmt(w),
        mw = fmt(-w),
        h = fmt(h),
        mh = fmt(-h),
    );

    let _ = write!(
        out,
        r#"<path d="{}" class="basic label-container" style="{}" transform="translate({}, {})"/>"#,
        escape_attr(&path_data),
        escape_attr(style),
        fmt(-w / 2.0),
        fmt(-(h / 2.0 + ry))
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_horizontal_cylinder(
    out: &mut String,
    layout_node: &crate::model::LayoutNode,
    style: &str,
    label_dx: &mut f64,
) {
    // Mermaid `tiltedCylinder.ts` (non-handDrawn): a single `<path>` with arc commands.
    //
    // Mermaid first computes the *inner* path width `w` from the label bbox, then calls
    // `updateNodeBounds(...)` which inflates the Dagre node bounds to include arc extents.
    // Our `layout_node.width` is the inflated width, so we reconstruct the inner segment
    // width by subtracting `2*rx`.
    let out_w = layout_node.width.max(1.0);
    let h = layout_node.height.max(1.0);
    let ry = h / 2.0;
    let rx = if ry == 0.0 {
        0.0
    } else {
        ry / (2.5 + h / 50.0)
    };
    let w = (out_w - 2.0 * rx).max(1.0);

    // Mermaid offsets the label left by `rx` for tilted cylinders.
    *label_dx = -rx;

    let path_data = format!(
        "M0,0 a{rx},{ry} 0,0,1 0,{neg_h} l{w},0 a{rx},{ry} 0,0,1 0,{h} M{w},{neg_h} a{rx},{ry} 0,0,0 0,{h} l{neg_w},0",
        rx = fmt(rx),
        ry = fmt(ry),
        neg_h = fmt(-h),
        w = fmt(w),
        h = fmt(h),
        neg_w = fmt(-w),
    );

    let _ = write!(
        out,
        r#"<path d="{}" class="basic label-container" style="{}" transform="translate({}, {} )"/>"#,
        escape_attr(&path_data),
        escape_attr(style),
        fmt(-w / 2.0),
        fmt(h / 2.0),
    );
}
