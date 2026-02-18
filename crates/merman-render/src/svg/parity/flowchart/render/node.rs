//! Flowchart node renderer.

use super::super::*;
use super::root::flowchart_wrap_svg_text_lines;
use crate::svg::parity::util;

mod geom;
mod helpers;
mod roughjs;
mod shapes;

use geom::{generate_circle_points, generate_full_sine_wave_points, path_from_points};
use roughjs::{roughjs_paths_for_svg_path, roughjs_stroke_path_for_svg_path};

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

    if wrapped_in_a {
        if let Some(href) = href {
            out.push_str(r#"<a xlink:href=""#);
            escape_xml_into(out, href);
            out.push_str(r#"" transform="translate("#);
            util::fmt_into(out, x);
            out.push_str(", ");
            util::fmt_into(out, y);
            out.push_str(r#")">"#);
        } else {
            out.push_str(r#"<a transform="translate("#);
            util::fmt_into(out, x);
            out.push_str(", ");
            util::fmt_into(out, y);
            out.push_str(r#")">"#);
        }
        out.push_str(r#"<g class=""#);
        helpers::write_class_attr(out, class_attr_base, node_classes);
        if let Some(dom_idx) = dom_idx {
            out.push_str(r#"" id="flowchart-"#);
            escape_xml_into(out, node_id);
            let _ = write!(out, "-{dom_idx}\"");
        } else {
            out.push_str(r#"" id=""#);
            escape_xml_into(out, node_id);
            out.push('"');
        }
    } else {
        out.push_str(r#"<g class=""#);
        helpers::write_class_attr(out, class_attr_base, node_classes);
        if let Some(dom_idx) = dom_idx {
            out.push_str(r#"" id="flowchart-"#);
            escape_xml_into(out, node_id);
            let _ = write!(out, r#"-{dom_idx}" transform="translate("#);
            util::fmt_into(out, x);
            out.push_str(", ");
            util::fmt_into(out, y);
            out.push_str(r#")""#);
        } else {
            out.push_str(r#"" id=""#);
            escape_xml_into(out, node_id);
            out.push_str(r#"" transform="translate("#);
            util::fmt_into(out, x);
            out.push_str(", ");
            util::fmt_into(out, y);
            out.push_str(r#")""#);
        }
    }
    if tooltip_enabled {
        let _ = write!(out, r#" title="{}""#, escape_attr_display(tooltip));
    }
    out.push('>');

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

    macro_rules! rough_timed {
        ($expr:expr) => {{
            if timing_enabled {
                details.node_roughjs_calls += 1;
                let start = std::time::Instant::now();
                let out = $expr;
                details.node_roughjs += start.elapsed();
                out
            } else {
                $expr
            }
        }};
    }

    macro_rules! label_html_timed {
        ($expr:expr) => {{
            if timing_enabled {
                details.node_label_html_calls += 1;
                let start = std::time::Instant::now();
                let out = $expr;
                details.node_label_html += start.elapsed();
                out
            } else {
                $expr
            }
        }};
    }

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
            let label_text_plain =
                flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
            let node_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
                &ctx.text_style,
                ctx.class_defs,
                node_classes,
                node_styles,
            );
            let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                ctx.measurer,
                label_text,
                label_type,
                &node_text_style,
                Some(ctx.wrapping_width),
                ctx.node_wrap_mode,
            );
            let span_css_height_parity = node_classes.iter().any(|c| {
                ctx.class_defs.get(c.as_str()).is_some_and(|styles| {
                    styles.iter().any(|s| {
                        matches!(
                            s.split_once(':').map(|p| p.0.trim()),
                            Some("background" | "border")
                        )
                    })
                })
            });
            if span_css_height_parity {
                crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                    &mut metrics,
                    &node_text_style,
                );
            }
            let label_has_visual_content = flowchart_html_contains_img_tag(label_text)
                || (label_type == "markdown" && label_text.contains("!["));
            if label_text_plain.trim().is_empty() && !label_has_visual_content {
                metrics.width = 0.0;
                metrics.height = 0.0;
            }

            let p = ctx.node_padding;
            let min_width = 80.0;
            let min_height = 50.0;
            let w = (metrics.width + 2.0 * p).max(min_width);
            let h = (metrics.height + 2.0 * p).max(min_height);
            let radius = h / 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0, -h / 2.0));
            points.push((w / 2.0 - radius, -h / 2.0));
            points.extend(generate_circle_points(
                -w / 2.0 + radius,
                0.0,
                radius,
                50,
                90.0,
                270.0,
            ));
            points.push((w / 2.0 - radius, h / 2.0));
            points.push((-w / 2.0, h / 2.0));

            let path_data = path_from_points(&points);
            let (fill_d, stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g class="basic label-container"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
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
            let h = layout_node.height.max(1.0);
            let pts = vec![(0.0, -h), (h, -h), (h / 2.0, 0.0)];
            let path_data = path_from_points(&pts);
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<g transform="translate({}, {})">"#,
                    fmt_display(-h / 2.0),
                    fmt_display(h / 2.0)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_xml_display(&fill_d),
                    escape_xml_display(fill_color),
                    escape_xml_display(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_xml_display(&stroke_d),
                    escape_xml_display(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_xml_display(stroke_dasharray),
                    escape_xml_display(&style)
                );
                out.push_str("</g>");
            }
        }
        "manual-input" | "sloped-rectangle" | "sl-rect" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let x = -w / 2.0;
            let y = -h / 2.0;
            let points = vec![(x, y), (x, y + h), (x + w, y + h), (x + w, y - h / 2.0)];
            let path_data = path_from_points(&points);
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container" transform="translate(0, {})">"#,
                    fmt(h / 4.0)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_xml_display(&fill_d),
                    escape_xml_display(fill_color),
                    escape_xml_display(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_xml_display(&stroke_d),
                    escape_xml_display(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_xml_display(stroke_dasharray),
                    escape_xml_display(&style)
                );
                out.push_str("</g>");
            }
        }
        "docs" | "documents" | "st-doc" | "stacked-document" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let wave_amplitude = h / 4.0;
            let final_h = h + wave_amplitude;
            let x = -w / 2.0;
            let y = -final_h / 2.0;
            let rect_offset = 5.0;

            let wave_points = generate_full_sine_wave_points(
                x - rect_offset,
                y + final_h + rect_offset,
                x + w - rect_offset,
                y + final_h + rect_offset,
                wave_amplitude,
                0.8,
            );
            let (_last_x, last_y) = wave_points[wave_points.len() - 1];

            let mut outer_points: Vec<(f64, f64)> = Vec::new();
            outer_points.push((x - rect_offset, y + rect_offset));
            outer_points.push((x - rect_offset, y + final_h + rect_offset));
            outer_points.extend(wave_points.iter().copied());
            outer_points.push((x + w - rect_offset, last_y - rect_offset));
            outer_points.push((x + w, last_y - rect_offset));
            outer_points.push((x + w, last_y - 2.0 * rect_offset));
            outer_points.push((x + w + rect_offset, last_y - 2.0 * rect_offset));
            outer_points.push((x + w + rect_offset, y - rect_offset));
            outer_points.push((x + rect_offset, y - rect_offset));
            outer_points.push((x + rect_offset, y));
            outer_points.push((x, y));
            outer_points.push((x, y + rect_offset));

            let inner_points = vec![
                (x, y + rect_offset),
                (x + w - rect_offset, y + rect_offset),
                (x + w - rect_offset, last_y - rect_offset),
                (x + w, last_y - rect_offset),
                (x + w, y),
                (x, y),
            ];

            let outer_path = path_from_points(&outer_points);
            let inner_path = path_from_points(&inner_points);

            let _ = write!(
                out,
                r#"<g class="basic label-container" transform="translate(0,{})">"#,
                fmt_display(-wave_amplitude / 2.0)
            );
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &outer_path,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_xml_display(&fill_d),
                    escape_xml_display(fill_color),
                    escape_xml_display(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_xml_display(&stroke_d),
                    escape_xml_display(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_xml_display(stroke_dasharray),
                    escape_xml_display(&style)
                );
            }
            out.push_str("<g>");
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &inner_path,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_xml_display(&fill_d),
                    escape_xml_display(fill_color),
                    escape_xml_display(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_xml_display(&stroke_d),
                    escape_xml_display(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_xml_display(stroke_dasharray),
                    escape_xml_display(&style)
                );
            }
            out.push_str("</g></g>");
        }
        "procs" | "processes" | "st-rect" | "stacked-rectangle" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let rect_offset = 5.0;
            let x = -w / 2.0;
            let y = -h / 2.0;

            let outer_points = vec![
                (x - rect_offset, y + rect_offset),
                (x - rect_offset, y + h + rect_offset),
                (x + w - rect_offset, y + h + rect_offset),
                (x + w - rect_offset, y + h),
                (x + w, y + h),
                (x + w, y + h - rect_offset),
                (x + w + rect_offset, y + h - rect_offset),
                (x + w + rect_offset, y - rect_offset),
                (x + rect_offset, y - rect_offset),
                (x + rect_offset, y),
                (x, y),
                (x, y + rect_offset),
            ];

            let inner_points = vec![
                (x, y + rect_offset),
                (x + w - rect_offset, y + rect_offset),
                (x + w - rect_offset, y + h),
                (x + w, y + h),
                (x + w, y),
                (x, y),
            ];

            let outer_path = path_from_points(&outer_points);
            let inner_path = path_from_points(&inner_points);

            out.push_str(r#"<g class="basic label-container">"#);
            out.push_str("<g>");
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &outer_path,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_attr(&fill_d),
                    escape_attr(fill_color),
                    escape_attr(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_attr(stroke_dasharray),
                    escape_attr(&style)
                );
            }
            out.push_str("</g>");
            if let Some(stroke_d) = rough_timed!(roughjs_stroke_path_for_svg_path(
                &inner_path,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_attr(stroke_dasharray),
                    escape_attr(&style)
                );
            }
            out.push_str("</g>");
        }
        "paper-tape" | "flag" => {
            let min_width = 100.0;
            let min_height = 50.0;

            let base_width = layout_node.width.max(1.0);
            let base_height = layout_node.height.max(1.0);
            let aspect_ratio = base_width / base_height.max(1e-9);

            let mut w = base_width;
            let mut h = base_height;
            if w > h * aspect_ratio {
                h = w / aspect_ratio;
            } else {
                w = h * aspect_ratio;
            }
            w = w.max(min_width);
            h = h.max(min_height);

            let wave_amplitude = (h * 0.2).min(h / 4.0);
            let final_h = h + wave_amplitude * 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0,
                final_h / 2.0,
                w / 2.0,
                final_h / 2.0,
                wave_amplitude,
                1.0,
            ));
            points.push((w / 2.0, -final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                w / 2.0,
                -final_h / 2.0,
                -w / 2.0,
                -final_h / 2.0,
                wave_amplitude,
                -1.0,
            ));

            let path_data = path_from_points(&points);
            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                out.push_str(r#"<g class="basic label-container">"#);
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_attr(&fill_d),
                    escape_attr(fill_color),
                    escape_attr(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_attr(stroke_dasharray),
                    escape_attr(&style)
                );
                out.push_str("</g>");
            }
        }
        "subroutine" | "fr-rect" | "subproc" | "subprocess" => {
            // Mermaid `subroutine.ts` (non-handDrawn): polygon via `insertPolygonShape(...)`.
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - 16.0).max(1.0);

            let pts: Vec<(f64, f64)> = vec![
                (0.0, 0.0),
                (w, 0.0),
                (w, -h),
                (0.0, -h),
                (0.0, 0.0),
                (-8.0, 0.0),
                (w + 8.0, 0.0),
                (w + 8.0, -h),
                (-8.0, -h),
                (-8.0, 0.0),
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
                OptionalStyleAttr(style.as_str())
            );
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
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let _ = write!(
                out,
                r#"<polygon points="{},0 {},{} {},{} 0,{}" class="label-container" transform="translate({}, {})"{} />"#,
                fmt(w / 2.0),
                fmt(w),
                fmt(-h / 2.0),
                fmt(w / 2.0),
                fmt(-h),
                fmt(-h / 2.0),
                fmt(-w / 2.0 + 0.5),
                fmt(h / 2.0),
                OptionalStyleAttr(style.as_str())
            );
        }
        "circle" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let r = (w.min(h) / 2.0).max(0.5);
            let _ = write!(
                out,
                r#"<circle class="basic label-container" style="{}" r="{}" cx="0" cy="0"/>"#,
                escape_attr(&style),
                fmt(r),
            );
        }
        "doublecircle" | "dbl-circ" | "double-circle" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let r = (w.min(h) / 2.0).max(0.5);
            let inner = (r - 5.0).max(0.5);
            let _ = write!(
                out,
                r#"<g class="basic label-container" style="{}"><circle class="outer-circle" cx="0" cy="0" r="{}" style="{}"/><circle class="inner-circle" cx="0" cy="0" r="{}" style="{}"/></g>"#,
                escape_attr(&style),
                fmt(r),
                escape_attr(&style),
                fmt(inner),
                escape_attr(&style),
            );
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
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h / 4.0).max(1.0);
            let x = -w / 2.0;
            let y = -h / 2.0;
            let notch = y / 2.0;
            let dx = -notch / 2.0;
            label_dx = dx;

            let pts: Vec<(f64, f64)> =
                vec![(x + notch, y), (x, 0.0), (x + notch, -y), (-x, -y), (-x, y)];
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                let _ = write!(
                    out,
                    r#"<g class="basic label-container" transform="translate({},0)">"#,
                    fmt(dx)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}" style="{}"/>"#,
                    escape_attr(&fill_d),
                    escape_attr(fill_color),
                    escape_attr(&style)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}" style="{}"/>"#,
                    escape_attr(&stroke_d),
                    escape_attr(stroke_color),
                    fmt_display(stroke_width as f64),
                    escape_attr(stroke_dasharray),
                    escape_attr(&style)
                );
                out.push_str("</g>");
            } else {
                let _ = write!(
                    out,
                    r#"<polygon points="{},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({}, {})"{} />"#,
                    fmt(x + notch),
                    fmt(y),
                    fmt(x),
                    fmt(0.0),
                    fmt(x + notch),
                    fmt(-y),
                    fmt(-x),
                    fmt(-y),
                    fmt(-x),
                    fmt(y),
                    fmt(dx),
                    fmt(0.0),
                    OptionalStyleAttr(style.as_str())
                );
            }
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

    let label_text_plain = flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
    let node_text_style = crate::flowchart::flowchart_effective_text_style_for_classes(
        &ctx.text_style,
        ctx.class_defs,
        node_classes,
        node_styles,
    );
    let mut metrics =
        if let (Some(w), Some(h)) = (layout_node.label_width, layout_node.label_height) {
            // Layout already had to measure labels to compute node sizes. Carry those metrics forward so
            // render does not repeat expensive HTML/markdown measurement work.
            crate::text::TextMetrics {
                width: w,
                height: h,
                line_count: 0,
            }
        } else {
            let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                ctx.measurer,
                label_text,
                label_type,
                &node_text_style,
                Some(ctx.wrapping_width),
                ctx.node_wrap_mode,
            );
            let span_css_height_parity = node_classes.iter().any(|c| {
                ctx.class_defs.get(c.as_str()).is_some_and(|styles| {
                    styles.iter().any(|s| {
                        matches!(
                            s.split_once(':').map(|p| p.0.trim()),
                            Some("background" | "border")
                        )
                    })
                })
            });
            if span_css_height_parity {
                crate::text::flowchart_apply_mermaid_styled_node_height_parity(
                    &mut metrics,
                    &node_text_style,
                );
            }
            metrics
        };
    let label_has_visual_content = flowchart_html_contains_img_tag(label_text)
        || (label_type == "markdown" && label_text.contains("!["));
    if label_text_plain.trim().is_empty() && !label_has_visual_content {
        metrics.width = 0.0;
        metrics.height = 0.0;
    }
    if !ctx.node_html_labels {
        let _ = write!(
            out,
            r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><g><rect class="background" style="stroke: none"/>"#,
            escape_xml_display(&compiled_styles.label_style),
            fmt_display(label_dx),
            fmt_display(-metrics.height / 2.0 + label_dy)
        );
        let wrapped = flowchart_wrap_svg_text_lines(
            ctx.measurer,
            &label_text_plain,
            &node_text_style,
            Some(ctx.wrapping_width),
            true,
        )
        .join("\n");
        write_flowchart_svg_text(out, &wrapped, true);
        out.push_str("</g></g></g>");
    } else {
        let label_html =
            label_html_timed!(flowchart_label_html(label_text, label_type, ctx.config));
        let span_style_attr = OptionalStyleXmlAttr(compiled_styles.label_style.as_str());
        let needs_wrap = if ctx.node_wrap_mode == crate::text::WrapMode::HtmlLike {
            let has_inline_style_tags = ctx.node_html_labels && label_type != "markdown" && {
                let lower = label_text.to_ascii_lowercase();
                crate::text::flowchart_html_has_inline_style_tags(&lower)
            };

            let raw = if label_type == "markdown" {
                crate::text::measure_markdown_with_flowchart_bold_deltas(
                    ctx.measurer,
                    label_text,
                    &node_text_style,
                    None,
                    ctx.node_wrap_mode,
                )
                .width
            } else if has_inline_style_tags {
                crate::text::measure_html_with_flowchart_bold_deltas(
                    ctx.measurer,
                    label_text,
                    &node_text_style,
                    None,
                    ctx.node_wrap_mode,
                )
                .width
            } else {
                ctx.measurer
                    .measure_wrapped(
                        &label_text_plain,
                        &node_text_style,
                        None,
                        ctx.node_wrap_mode,
                    )
                    .width
            };
            raw > ctx.wrapping_width
        } else {
            false
        };

        fn parse_hex_rgb_u8(v: &str) -> Option<(u8, u8, u8)> {
            let v = v.trim();
            let hex = v.strip_prefix('#')?;
            match hex.len() {
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    Some((r, g, b))
                }
                3 => {
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    Some((r, g, b))
                }
                _ => None,
            }
        }

        let mut div_style = String::new();
        if let Some(color) = compiled_styles.label_color.as_deref() {
            let color = color.trim();
            if !color.is_empty() {
                if let Some((r, g, b)) = parse_hex_rgb_u8(color) {
                    let _ = write!(&mut div_style, "color: rgb({r}, {g}, {b}) !important; ");
                } else {
                    div_style.push_str("color: ");
                    div_style.push_str(&color.to_ascii_lowercase());
                    div_style.push_str(" !important; ");
                }
            }
        }
        if let Some(v) = compiled_styles.label_font_size.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-size: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_font_weight.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-weight: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_font_family.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "font-family: {v} !important; ");
            }
        }
        if let Some(v) = compiled_styles.label_opacity.as_deref() {
            let v = v.trim();
            if !v.is_empty() {
                let _ = write!(&mut div_style, "opacity: {v} !important; ");
            }
        }
        if needs_wrap {
            let _ = write!(
                &mut div_style,
                "display: table; white-space: break-spaces; line-height: 1.5; max-width: 200px; text-align: center; width: {}px;",
                fmt_display(ctx.wrapping_width)
            );
        } else {
            div_style.push_str(
                "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;",
            );
        }
        if compact_label_translate {
            let _ = write!(
                out,
                r#"<g class="label" style="{}" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                escape_xml_display(&compiled_styles.label_style),
                fmt_display(-metrics.width / 2.0 + label_dx),
                fmt_display(-metrics.height / 2.0 + label_dy),
                fmt_display(metrics.width),
                fmt_display(metrics.height),
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
        } else {
            let _ = write!(
                out,
                r#"<g class="label" style="{}" transform="translate({}, {})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="{}"><span class="nodeLabel"{}>{}</span></div></foreignObject></g></g>"#,
                escape_xml_display(&compiled_styles.label_style),
                fmt_display(-metrics.width / 2.0 + label_dx),
                fmt_display(-metrics.height / 2.0 + label_dy),
                fmt_display(metrics.width),
                fmt_display(metrics.height),
                escape_xml_display(&div_style),
                span_style_attr,
                label_html
            );
        }
    }
    if wrapped_in_a {
        out.push_str("</a>");
    }
}
