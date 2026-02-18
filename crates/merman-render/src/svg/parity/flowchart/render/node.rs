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

    enum RenderNodeKind<'a> {
        Normal(&'a crate::flowchart::FlowNode),
        EmptySubgraph(&'a crate::flowchart::FlowSubgraph),
    }

    let node_kind = if let Some(node) = ctx.nodes_by_id.get(node_id) {
        RenderNodeKind::Normal(node)
    } else if let Some(sg) = ctx.subgraphs_by_id.get(node_id) {
        if sg.nodes.is_empty() {
            RenderNodeKind::EmptySubgraph(sg)
        } else {
            return;
        }
    } else {
        return;
    };

    let tooltip = ctx.tooltips.get(node_id).map(|s| s.as_str()).unwrap_or("");
    let tooltip_enabled = !tooltip.trim().is_empty();

    let dom_idx: Option<usize>;
    let class_attr_base: &str;
    let wrapped_in_a: bool;
    let href: Option<&str>;
    let mut label_text: &str;
    let mut label_type: &str;
    let shape: &str;
    let node_icon: Option<&str>;
    let node_img: Option<&str>;
    let node_pos: Option<&str>;
    let node_constraint: Option<&str>;
    let node_asset_width: Option<f64>;
    let node_asset_height: Option<f64>;
    let node_styles: &[String];
    let node_classes: &[String];

    match node_kind {
        RenderNodeKind::Normal(node) => {
            dom_idx = Some(ctx.node_dom_index.get(node_id).copied().unwrap_or(0));
            shape = node.layout_shape.as_deref().unwrap_or("squareRect");

            // Mermaid flowchart-v2 uses a distinct wrapper class for icon/image nodes.
            class_attr_base = if shape == "imageSquare" {
                "image-shape default"
            } else if shape == "icon" || shape.starts_with("icon") {
                "icon-shape default"
            } else {
                "node default"
            };

            let link = node
                .link
                .as_deref()
                .map(|u| u.trim())
                .filter(|u| !u.is_empty());
            let link_present = link.is_some();
            // Mermaid sanitizes unsafe URLs (e.g. `javascript:` in strict mode) into
            // `about:blank`, but the resulting SVG `<a>` carries no `xlink:href` attribute.
            href = link
                .filter(|u| *u != "about:blank")
                .filter(|u| helpers::href_is_safe_in_strict_mode(u, ctx.config));
            // Mermaid wraps nodes in `<a>` only when a link is present. Callback-based
            // interactions (`click A someFn`) still mark the node as clickable, but do not
            // emit an anchor element in the SVG.
            wrapped_in_a = link_present;

            label_text = node.label.as_deref().unwrap_or(node_id);
            label_type = node.label_type.as_deref().unwrap_or("text");
            node_icon = node.icon.as_deref();
            node_img = node.img.as_deref();
            node_pos = node.pos.as_deref();
            node_constraint = node.constraint.as_deref();
            node_asset_width = node.asset_width;
            node_asset_height = node.asset_height;
            node_styles = &node.styles;
            node_classes = &node.classes;
        }
        RenderNodeKind::EmptySubgraph(sg) => {
            dom_idx = None;
            shape = "squareRect";
            wrapped_in_a = false;
            href = None;
            class_attr_base = "node";
            label_text = sg.title.as_str();
            label_type = sg.label_type.as_deref().unwrap_or("text");
            node_icon = None;
            node_img = None;
            node_pos = None;
            node_constraint = None;
            node_asset_width = None;
            node_asset_height = None;
            node_styles = &[];
            node_classes = &sg.classes;
        }
    }

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

    match shape {
        // Flowchart v2 shapes with no label group are handled above.

        // Flowchart v2 hourglass/collate: Mermaid clears `node.label` but still emits an empty
        // label group (via `labelHelper(...)`).
        "hourglass" | "collate" => {
            label_text = "";
            label_type = "text";
            shapes::render_hourglass_collate(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 card/notched-rectangle.
        "notch-rect" | "notched-rectangle" | "card" => {
            shapes::render_notched_rectangle(out, layout_node);
        }

        // Flowchart v2 delay / half-rounded rectangle.
        "delay" => {
            shapes::render_delay(
                out,
                ctx,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 lined cylinder (Disk storage).
        "lin-cyl" => {
            shapes::render_lined_cylinder(out, layout_node, &mut label_dy);
        }

        // Flowchart v2 curved trapezoid (Display).
        "curv-trap" => {
            shapes::render_curved_trapezoid(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 divided rectangle (Divided process).
        "div-rect" => {
            shapes::render_divided_rect(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dy,
            );
        }

        // Flowchart v2 notched pentagon (Loop limit).
        "notch-pent" => {
            shapes::render_notched_pentagon(
                out,
                ctx,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 bow tie rectangle (Stored data).
        "bow-rect" => {
            shapes::render_bow_tie_rect(
                out,
                ctx,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 tagged rectangle (Tagged process).
        "tag-rect" => {
            shapes::render_tag_rect(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 wave edged rectangle (Document).
        "doc" => {
            compact_label_translate = true;
            shapes::render_wave_document(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dx,
                &mut label_dy,
            );
        }

        // Flowchart v2 lined wave edged rectangle (Lined document).
        "lin-doc" => {
            compact_label_translate = true;
            shapes::render_lined_wave_document(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dx,
                &mut label_dy,
            );
        }

        // Flowchart v2 tagged wave edged rectangle (Tagged document).
        "tag-doc" => {
            compact_label_translate = true;
            shapes::render_tagged_wave_document(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dx,
                &mut label_dy,
            );
        }

        // Flowchart v2 triangle (Extract).
        "tri" => {
            shapes::render_triangle_extract(
                out,
                ctx,
                label_text,
                label_type,
                node_classes,
                node_styles,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dy,
            );
        }

        // Flowchart v2 shaded process / lined rectangle.
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" => {
            label_dx = 4.0;
            compact_label_translate = true;
            shapes::render_shaded_process(
                out,
                layout_node,
                fill_color,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }

        // Flowchart v2 curly brace/comment shapes (rendering-elements).
        "comment" | "brace" | "brace-l" | "brace-r" | "braces" => {
            shapes::render_curly_brace_comment(
                out,
                shape,
                layout_node,
                stroke_color,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut compact_label_translate,
                &mut label_dx,
            );
        }

        "imageSquare" => {
            if shapes::try_render_image_square(
                out,
                ctx,
                layout_node,
                label_text,
                label_type,
                node_pos,
                node_img,
                node_asset_height,
                node_asset_width,
                node_constraint,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                wrapped_in_a,
                timing_enabled,
                details,
            ) {
                return;
            }
        }
        "iconSquare" => {
            if shapes::try_render_icon_square(
                out,
                ctx,
                label_text,
                label_type,
                node_icon,
                node_pos,
                node_asset_width,
                node_asset_height,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                wrapped_in_a,
            ) {
                return;
            }
        }
        "manual-file" | "flipped-triangle" | "flip-tri" => {
            shapes::render_manual_file(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "manual-input" | "sloped-rectangle" | "sl-rect" => {
            shapes::render_manual_input(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            shapes::render_stacked_document(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "procs" | "processes" | "st-rect" | "stacked-rectangle" => {
            shapes::render_stacked_rectangle(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "paper-tape" | "flag" => {
            shapes::render_paper_tape(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "subroutine" | "fr-rect" | "subproc" | "subprocess" => {
            shapes::render_subroutine(out, layout_node, style.as_str());
        }
        "cylinder" | "cyl" => {
            shapes::render_cylinder(out, ctx, layout_node, &style, &mut label_dy);
        }
        "h-cyl" | "das" | "horizontal-cylinder" => {
            shapes::render_horizontal_cylinder(out, layout_node, &style, &mut label_dx);
        }
        "win-pane" | "internal-storage" | "window-pane" => {
            shapes::render_window_pane(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                &mut label_dx,
                &mut label_dy,
            );
        }
        "diamond" | "question" | "diam" => {
            shapes::render_diamond(out, layout_node, style.as_str());
        }
        "circle" => {
            shapes::render_circle(out, layout_node, style.as_str());
        }
        "doublecircle" | "dbl-circ" | "double-circle" => {
            shapes::render_double_circle(out, layout_node, style.as_str());
        }
        "roundedRect" | "rounded" => {
            shapes::render_rounded_rect(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "stadium" => {
            shapes::render_stadium(
                out,
                ctx,
                label_text,
                label_type,
                node_classes,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "hexagon" | "hex" => {
            shapes::render_hexagon(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
            );
        }
        "lean_right" | "lean-r" | "lean-right" => {
            shapes::render_lean_right(out, layout_node, style.as_str());
        }
        "lean_left" | "lean-l" | "lean-left" => {
            shapes::render_lean_left(out, layout_node, style.as_str());
        }
        "trapezoid" | "trap-b" => {
            shapes::render_trapezoid(out, layout_node, style.as_str());
        }
        "inv_trapezoid" | "inv-trapezoid" | "trap-t" => {
            shapes::render_inv_trapezoid(out, layout_node, style.as_str());
        }
        "odd" => {
            shapes::render_odd(
                out,
                layout_node,
                &style,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
                timing_enabled,
                details,
                &mut label_dx,
            );
        }
        "text" => {
            // Mermaid `text.ts`: invisible rect used only to size/position the label.
            let w = layout_node.width.max(0.0);
            let h = layout_node.height.max(0.0);
            let _ = write!(
                out,
                r#"<rect class="text" style="{}" rx="0" ry="0" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(&style),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h)
            );
        }
        _ => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let _ = write!(
                out,
                r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                escape_attr(&style),
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                fmt(w),
                fmt(h)
            );
        }
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
