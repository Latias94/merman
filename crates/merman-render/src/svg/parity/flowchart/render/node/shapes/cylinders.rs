//! Flowchart v2 cylinder shapes.

use std::fmt::Write as _;

use crate::flowchart::flowchart_effective_text_style_for_node_classes;
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
        r#"<path d="{}" class="basic label-container" style="{}" transform="translate({},{})"/>"#,
        escape_attr(&path_data),
        escape_attr(style),
        fmt(-w / 2.0),
        fmt(-(h / 2.0 + ry))
    );
}

pub(in crate::svg::parity::flowchart::render::node) fn render_horizontal_cylinder(
    out: &mut String,
    ctx: &crate::svg::parity::flowchart::types::FlowchartRenderCtx<'_>,
    _layout_node: &crate::model::LayoutNode,
    label_text: &str,
    label_type: &str,
    node_classes: &[String],
    node_styles: &[String],
    style: &str,
    label_dx: &mut f64,
    label_dy: &mut f64,
) {
    // Mermaid `tiltedCylinder.ts` (non-handDrawn): a single `<path>` with arc commands.
    //
    // Mermaid computes the rendered path from the label bbox, then separately calls
    // `updateNodeBounds(...)` and feeds that DOM bbox into Dagre. Those two sizes can diverge
    // slightly, so rebuild the path from label metrics instead of `layout_node.width`.
    let metrics = super::super::helpers::compute_node_label_metrics(
        ctx,
        label_text,
        label_type,
        node_classes,
        node_styles,
    );
    let label_padding = ctx.node_padding / 2.0;
    let h = (metrics.height + label_padding).max(1.0);
    let ry = h / 2.0;
    let rx = if ry == 0.0_f64 {
        0.0
    } else {
        ry / (2.5 + h / 50.0)
    };
    let w = (metrics.width + rx + label_padding).max(1.0);

    // Mermaid offsets the label left by `rx` for tilted cylinders.
    *label_dx = -rx;
    if !ctx.node_html_labels {
        let node_text_style = flowchart_effective_text_style_for_node_classes(
            &ctx.text_style,
            ctx.class_defs,
            node_classes,
            node_styles,
        );
        *label_dy -= crate::text::svg_create_text_bbox_y_offset_px(&node_text_style);
    }

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
        r#"<path d="{}" class="basic label-container" style="{}" transform="translate({},{})"/>"#,
        escape_attr(&path_data),
        escape_attr(style),
        fmt(-w / 2.0),
        fmt(h / 2.0),
    );
}
