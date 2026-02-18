//! Flowchart node renderer.

use super::super::*;

mod geom;
mod helpers;
mod label;
mod roughjs;
mod shapes;

pub(in crate::svg::parity::flowchart) fn render_flowchart_node(
    out: &mut String,
    ctx: &FlowchartRenderCtx<'_>,
    node_id: &str,
    origin_x: f64,
    origin_y: f64,
    timing_enabled: bool,
    details: &mut FlowchartRenderDetails,
) {
    let Some(layout_node) = ctx.layout_nodes_by_id.get(node_id) else {
        return;
    };

    let x = layout_node.x + ctx.tx - origin_x;
    let y = layout_node.y + ctx.ty - origin_y;

    if helpers::try_render_self_loop_label_placeholder(out, node_id, x, y) {
        return;
    }

    let Some(resolved) = helpers::resolve_node_render_info(ctx, node_id) else {
        return;
    };

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_enabled = !tooltip.trim().is_empty();

    let dom_idx = resolved.dom_idx;
    let class_attr_base = resolved.class_attr_base;
    let wrapped_in_a = resolved.wrapped_in_a;
    let href = resolved.href;
    let mut label_text: &str = if resolved.label_text_is_node_id {
        node_id
    } else {
        resolved.label_text
    };
    let mut label_type: &str = resolved.label_type;
    let shape: &str = resolved.shape;
    let node_icon = resolved.node_icon;
    let node_img = resolved.node_img;
    let node_pos = resolved.node_pos;
    let node_constraint = resolved.node_constraint;
    let node_asset_width = resolved.node_asset_width;
    let node_asset_height = resolved.node_asset_height;
    let node_styles = resolved.node_styles;
    let node_classes = resolved.node_classes;

    helpers::open_node_wrapper(
        out,
        node_id,
        dom_idx,
        class_attr_base,
        node_classes,
        wrapped_in_a,
        href,
        x,
        y,
        tooltip_enabled,
        tooltip,
    );

    let style_start = timing_enabled.then(std::time::Instant::now);
    let mut compiled_styles =
        flowchart_compile_styles(ctx.class_defs, node_classes, node_styles, &[]);
    if let Some(s) = style_start {
        details.node_style_compile += s.elapsed();
    }
    let style = std::mem::take(&mut compiled_styles.node_style);
    let mut label_dx: f64 = 0.0;
    let mut label_dy: f64 = 0.0;
    let mut compact_label_translate: bool = false;
    let fill_color = compiled_styles
        .fill
        .as_deref()
        .unwrap_or(ctx.node_fill_color.as_str());
    let stroke_color = compiled_styles
        .stroke
        .as_deref()
        .unwrap_or(ctx.node_border_color.as_str());
    let stroke_width: f32 = compiled_styles
        .stroke_width
        .as_deref()
        .and_then(|v| v.trim_end_matches("px").trim().parse::<f32>().ok())
        .unwrap_or(1.3);
    let stroke_dasharray = compiled_styles
        .stroke_dasharray
        .as_deref()
        .unwrap_or("0 0")
        .trim();

    let hand_drawn_seed = ctx
        .config
        .as_value()
        .get("handDrawnSeed")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);

    if shapes::try_render_flowchart_v2_no_label(
        out,
        ctx,
        shape,
        layout_node,
        fill_color,
        stroke_color,
        hand_drawn_seed,
        timing_enabled,
        details,
    ) {
        out.push_str("</g>");
        if wrapped_in_a {
            out.push_str("</a>");
        }
        return;
    }

    if shapes::render_flowchart_v2_shape(
        out,
        ctx,
        shape,
        layout_node,
        &mut label_text,
        &mut label_type,
        node_classes,
        node_styles,
        node_icon,
        node_img,
        node_pos,
        node_constraint,
        node_asset_width,
        node_asset_height,
        &style,
        fill_color,
        stroke_color,
        stroke_width,
        stroke_dasharray,
        hand_drawn_seed,
        wrapped_in_a,
        timing_enabled,
        details,
        &mut label_dx,
        &mut label_dy,
        &mut compact_label_translate,
    ) {
        return;
    }

    label::render_flowchart_node_label(
        out,
        ctx,
        layout_node,
        label_text,
        label_type,
        node_classes,
        node_styles,
        &compiled_styles,
        label_dx,
        label_dy,
        compact_label_translate,
        wrapped_in_a,
        timing_enabled,
        details,
    );
}
