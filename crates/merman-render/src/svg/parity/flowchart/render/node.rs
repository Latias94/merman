//! Flowchart node renderer.

use super::super::*;
use super::root::flowchart_wrap_svg_text_lines;
use crate::svg::parity::util;

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

    fn is_self_loop_label_node_id(id: &str) -> bool {
        let mut parts = id.split("---");
        let Some(a) = parts.next() else {
            return false;
        };
        let Some(b) = parts.next() else {
            return false;
        };
        let Some(n) = parts.next() else {
            return false;
        };
        parts.next().is_none() && a == b && (n == "1" || n == "2")
    }

    if is_self_loop_label_node_id(node_id) {
        let _ = write!(
            out,
            r#"<g class="label edgeLabel" id="{}" transform="translate({}, {})"><rect width="0.1" height="0.1"/><g class="label" style="" transform="translate(0, 0)"><rect/><foreignObject width="0" height="0"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 10px; text-align: center;"><span class="nodeLabel"></span></div></foreignObject></g></g>"#,
            escape_xml_display(node_id),
            fmt_display(x),
            fmt_display(y)
        );
        return;
    }

    fn href_is_safe_in_strict_mode(href: &str, config: &merman_core::MermaidConfig) -> bool {
        if config.get_str("securityLevel") == Some("loose") {
            return true;
        }

        let href = href.trim();
        if href.is_empty() {
            return false;
        }

        let lower = href.to_ascii_lowercase();
        if lower.starts_with('#')
            || lower.starts_with("mailto:")
            || lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("//")
            || lower.starts_with('/')
            || lower.starts_with("./")
            || lower.starts_with("../")
        {
            return true;
        }

        // In Mermaid's browser pipeline, the rendered SVG is further sanitized in strict mode.
        // This strips unknown deep-link schemes (e.g. `notes://...`) from `xlink:href`.
        !lower.contains("://")
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
                .filter(|u| href_is_safe_in_strict_mode(u, ctx.config));
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

    fn write_class_attr(out: &mut String, base: &str, classes: &[String]) {
        escape_xml_into(out, base);
        for c in classes {
            let t = c.trim();
            if t.is_empty() {
                continue;
            }
            out.push(' ');
            escape_xml_into(out, t);
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
        write_class_attr(out, class_attr_base, node_classes);
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
        write_class_attr(out, class_attr_base, node_classes);
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

    fn parse_hex_color_to_srgba(s: &str) -> Option<roughr::Srgba> {
        let s = s.trim();
        let hex = s.strip_prefix('#')?;
        let (r, g, b) = match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                (r, g, b)
            }
            3 => {
                let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                (r, g, b)
            }
            _ => return None,
        };
        Some(roughr::Srgba::new(
            r as f32 / 255.0,
            g as f32 / 255.0,
            b as f32 / 255.0,
            1.0,
        ))
    }

    fn path_from_points(points: &[(f64, f64)]) -> String {
        let mut out = String::new();
        for (i, (x, y)) in points.iter().copied().enumerate() {
            let cmd = if i == 0 { 'M' } else { 'L' };
            let _ = write!(&mut out, "{cmd}{x},{y} ");
        }
        out.push('Z');
        out
    }

    fn generate_circle_points(
        center_x: f64,
        center_y: f64,
        radius: f64,
        num_points: usize,
        start_angle_deg: f64,
        end_angle_deg: f64,
    ) -> Vec<(f64, f64)> {
        // Ported from Mermaid `generateCirclePoints(...)` in
        // `packages/mermaid/src/rendering-util/rendering-elements/shapes/util.ts`.
        //
        // Note: Mermaid pushes negated coordinates (`{ x: -x, y: -y }`).
        let start = start_angle_deg.to_radians();
        let end = end_angle_deg.to_radians();
        let angle_range = end - start;
        let step = angle_range / (num_points.saturating_sub(1).max(1) as f64);
        let mut pts: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let angle = start + (i as f64) * step;
            let x = center_x + radius * angle.cos();
            let y = center_y + radius * angle.sin();
            pts.push((-x, -y));
        }
        pts
    }

    fn generate_full_sine_wave_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        amplitude: f64,
        num_cycles: f64,
    ) -> Vec<(f64, f64)> {
        // Ported from Mermaid `generateFullSineWavePoints` (50 segments).
        let steps: usize = 50;
        let delta_x = x2 - x1;
        let delta_y = y2 - y1;
        let cycle_length = delta_x / num_cycles;
        let frequency = (2.0 * std::f64::consts::PI) / cycle_length;
        let mid_y = y1 + delta_y / 2.0;

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(steps + 1);
        for i in 0..=steps {
            let t = (i as f64) / (steps as f64);
            let x = x1 + t * delta_x;
            let y = mid_y + amplitude * (frequency * (x - x1)).sin();
            points.push((x, y));
        }
        points
    }

    fn arc_points(
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        rx: f64,
        ry: f64,
        clockwise: bool,
    ) -> Vec<(f64, f64)> {
        // Port of Mermaid `@11.12.2` `generateArcPoints(...)` in
        // `packages/mermaid/src/rendering-util/rendering-elements/shapes/roundedRect.ts`.
        let num_points: usize = 20;

        let mid_x = (x1 + x2) / 2.0;
        let mid_y = (y1 + y2) / 2.0;
        let angle = (y2 - y1).atan2(x2 - x1);

        let dx = (x2 - x1) / 2.0;
        let dy = (y2 - y1) / 2.0;
        let transformed_x = dx / rx;
        let transformed_y = dy / ry;
        let distance = (transformed_x * transformed_x + transformed_y * transformed_y).sqrt();
        if distance > 1.0 {
            return vec![(x1, y1), (x2, y2)];
        }

        let scaled_center_distance = (1.0 - distance * distance).sqrt();
        let sign = if clockwise { -1.0 } else { 1.0 };
        let center_x = mid_x + scaled_center_distance * ry * angle.sin() * sign;
        let center_y = mid_y - scaled_center_distance * rx * angle.cos() * sign;

        let start_angle = ((y1 - center_y) / ry).atan2((x1 - center_x) / rx);
        let end_angle = ((y2 - center_y) / ry).atan2((x2 - center_x) / rx);

        let mut angle_range = end_angle - start_angle;
        if clockwise && angle_range < 0.0 {
            angle_range += 2.0 * std::f64::consts::PI;
        }
        if !clockwise && angle_range > 0.0 {
            angle_range -= 2.0 * std::f64::consts::PI;
        }

        let mut points: Vec<(f64, f64)> = Vec::with_capacity(num_points);
        for i in 0..num_points {
            let t = i as f64 / (num_points - 1) as f64;
            let a = start_angle + t * angle_range;
            let x = center_x + rx * a.cos();
            let y = center_y + ry * a.sin();
            points.push((x, y));
        }
        points
    }

    fn roughjs_paths_for_svg_path(
        svg_path_data: &str,
        fill: &str,
        stroke: &str,
        stroke_width: f32,
        stroke_dasharray: &str,
        seed: u64,
    ) -> Option<(String, String)> {
        let fill = parse_hex_color_to_srgba(fill)?;
        let stroke = parse_hex_color_to_srgba(stroke)?;
        let dash = stroke_dasharray.trim().replace(',', " ");
        let nums: Vec<f32> = dash
            .split_whitespace()
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        let (dash0, dash1) = match nums.as_slice() {
            [a] => (*a, *a),
            [a, b, ..] => (*a, *b),
            _ => (0.0, 0.0),
        };
        let base_options = roughr::core::OptionsBuilder::default()
            .seed(seed)
            .roughness(0.0)
            .bowing(1.0)
            .fill(fill)
            .fill_style(roughr::core::FillStyle::Solid)
            .stroke(stroke)
            .stroke_width(stroke_width)
            .stroke_line_dash(vec![dash0 as f64, dash1 as f64])
            .stroke_line_dash_offset(0.0)
            .fill_line_dash(vec![0.0, 0.0])
            .fill_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;

        // Rough.js' generator emits path data via `opsToPath(...)`, which uses `Number.toString()`
        // precision (not Mermaid's usual 3-decimal `fmt(...)` formatting). Avoid quantization here.
        fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
            let mut out = String::new();
            for op in &opset.ops {
                match op.op {
                    roughr::core::OpType::Move => {
                        let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
                    }
                    roughr::core::OpType::BCurveTo => {
                        let _ = write!(
                            &mut out,
                            "C{} {}, {} {}, {} {} ",
                            op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                        );
                    }
                    roughr::core::OpType::LineTo => {
                        let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
                    }
                }
            }
            out.trim_end().to_string()
        }

        // Rough.js `generator.path(...)`:
        // - `sets = pointsOnPath(d, 1, distance)`
        // - for solid fill, if `sets.length === 1`: fill path from `svgPath(...)` with
        //   `disableMultiStroke: true`, then drop subsequent `move` ops (`_mergedShape`).
        // - otherwise for solid fill: `solidFillPolygon(sets, o)`
        let distance = (1.0 + base_options.roughness.unwrap_or(1.0) as f64) / 2.0;
        let sets = roughr::points_on_path::points_on_path::<f64>(
            svg_path_data.to_string(),
            Some(1.0),
            Some(distance),
        );

        // Rough.js `generator.path(...)` builds the stroke opset first (`shape = svgPath(d, o)`),
        // which initializes and advances `o.randomizer`. For the solid-fill special-case
        // (`sets.length === 1`), it then calls `svgPath(d, Object.assign({}, o, ...))`, which
        // copies the *existing* `randomizer` by reference and therefore continues the PRNG stream.
        //
        // In headless Rust we model that by emitting the stroke opset first (advancing the
        // in-options PRNG state), then cloning the mutated options for the fill pass.
        let mut stroke_opts = base_options.clone();
        let stroke_opset =
            roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut stroke_opts);

        let fill_opset = if sets.len() == 1 {
            let mut fill_opts = stroke_opts.clone();
            fill_opts.disable_multi_stroke = Some(true);
            let base_rough = fill_opts.roughness.unwrap_or(1.0);
            fill_opts.roughness = Some(if base_rough != 0.0 {
                base_rough + 0.8
            } else {
                0.0
            });

            let mut opset =
                roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut fill_opts);
            opset.ops = opset
                .ops
                .iter()
                .cloned()
                .enumerate()
                .filter_map(|(idx, op)| {
                    if idx != 0 && op.op == roughr::core::OpType::Move {
                        return None;
                    }
                    Some(op)
                })
                .collect();
            opset
        } else {
            let mut fill_opts = stroke_opts.clone();
            roughr::renderer::solid_fill_polygon(&sets, &mut fill_opts)
        };

        Some((
            ops_to_svg_path_d(&fill_opset),
            ops_to_svg_path_d(&stroke_opset),
        ))
    }

    fn roughjs_stroke_path_for_svg_path(
        svg_path_data: &str,
        stroke: &str,
        stroke_width: f32,
        stroke_dasharray: &str,
        seed: u64,
    ) -> Option<String> {
        let stroke = parse_hex_color_to_srgba(stroke)?;
        let dash = stroke_dasharray.trim().replace(',', " ");
        let nums: Vec<f32> = dash
            .split_whitespace()
            .filter_map(|t| t.parse::<f32>().ok())
            .collect();
        let (dash0, dash1) = match nums.as_slice() {
            [a] => (*a, *a),
            [a, b, ..] => (*a, *b),
            _ => (0.0, 0.0),
        };
        let mut options = roughr::core::OptionsBuilder::default()
            .seed(seed)
            .roughness(0.0)
            .bowing(1.0)
            .stroke(stroke)
            .stroke_width(stroke_width)
            .stroke_line_dash(vec![dash0 as f64, dash1 as f64])
            .stroke_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .build()
            .ok()?;

        fn ops_to_svg_path_d(opset: &roughr::core::OpSet<f64>) -> String {
            let mut out = String::new();
            for op in &opset.ops {
                match op.op {
                    roughr::core::OpType::Move => {
                        let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
                    }
                    roughr::core::OpType::BCurveTo => {
                        let _ = write!(
                            &mut out,
                            "C{} {}, {} {}, {} {} ",
                            op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                        );
                    }
                    roughr::core::OpType::LineTo => {
                        let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
                    }
                }
            }
            out.trim_end().to_string()
        }

        let opset = roughr::renderer::svg_path::<f64>(svg_path_data.to_string(), &mut options);
        Some(ops_to_svg_path_d(&opset))
    }

    fn roughjs_circle_path_d(diameter: f64, seed: u64) -> Option<String> {
        // Port of Mermaid `stateEnd.ts`/`stateStart.ts` which use RoughJS even for classic look
        // (roughness=0). Use RoughJS `opsToPath(...)` formatting (no `fmt(...)` quantization).
        let mut opts = roughr::core::OptionsBuilder::default()
            .seed(seed)
            .roughness(0.0)
            .fill_style(roughr::core::FillStyle::Solid)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;
        let opset = roughr::renderer::ellipse::<f64>(0.0, 0.0, diameter, diameter, &mut opts);
        let mut out = String::new();
        for op in &opset.ops {
            match op.op {
                roughr::core::OpType::Move => {
                    let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
                }
                roughr::core::OpType::BCurveTo => {
                    let _ = write!(
                        &mut out,
                        "C{} {}, {} {}, {} {} ",
                        op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                    );
                }
                roughr::core::OpType::LineTo => {
                    let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
                }
            }
        }
        Some(out.trim_end().to_string())
    }

    fn roughjs_paths_for_rect(
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        fill: &str,
        stroke: &str,
        stroke_width: f32,
        seed: u64,
    ) -> Option<(String, String)> {
        // Port of Mermaid `forkJoin.ts` generation order: outline first (advancing PRNG), then fill;
        // SVG emission order is fill first, stroke second.
        let fill = parse_hex_color_to_srgba(fill)?;
        let stroke = parse_hex_color_to_srgba(stroke)?;
        let mut opts = roughr::core::OptionsBuilder::default()
            .seed(seed)
            .roughness(0.0)
            .fill_style(roughr::core::FillStyle::Solid)
            .fill(fill)
            .stroke(stroke)
            .stroke_width(stroke_width)
            .stroke_line_dash(vec![0.0, 0.0])
            .stroke_line_dash_offset(0.0)
            .fill_line_dash(vec![0.0, 0.0])
            .fill_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;

        let fill_poly = vec![vec![
            roughr::Point2D::new(x, y),
            roughr::Point2D::new(x + w, y),
            roughr::Point2D::new(x + w, y + h),
            roughr::Point2D::new(x, y + h),
        ]];
        let stroke_opset = roughr::renderer::rectangle::<f64>(x, y, w, h, &mut opts);
        let fill_opset = roughr::renderer::solid_fill_polygon(&fill_poly, &mut opts);

        fn ops_to_d(opset: &roughr::core::OpSet<f64>) -> String {
            let mut out = String::new();
            for op in &opset.ops {
                match op.op {
                    roughr::core::OpType::Move => {
                        let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
                    }
                    roughr::core::OpType::BCurveTo => {
                        let _ = write!(
                            &mut out,
                            "C{} {}, {} {}, {} {} ",
                            op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                        );
                    }
                    roughr::core::OpType::LineTo => {
                        let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
                    }
                }
            }
            out.trim_end().to_string()
        }

        Some((ops_to_d(&fill_opset), ops_to_d(&stroke_opset)))
    }

    fn roughjs_paths_for_polygon(
        points: &[(f64, f64)],
        fill: &str,
        stroke: &str,
        stroke_width: f32,
        seed: u64,
    ) -> Option<(String, String)> {
        // Mirror RoughJS `generator.polygon(...)` generation order: outline first, then fill, then
        // emit fill before outline.
        let fill = parse_hex_color_to_srgba(fill)?;
        let stroke = parse_hex_color_to_srgba(stroke)?;
        let mut opts = roughr::core::OptionsBuilder::default()
            .seed(seed)
            .roughness(0.0)
            .fill_style(roughr::core::FillStyle::Solid)
            .fill(fill)
            .stroke(stroke)
            .stroke_width(stroke_width)
            .stroke_line_dash(vec![0.0, 0.0])
            .stroke_line_dash_offset(0.0)
            .fill_line_dash(vec![0.0, 0.0])
            .fill_line_dash_offset(0.0)
            .disable_multi_stroke(false)
            .disable_multi_stroke_fill(false)
            .build()
            .ok()?;

        let pts: Vec<_> = points
            .iter()
            .copied()
            .map(|(x, y)| roughr::Point2D::new(x, y))
            .collect();
        let outline_opset = roughr::renderer::polygon::<f64>(&pts, &mut opts);
        let fill_opset = roughr::renderer::solid_fill_polygon(&vec![pts.clone()], &mut opts);

        fn ops_to_d(opset: &roughr::core::OpSet<f64>) -> String {
            let mut out = String::new();
            for op in &opset.ops {
                match op.op {
                    roughr::core::OpType::Move => {
                        let _ = write!(&mut out, "M{} {} ", op.data[0], op.data[1]);
                    }
                    roughr::core::OpType::BCurveTo => {
                        let _ = write!(
                            &mut out,
                            "C{} {}, {} {}, {} {} ",
                            op.data[0], op.data[1], op.data[2], op.data[3], op.data[4], op.data[5]
                        );
                    }
                    roughr::core::OpType::LineTo => {
                        let _ = write!(&mut out, "L{} {} ", op.data[0], op.data[1]);
                    }
                }
            }
            out.trim_end().to_string()
        }

        Some((ops_to_d(&fill_opset), ops_to_d(&outline_opset)))
    }

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

    match shape {
        // Flowchart v2 "rendering-elements" aliases for state diagram start/end nodes.
        // Mermaid ignores `node.label` for these shapes and does not emit a label group.
        "sm-circ" | "small-circle" | "start" => {
            out.push_str(r#"<circle class="state-start" r="7" width="14" height="14"/>"#);
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }
        "fr-circ" | "framed-circle" | "stop" => {
            let line_color = theme_color(ctx.config.as_value(), "lineColor", "#333333");
            let inner_fill =
                config_string(ctx.config.as_value(), &["themeVariables", "stateBorder"])
                    .unwrap_or_else(|| ctx.node_border_color.clone());

            let outer_d = rough_timed!(roughjs_circle_path_d(14.0, hand_drawn_seed))
                .unwrap_or_else(|| "M0,0".to_string());
            let inner_d = rough_timed!(roughjs_circle_path_d(5.0, hand_drawn_seed))
                .unwrap_or_else(|| "M0,0".to_string());

            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="2" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                outer_d,
                escape_attr(ctx.node_fill_color.as_str()),
                outer_d,
                escape_attr(&line_color),
                inner_d,
                escape_attr(&inner_fill),
                inner_d,
                escape_attr(&inner_fill),
            );
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }

        // Flowchart v2 fork/join (no label; uses `lineColor` fill/stroke).
        "fork" | "join" => {
            // Mermaid inflates Dagre dimensions after `updateNodeBounds(...)` but does not
            // re-render the bar at the inflated size. Render the canonical shape dimensions.
            let (w, h) = if layout_node.width >= layout_node.height {
                (70.0, 10.0)
            } else {
                (10.0, 70.0)
            };
            let line_color = theme_color(ctx.config.as_value(), "lineColor", "#333333");
            let (fill_d, stroke_d) = rough_timed!(roughjs_paths_for_rect(
                -w / 2.0,
                -h / 2.0,
                w,
                h,
                &line_color,
                &line_color,
                1.3,
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fill_d,
                escape_attr(&line_color),
                stroke_d,
                escape_attr(&line_color),
            );
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }

        // Flowchart v2 lightning bolt (Communication link). Mermaid clears `node.label` and does
        // not emit a label group.
        "bolt" => {
            // Mermaid uses `width = max(35, node.width)` and `height = max(35, node.height)`,
            // then draws a 2*height tall bolt and translates it by `(-width/2, -height)`.
            let width = layout_node.width.max(35.0);
            let height = (layout_node.height / 2.0).max(35.0);
            let gap = 7.0;

            let points: Vec<(f64, f64)> = vec![
                (width, 0.0),
                (0.0, height + gap / 2.0),
                (width - 2.0 * gap, height + gap / 2.0),
                (0.0, 2.0 * height),
                (width, height - gap / 2.0),
                (2.0 * gap, height - gap / 2.0),
            ];
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
                r#"<g transform="translate({},{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"#,
                fmt(-width / 2.0),
                fmt(-height),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }

        // Flowchart v2 filled circle (junction). Mermaid clears `node.label` and does not emit a
        // label group. Note that even in non-handDrawn mode Mermaid still uses RoughJS circle
        // paths (roughness=0), which have a slightly asymmetric bbox in Chromium.
        "f-circ" => {
            let border = config_string(ctx.config.as_value(), &["themeVariables", "nodeBorder"])
                .unwrap_or_else(|| ctx.node_border_color.clone());

            let d = rough_timed!(roughjs_circle_path_d(14.0, hand_drawn_seed))
                .unwrap_or_else(|| "M0,0".into());
            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style="fill: {} !important;"/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style="fill: {} !important;"/></g>"##,
                escape_attr(&d),
                escape_attr(fill_color),
                escape_attr(&border),
                escape_attr(&d),
                escape_attr(stroke_color),
                escape_attr(&border),
            );
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }

        // Flowchart v2 crossed circle (summary). Mermaid clears `node.label` and does not emit a
        // label group.
        "cross-circ" => {
            // Mermaid uses `radius = max(30, node.width)` before `updateNodeBounds(...)`. In
            // practice `node.width` is usually unset here, so radius=30.
            let radius = 30.0;

            let circle_d = rough_timed!(roughjs_circle_path_d(radius * 2.0, hand_drawn_seed))
                .unwrap_or_else(|| "M0,0".into());

            // Port of Mermaid `createLine(r)` in `crossedCircle.ts`.
            let x_axis_45 = (std::f64::consts::PI / 4.0).cos();
            let y_axis_45 = (std::f64::consts::PI / 4.0).sin();
            let point_q1 = (radius * x_axis_45, radius * y_axis_45);
            let point_q2 = (-radius * x_axis_45, radius * y_axis_45);
            let point_q3 = (-radius * x_axis_45, -radius * y_axis_45);
            let point_q4 = (radius * x_axis_45, -radius * y_axis_45);
            let line_path = format!(
                "M {},{} L {},{} M {},{} L {},{}",
                point_q2.0,
                point_q2.1,
                point_q4.0,
                point_q4.1,
                point_q1.0,
                point_q1.1,
                point_q3.0,
                point_q3.1
            );
            let (line_fill_d, line_stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &line_path,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("".to_string(), "M0,0".to_string()));

            let _ = write!(
                out,
                r##"<g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                escape_attr(&circle_d),
                escape_attr(fill_color),
                escape_attr(&circle_d),
                escape_attr(stroke_color),
                escape_attr(&line_fill_d),
                escape_attr(fill_color),
                escape_attr(&line_stroke_d),
                escape_attr(stroke_color),
            );
            out.push_str("</g>");
            if wrapped_in_a {
                out.push_str("</a>");
            }
            return;
        }

        // Flowchart v2 hourglass/collate: Mermaid clears `node.label` but still emits an empty
        // label group (via `labelHelper(...)`).
        "hourglass" | "collate" => {
            label_text = "";
            label_type = "text";
            let w = layout_node.width.max(30.0);
            let h = layout_node.height.max(30.0);
            let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (w, 0.0), (0.0, h), (w, h)];
            let path_data = path_from_points(&pts);
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
                r##"<g class="basic label-container" transform="translate({}, {})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 card/notched-rectangle.
        "notch-rect" | "notched-rectangle" | "card" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let notch = 12.0;
            let pts: Vec<(f64, f64)> = vec![
                (notch, -h),
                (w, -h),
                (w, 0.0),
                (0.0, 0.0),
                (0.0, -h + notch),
                (notch, -h),
            ];
            let mut points_attr = String::new();
            for (idx, (px, py)) in pts.iter().copied().enumerate() {
                if idx > 0 {
                    points_attr.push(' ');
                }
                let _ = write!(&mut points_attr, "{},{}", fmt(px), fmt(py));
            }
            let _ = write!(
                out,
                r#"<polygon points="{}" class="label-container" transform="translate({},{})"/>"#,
                points_attr,
                fmt(-w / 2.0),
                fmt(h / 2.0)
            );
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
            // Mirror Mermaid `linedCylinder.ts` (non-handDrawn) + translate.
            let w = layout_node.width.max(1.0);
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            let out_h = layout_node.height.max(1.0);
            let h = (out_h - 2.0 * ry).max(0.0);
            let outer_offset = h * 0.1;

            // Mermaid moves the label down by `ry`.
            label_dy = ry;

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
                r#"<path d="{}" class="basic label-container" style="" transform="translate({}, {})"/>"#,
                escape_attr(&path_data),
                fmt(-w / 2.0),
                fmt(-(h / 2.0 + ry))
            );
        }

        // Flowchart v2 curved trapezoid (Display).
        "curv-trap" => {
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
            let min_height = 20.0;
            let w = ((metrics.width + 2.0 * p) * 1.25).max(min_width);
            let h = (metrics.height + 2.0 * p).max(min_height);
            let radius = h / 2.0;

            let total_width = w;
            let total_height = h;
            let rw = total_width - radius;
            let tw = total_height / 4.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((rw, 0.0));
            points.push((tw, 0.0));
            points.push((0.0, total_height / 2.0));
            points.push((tw, total_height));
            points.push((rw, total_height));
            points.extend(generate_circle_points(
                -rw,
                -total_height / 2.0,
                radius,
                50,
                270.0,
                90.0,
            ));

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
                r##"<g class="basic label-container" transform="translate({}, {})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(-w / 2.0),
                fmt(-h / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 divided rectangle (Divided process).
        "div-rect" => {
            // Mermaid draws the polygon using `h` and then the rendered bbox expands to
            // `out_h = h + rectOffset` where `rectOffset = h * 0.2`, i.e. `out_h = 1.2*h`.
            let out_w = layout_node.width.max(1.0);
            let out_h = layout_node.height.max(1.0);
            let h = out_h / 1.2;
            let w = out_w;
            let rect_offset = h * 0.2;
            let x = -w / 2.0;
            let y = -h / 2.0 - rect_offset / 2.0;

            // Label is shifted down by `rectOffset/2`.
            label_dy = rect_offset / 2.0;

            let pts: Vec<(f64, f64)> = vec![
                (x, y + rect_offset),
                (-x, y + rect_offset),
                (-x, -y),
                (x, -y),
                (x, y),
                (-x, y),
                (-x, y + rect_offset),
            ];
            let (fill_d, stroke_d) = rough_timed!(roughjs_paths_for_polygon(
                &pts,
                fill_color,
                stroke_color,
                1.3,
                hand_drawn_seed
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g class="basic label-container" style=""><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 notched pentagon (Loop limit).
        "notch-pent" => {
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
            let min_width = 60.0;
            let min_height = 20.0;
            let w = (metrics.width + 2.0 * p).max(min_width);
            let h = (metrics.height + 2.0 * p).max(min_height);
            let points = vec![
                ((-w / 2.0) * 0.8, -h / 2.0),
                ((w / 2.0) * 0.8, -h / 2.0),
                (w / 2.0, (-h / 2.0) * 0.6),
                (w / 2.0, h / 2.0),
                (-w / 2.0, h / 2.0),
                (-w / 2.0, (-h / 2.0) * 0.6),
            ];
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

        // Flowchart v2 bow tie rectangle (Stored data).
        "bow-rect" => {
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
            let w = metrics.width + p + 20.0;
            let h = metrics.height + p;
            let ry = h / 2.0;
            let rx = ry / (2.5 + h / 50.0);

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((w / 2.0, -h / 2.0));
            points.push((-w / 2.0, -h / 2.0));
            points.extend(arc_points(
                -w / 2.0,
                -h / 2.0,
                -w / 2.0,
                h / 2.0,
                rx,
                ry,
                false,
            ));
            points.push((w / 2.0, h / 2.0));
            points.extend(arc_points(
                w / 2.0,
                h / 2.0,
                w / 2.0,
                -h / 2.0,
                rx,
                ry,
                true,
            ));

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
                r##"<g class="basic label-container" transform="translate({}, 0)"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(rx / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 tagged rectangle (Tagged process).
        "tag-rect" => {
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
            let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
            let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
            let x = -w / 2.0;
            let y = -h / 2.0;
            let tag_w = 0.2 * h;
            let tag_h = 0.2 * h;

            let rect_points = vec![
                (x - tag_w / 2.0, y),
                (x + w + tag_w / 2.0, y),
                (x + w + tag_w / 2.0, y + h),
                (x - tag_w / 2.0, y + h),
            ];
            let tag_points = vec![
                (x + w - tag_w / 2.0, y + h),
                (x + w + tag_w / 2.0, y + h),
                (x + w + tag_w / 2.0, y + h - tag_h),
            ];

            let rect_path = path_from_points(&rect_points);
            let (rect_fill_d, rect_stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &rect_path,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

            let tag_path = path_from_points(&tag_points);
            let (tag_fill_d, tag_stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &tag_path,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

            let _ = write!(
                out,
                r##"<g class="basic label-container"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                escape_attr(&rect_fill_d),
                escape_attr(fill_color),
                escape_attr(&rect_stroke_d),
                escape_attr(stroke_color),
                escape_attr(&tag_fill_d),
                escape_attr(fill_color),
                escape_attr(&tag_stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 wave edged rectangle (Document).
        "doc" => {
            compact_label_translate = true;

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
            let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
            let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
            let wave_amplitude = h / 8.0;
            let final_h = h + wave_amplitude;

            // Mermaid keeps a minimum width (70px) for wave edged rectangles.
            let min_width = 70.0;
            let extra_w = ((min_width - w).max(0.0)) / 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - extra_w, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - extra_w,
                final_h / 2.0,
                w / 2.0 + extra_w,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + extra_w, -final_h / 2.0));
            points.push((-w / 2.0 - extra_w, -final_h / 2.0));

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
                r##"<g class="basic label-container" transform="translate(0,{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(-wave_amplitude / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );

            // Mirror Mermaid `waveEdgedRectangle.ts` label placement.
            label_dx = -w / 2.0 + p + metrics.width / 2.0;
            label_dy = -h / 2.0 + p - wave_amplitude + metrics.height / 2.0;
        }

        // Flowchart v2 lined wave edged rectangle (Lined document).
        "lin-doc" => {
            compact_label_translate = true;

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
            let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
            let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
            let wave_amplitude = h / 4.0;
            let final_h = h + wave_amplitude;
            let ext = (w / 2.0) * 0.1;

            // Mermaid nudges label by half the left extension, and shifts it up by waveAmplitude/2.
            label_dx = ext / 2.0;
            label_dy = -wave_amplitude / 2.0;

            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - ext, -final_h / 2.0));
            points.push((-w / 2.0 - ext, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - ext,
                final_h / 2.0,
                w / 2.0 + ext,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + ext, -final_h / 2.0));
            points.push((-w / 2.0 - ext, -final_h / 2.0));
            points.push((-w / 2.0, -final_h / 2.0));
            points.push((-w / 2.0, (final_h / 2.0) * 1.1));
            points.push((-w / 2.0, -final_h / 2.0));

            let (fill_d, stroke_d) = rough_timed!(roughjs_paths_for_polygon(
                &points,
                fill_color,
                stroke_color,
                1.3,
                hand_drawn_seed
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g class="basic label-container" transform="translate(0,{})"><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(-wave_amplitude / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 tagged wave edged rectangle (Tagged document).
        "tag-doc" => {
            compact_label_translate = true;

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
            let w = (metrics.width + 2.0 * p).max(layout_node.width.max(0.0));
            let h = (metrics.height + 2.0 * p).max(layout_node.height.max(0.0));
            let wave_amplitude = h / 4.0;
            let tag_width = 0.2 * w;
            let tag_height = 0.2 * h;
            let final_h = h + wave_amplitude;

            // Mermaid shifts label to the left padding origin and up by waveAmplitude/2.
            label_dx = -w / 2.0 + p + metrics.width / 2.0;
            label_dy = -h / 2.0 + p - wave_amplitude / 2.0 + metrics.height / 2.0;

            let ext = (w / 2.0) * 0.1;
            let mut points: Vec<(f64, f64)> = Vec::new();
            points.push((-w / 2.0 - ext, final_h / 2.0));
            points.extend(generate_full_sine_wave_points(
                -w / 2.0 - ext,
                final_h / 2.0,
                w / 2.0 + ext,
                final_h / 2.0,
                wave_amplitude,
                0.8,
            ));
            points.push((w / 2.0 + ext, -final_h / 2.0));
            points.push((-w / 2.0 - ext, -final_h / 2.0));

            let x = -w / 2.0 + ext;
            let y = -final_h / 2.0 - tag_height * 0.4;
            let mut tag_points: Vec<(f64, f64)> = Vec::new();
            tag_points.push((x + w - tag_width, (y + h) * 1.4));
            tag_points.push((x + w, y + h - tag_height));
            tag_points.push((x + w, (y + h) * 0.9));
            tag_points.extend(generate_full_sine_wave_points(
                x + w,
                (y + h) * 1.3,
                x + w - tag_width,
                (y + h) * 1.5,
                -h * 0.03,
                0.5,
            ));

            let wave_rect_path = path_from_points(&points);
            let (wave_fill_d, wave_stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &wave_rect_path,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

            let tag_path = path_from_points(&tag_points);
            let (tag_fill_d, tag_stroke_d) = rough_timed!(roughjs_paths_for_svg_path(
                &tag_path,
                fill_color,
                stroke_color,
                1.3,
                "0 0",
                hand_drawn_seed,
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));

            let _ = write!(
                out,
                r##"<g class="basic label-container" transform="translate(0,{})"><g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                fmt(-wave_amplitude / 2.0),
                escape_attr(&wave_fill_d),
                escape_attr(fill_color),
                escape_attr(&wave_stroke_d),
                escape_attr(stroke_color),
                escape_attr(&tag_fill_d),
                escape_attr(fill_color),
                escape_attr(&tag_stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 triangle (Extract).
        "tri" => {
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
            let w = metrics.width + p;
            let h = w + metrics.height;
            let tw = w + metrics.height;
            let pts = vec![(0.0, 0.0), (tw, 0.0), (tw / 2.0, -h)];
            let path_data = path_from_points(&pts);
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
                r#"<g transform="translate({}, {})"><path d="{}" stroke="none" stroke-width="0" fill="{}" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"#,
                fmt(-h / 2.0),
                fmt(h / 2.0),
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );

            // Mermaid places the label near the base; in htmlLabels mode the padding term is /2.
            label_dy = h / 2.0 - metrics.height / 2.0 - p / 2.0;
        }

        // Flowchart v2 shaded process / lined rectangle.
        "lin-rect" | "lined-rectangle" | "lined-process" | "lin-proc" => {
            // Mermaid `shadedProcess.ts`:
            // - outer bbox includes an extra 8px on both sides (and an internal vertical line),
            // - label is nudged +4px on x.
            label_dx = 4.0;
            compact_label_translate = true;
            let out_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (out_w - 16.0).max(1.0);
            let x = -out_w / 2.0 + 8.0;
            let y = -h / 2.0;
            let pts: Vec<(f64, f64)> = vec![
                (x, y),
                (x + w + 8.0, y),
                (x + w + 8.0, y + h),
                (x - 8.0, y + h),
                (x - 8.0, y),
                (x, y),
                (x, y + h),
            ];
            let (fill_d, stroke_d) = rough_timed!(roughjs_paths_for_polygon(
                &pts,
                fill_color,
                stroke_color,
                1.3,
                hand_drawn_seed
            ))
            .unwrap_or_else(|| ("M0,0".to_string(), "M0,0".to_string()));
            let _ = write!(
                out,
                r##"<g class="basic label-container" style=""><path d="{}" stroke="none" stroke-width="0" fill="{}" fill-rule="evenodd" style=""/><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g>"##,
                escape_attr(&fill_d),
                escape_attr(fill_color),
                escape_attr(&stroke_d),
                escape_attr(stroke_color),
            );
        }

        // Flowchart v2 curly brace/comment shapes (rendering-elements).
        "comment" | "brace" | "brace-l" | "brace-r" | "braces" => {
            fn circle_points(
                center_x: f64,
                center_y: f64,
                radius: f64,
                num_points: usize,
                start_deg: f64,
                end_deg: f64,
                negate: bool,
            ) -> Vec<(f64, f64)> {
                let start = start_deg.to_radians();
                let end = end_deg.to_radians();
                let angle_range = end - start;
                let angle_step = if num_points > 1 {
                    angle_range / (num_points as f64 - 1.0)
                } else {
                    0.0
                };
                let mut out: Vec<(f64, f64)> = Vec::with_capacity(num_points);
                for i in 0..num_points {
                    let a = start + (i as f64) * angle_step;
                    let x = center_x + radius * a.cos();
                    let y = center_y + radius * a.sin();
                    if negate {
                        out.push((-x, -y));
                    } else {
                        out.push((x, y));
                    }
                }
                out
            }

            let out_w = layout_node.width.max(1.0);
            let out_h = layout_node.height.max(1.0);

            // Mermaid's `label.attr('transform', ...)` for curly brace shapes renders without a
            // space after the comma (e.g. `translate(-34.265625,-12)`).
            compact_label_translate = true;

            // Radius depends on the *inner* height in Mermaid (`h = bbox.height + padding`).
            // Solve `radius = max(5, (out_h - 2*radius) * 0.1)` by a few fixed-point iterations.
            let mut radius: f64 = 5.0;
            for _ in 0..3 {
                let inner_h = (out_h - 2.0 * radius).max(0.0);
                let next = (inner_h * 0.1).max(5.0);
                if (next - radius).abs() < 1e-9 {
                    break;
                }
                radius = next;
            }
            let h = (out_h - 2.0 * radius).max(0.0);

            let w = match shape {
                "comment" | "brace" | "brace-l" => (out_w - 2.0 * radius) / 1.1,
                "brace-r" | "braces" => out_w - 3.0 * radius,
                _ => out_w - 3.0 * radius,
            };

            let (group_tx, local_label_dx) = match shape {
                "comment" | "brace" | "brace-l" => (radius, -radius / 2.0),
                "brace-r" => (-radius, 0.0),
                "braces" => (radius - radius / 4.0, 0.0),
                _ => (0.0, 0.0),
            };
            label_dx = local_label_dx;

            let mut stroke_d = |d: &str| {
                rough_timed!(roughjs_stroke_path_for_svg_path(
                    d,
                    stroke_color,
                    1.3,
                    "0 0",
                    hand_drawn_seed
                ))
                .unwrap_or_else(|| "M0,0".to_string())
            };

            if shape == "braces" {
                // Mermaid `curlyBraces.ts`: two visible brace paths + one invisible rect path.
                let left_points: Vec<(f64, f64)> = [
                    circle_points(w / 2.0, -h / 2.0, radius, 30, -90.0, 0.0, true),
                    vec![(-w / 2.0 - radius, radius)],
                    circle_points(
                        w / 2.0 + radius * 2.0,
                        -radius,
                        radius,
                        20,
                        -180.0,
                        -270.0,
                        true,
                    ),
                    circle_points(
                        w / 2.0 + radius * 2.0,
                        radius,
                        radius,
                        20,
                        -90.0,
                        -180.0,
                        true,
                    ),
                    vec![(-w / 2.0 - radius, -h / 2.0)],
                    circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
                ]
                .into_iter()
                .flatten()
                .collect();
                let right_points: Vec<(f64, f64)> = [
                    circle_points(
                        -w / 2.0 + radius + radius / 2.0,
                        -h / 2.0,
                        radius,
                        20,
                        -90.0,
                        -180.0,
                        true,
                    ),
                    vec![(w / 2.0 - radius / 2.0, radius)],
                    circle_points(
                        -w / 2.0 - radius / 2.0,
                        -radius,
                        radius,
                        20,
                        0.0,
                        90.0,
                        true,
                    ),
                    circle_points(
                        -w / 2.0 - radius / 2.0,
                        radius,
                        radius,
                        20,
                        -90.0,
                        0.0,
                        true,
                    ),
                    vec![(w / 2.0 - radius / 2.0, -radius)],
                    circle_points(
                        -w / 2.0 + radius + radius / 2.0,
                        h / 2.0,
                        radius,
                        30,
                        -180.0,
                        -270.0,
                        true,
                    ),
                ]
                .into_iter()
                .flatten()
                .collect();
                let rect_points: Vec<(f64, f64)> = [
                    vec![(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)],
                    circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, true),
                    vec![(-w / 2.0 - radius, -radius)],
                    circle_points(
                        w / 2.0 + radius * 2.0,
                        -radius,
                        radius,
                        20,
                        -180.0,
                        -270.0,
                        true,
                    ),
                    circle_points(
                        w / 2.0 + radius * 2.0,
                        radius,
                        radius,
                        20,
                        -90.0,
                        -180.0,
                        true,
                    ),
                    vec![(-w / 2.0 - radius, h / 2.0)],
                    circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
                    vec![
                        (-w / 2.0, h / 2.0 + radius),
                        (w / 2.0 - radius - radius / 2.0, h / 2.0 + radius),
                    ],
                    circle_points(
                        -w / 2.0 + radius + radius / 2.0,
                        -h / 2.0,
                        radius,
                        20,
                        -90.0,
                        -180.0,
                        true,
                    ),
                    vec![(w / 2.0 - radius / 2.0, radius)],
                    circle_points(
                        -w / 2.0 - radius / 2.0,
                        -radius,
                        radius,
                        20,
                        0.0,
                        90.0,
                        true,
                    ),
                    circle_points(
                        -w / 2.0 - radius / 2.0,
                        radius,
                        radius,
                        20,
                        -90.0,
                        0.0,
                        true,
                    ),
                    vec![(w / 2.0 - radius / 2.0, -radius)],
                    circle_points(
                        -w / 2.0 + radius + radius / 2.0,
                        h / 2.0,
                        radius,
                        30,
                        -180.0,
                        -270.0,
                        true,
                    ),
                ]
                .into_iter()
                .flatten()
                .collect();

                let left_path = path_from_points(&left_points)
                    .trim_end_matches('Z')
                    .to_string();
                let right_path = path_from_points(&right_points)
                    .trim_end_matches('Z')
                    .to_string();
                let rect_path = path_from_points(&rect_points);

                let left_d = stroke_d(&left_path);
                let right_d = stroke_d(&right_path);
                let rect_d = stroke_d(&rect_path);

                let _ = write!(
                    out,
                    r##"<g class="text" transform="translate({}, 0)"><g><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><g><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><g stroke-opacity="0"><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                    fmt(group_tx),
                    escape_attr(&left_d),
                    escape_attr(stroke_color),
                    escape_attr(&right_d),
                    escape_attr(stroke_color),
                    escape_attr(&rect_d),
                    escape_attr(stroke_color),
                );
            } else {
                // Mermaid `curlyBraceLeft.ts` / `curlyBraceRight.ts`.
                let (negate, points, rect_points) = if shape == "brace-r" {
                    let points: Vec<(f64, f64)> = [
                        circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, false),
                        vec![(w / 2.0 + radius, -radius)],
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            -radius,
                            radius,
                            20,
                            -180.0,
                            -270.0,
                            false,
                        ),
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            radius,
                            radius,
                            20,
                            -90.0,
                            -180.0,
                            false,
                        ),
                        vec![(w / 2.0 + radius, h / 2.0)],
                        circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, false),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    let rect_points: Vec<(f64, f64)> = [
                        vec![(-w / 2.0, -h / 2.0 - radius), (w / 2.0, -h / 2.0 - radius)],
                        circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, false),
                        vec![(w / 2.0 + radius, -radius)],
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            -radius,
                            radius,
                            20,
                            -180.0,
                            -270.0,
                            false,
                        ),
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            radius,
                            radius,
                            20,
                            -90.0,
                            -180.0,
                            false,
                        ),
                        vec![(w / 2.0 + radius, h / 2.0)],
                        circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, false),
                        vec![(w / 2.0, h / 2.0 + radius), (-w / 2.0, h / 2.0 + radius)],
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    (false, points, rect_points)
                } else {
                    let points: Vec<(f64, f64)> = [
                        circle_points(w / 2.0, -h / 2.0, radius, 30, -90.0, 0.0, true),
                        vec![(-w / 2.0 - radius, radius)],
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            -radius,
                            radius,
                            20,
                            -180.0,
                            -270.0,
                            true,
                        ),
                        circle_points(
                            w / 2.0 + radius * 2.0,
                            radius,
                            radius,
                            20,
                            -90.0,
                            -180.0,
                            true,
                        ),
                        vec![(-w / 2.0 - radius, -h / 2.0)],
                        circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    let rect_points: Vec<(f64, f64)> = [
                        vec![(w / 2.0, -h / 2.0 - radius), (-w / 2.0, -h / 2.0 - radius)],
                        circle_points(w / 2.0, -h / 2.0, radius, 20, -90.0, 0.0, true),
                        vec![(-w / 2.0 - radius, -radius)],
                        circle_points(w / 2.0 + w * 0.1, -radius, radius, 20, -180.0, -270.0, true),
                        circle_points(w / 2.0 + w * 0.1, radius, radius, 20, -90.0, -180.0, true),
                        vec![(-w / 2.0 - radius, h / 2.0)],
                        circle_points(w / 2.0, h / 2.0, radius, 20, 0.0, 90.0, true),
                        vec![(-w / 2.0, h / 2.0 + radius), (w / 2.0, h / 2.0 + radius)],
                    ]
                    .into_iter()
                    .flatten()
                    .collect();
                    (true, points, rect_points)
                };
                let _ = negate;

                let brace_path = path_from_points(&points).trim_end_matches('Z').to_string();
                let rect_path = path_from_points(&rect_points);
                let brace_d = stroke_d(&brace_path);
                let rect_d = stroke_d(&rect_path);
                let _ = write!(
                    out,
                    r##"<g class="text" transform="translate({}, 0)"><g><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g><g stroke-opacity="0"><path d="{}" stroke="{}" stroke-width="1.3" fill="none" stroke-dasharray="0 0" style=""/></g></g>"##,
                    fmt(group_tx),
                    escape_attr(&brace_d),
                    escape_attr(stroke_color),
                    escape_attr(&rect_d),
                    escape_attr(stroke_color),
                );
            }
        }

        "imageSquare" => {
            // Port of Mermaid `imageSquare.ts` (`image-shape default`).
            if let Some(img_href) = node_img.filter(|s| !s.trim().is_empty()) {
                let label_text_plain =
                    flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
                let has_label = !label_text_plain.trim().is_empty();
                let label_padding = if has_label { 8.0 } else { 0.0 };
                let top_label = node_pos == Some("t");

                let assumed_aspect_ratio = 1.0f64;
                let asset_h = node_asset_height.unwrap_or(60.0).max(1.0);
                let asset_w = node_asset_width.unwrap_or(asset_h).max(1.0);
                let aspect_ratio = if asset_h > 0.0 {
                    asset_w / asset_h
                } else {
                    assumed_aspect_ratio
                };

                let default_width = ctx.wrapping_width.max(0.0);
                let image_raw_width = asset_w.max(if has_label { default_width } else { 0.0 });

                let constraint_on = node_constraint == Some("on");
                let image_width = if constraint_on && node_asset_height.is_some() {
                    asset_h * aspect_ratio
                } else {
                    image_raw_width
                };
                let image_height = if constraint_on {
                    if aspect_ratio != 0.0 {
                        image_width / aspect_ratio
                    } else {
                        asset_h
                    }
                } else {
                    asset_h
                };

                let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                    ctx.measurer,
                    label_text,
                    label_type,
                    &ctx.text_style,
                    Some(ctx.wrapping_width),
                    ctx.node_wrap_mode,
                );
                if !has_label {
                    metrics.width = 0.0;
                    metrics.height = 0.0;
                }

                let outer_w = image_width.max(metrics.width);
                let outer_h = image_height + metrics.height + label_padding;

                let x0 = -image_width / 2.0;
                let y0 = -image_height / 2.0;
                // Mermaid `imageSquare` fills with a straight rect (not rough).
                let rect_fill_path = format!(
                    "M{} {} L{} {} L{} {} L{} {}",
                    fmt_display(x0),
                    fmt_display(y0),
                    fmt_display(x0 + image_width),
                    fmt_display(y0),
                    fmt_display(x0 + image_width),
                    fmt_display(y0 + image_height),
                    fmt_display(x0),
                    fmt_display(y0 + image_height)
                );
                // Stroke uses RoughJS and must be a closed path so the left edge is included.
                let rect_stroke_path = format!(
                    "M{} {} L{} {} L{} {} L{} {} L{} {}",
                    fmt_display(x0),
                    fmt_display(y0),
                    fmt_display(x0 + image_width),
                    fmt_display(y0),
                    fmt_display(x0 + image_width),
                    fmt_display(y0 + image_height),
                    fmt_display(x0),
                    fmt_display(y0 + image_height),
                    fmt_display(x0),
                    fmt_display(y0)
                );

                let icon_dy = if top_label {
                    metrics.height / 2.0 + label_padding / 2.0
                } else {
                    -metrics.height / 2.0 - label_padding / 2.0
                };
                let _ = write!(
                    out,
                    r#"<g transform="translate(0,{})">"#,
                    fmt_display(icon_dy)
                );
                let _ = write!(
                    out,
                    r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
                    escape_xml_display(&rect_fill_path),
                    escape_xml_display(fill_color)
                );
                if let Some(stroke_d) = rough_timed!(roughjs_stroke_path_for_svg_path(
                    &rect_stroke_path,
                    stroke_color,
                    stroke_width,
                    stroke_dasharray,
                    hand_drawn_seed,
                )) {
                    let _ = write!(
                        out,
                        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                        escape_xml_display(&stroke_d),
                        escape_xml_display(stroke_color),
                        fmt_display(stroke_width as f64),
                        escape_xml_display(stroke_dasharray)
                    );
                }
                out.push_str("</g>");

                // Label group uses a background class in Mermaid's image/icon helpers.
                let label_html =
                    label_html_timed!(flowchart_label_html(label_text, label_type, ctx.config));
                let label_dy = if top_label {
                    -image_height / 2.0 - metrics.height / 2.0 - label_padding / 2.0
                } else {
                    image_height / 2.0 - metrics.height / 2.0 + label_padding / 2.0
                };
                let _ = write!(
                    out,
                    r#"<g class="label" style="" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g>"#,
                    fmt_display(-metrics.width / 2.0),
                    fmt_display(label_dy),
                    fmt_display(metrics.width),
                    fmt_display(metrics.height),
                    label_html
                );

                let outer_x0 = -outer_w / 2.0;
                let outer_y0 = -outer_h / 2.0;
                let outer_path = format!(
                    "M{} {} L{} {} L{} {} L{} {}",
                    outer_x0,
                    outer_y0,
                    outer_x0 + outer_w,
                    outer_y0,
                    outer_x0 + outer_w,
                    outer_y0 + outer_h,
                    outer_x0,
                    outer_y0 + outer_h
                );
                let _ = write!(
                    out,
                    r#"<g><path d="{}" stroke="none" stroke-width="0" fill="none"/></g>"#,
                    escape_xml_display(&outer_path)
                );

                let img_translate_y = if top_label {
                    outer_h / 2.0 - image_height
                } else {
                    -outer_h / 2.0
                };
                let _ = write!(
                    out,
                    r#"<image href="{}" width="{}" height="{}" preserveAspectRatio="none" transform="translate({},{})"/>"#,
                    escape_xml_display(img_href),
                    fmt_display(image_width),
                    fmt_display(image_height),
                    fmt_display(-image_width / 2.0),
                    fmt_display(img_translate_y)
                );

                out.push_str("</g>");
                if wrapped_in_a {
                    out.push_str("</a>");
                }
                return;
            } else {
                // Fall back to a normal node if the image URL is missing.
                let w = layout_node.width.max(1.0);
                let h = layout_node.height.max(1.0);
                let _ = write!(
                    out,
                    r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}"/>"#,
                    escape_xml_display(&style),
                    fmt_display(-w / 2.0),
                    fmt_display(-h / 2.0),
                    fmt_display(w),
                    fmt_display(h)
                );
                // Keep default label rendering.
            }
        }
        "iconSquare" => {
            // Port of Mermaid `iconSquare.ts` (`icon-shape default`).
            if let Some(_icon_name) = node_icon.filter(|s| !s.trim().is_empty()) {
                let label_text_plain =
                    flowchart_label_plain_text(label_text, label_type, ctx.node_html_labels);
                let has_label = !label_text_plain.trim().is_empty();
                let label_padding = if has_label { 8.0 } else { 0.0 };
                let top_label = node_pos == Some("t");

                let asset_h = node_asset_height.unwrap_or(48.0).max(1.0);
                let asset_w = node_asset_width.unwrap_or(48.0).max(1.0);
                let icon_size = asset_h.max(asset_w);

                let half_padding = ctx.node_padding / 2.0;
                let height = icon_size + half_padding * 2.0;
                let width = icon_size + half_padding * 2.0;
                let x = -width / 2.0;
                let y = -height / 2.0;

                let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                    ctx.measurer,
                    label_text,
                    label_type,
                    &ctx.text_style,
                    Some(ctx.wrapping_width),
                    ctx.node_wrap_mode,
                );
                if !has_label {
                    metrics.width = 0.0;
                    metrics.height = 0.0;
                }

                // Mermaid's `labelHelper(...)` wraps icon labels in `.labelBkg` (2px padding).
                let label_bbox_w = metrics.width + if has_label { 4.0 } else { 0.0 };
                let label_bbox_h = metrics.height + if has_label { 4.0 } else { 0.0 };

                let outer_w = width.max(label_bbox_w);
                let outer_h = height + label_bbox_h + label_padding;

                fn rounded_rect_path_d(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
                    // Mermaid `roundedRectPath.ts`.
                    format!(
                        "M {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} H {} A {} {} 0 0 1 {} {} V {} A {} {} 0 0 1 {} {} Z",
                        x + r,
                        y,
                        x + w - r,
                        r,
                        r,
                        x + w,
                        y + r,
                        y + h - r,
                        r,
                        r,
                        x + w - r,
                        y + h,
                        x + r,
                        r,
                        r,
                        x,
                        y + h - r,
                        y + r,
                        r,
                        r,
                        x + r,
                        y,
                    )
                }

                // Mermaid sets `options.stroke = fill ?? mainBkg` for iconSquare, so the outline
                // stroke matches the fill color (not the node border color).
                let icon_path = rounded_rect_path_d(x, y, width, height, 0.1);
                if let Some((fill_d, stroke_d)) = roughjs_paths_for_svg_path(
                    &icon_path,
                    fill_color,
                    fill_color,
                    stroke_width,
                    stroke_dasharray,
                    hand_drawn_seed,
                ) {
                    let icon_dy = if top_label {
                        label_bbox_h / 2.0 + label_padding / 2.0
                    } else {
                        -label_bbox_h / 2.0 - label_padding / 2.0
                    };

                    // Mermaid uses `translate(0,18)` without a space after the comma.
                    let _ = write!(out, r#"<g transform="translate(0,{})">"#, fmt(icon_dy));
                    let _ = write!(
                        out,
                        r#"<path d="{}" stroke="none" stroke-width="0" fill="{}"/>"#,
                        escape_attr(&fill_d),
                        escape_attr(fill_color)
                    );
                    let _ = write!(
                        out,
                        r#"<path d="{}" stroke="{}" stroke-width="{}" fill="none" stroke-dasharray="{}"/>"#,
                        escape_attr(&stroke_d),
                        escape_attr(fill_color),
                        fmt(stroke_width as f64),
                        escape_attr(stroke_dasharray)
                    );
                    out.push_str("</g>");
                }

                let label_html = flowchart_label_html(&label_text, &label_type, &ctx.config);
                let label_y = if top_label {
                    -outer_h / 2.0
                } else {
                    outer_h / 2.0 - label_bbox_h
                };
                let _ = write!(
                    out,
                    r#"<g class="label" style="" transform="translate({},{})"><rect/><foreignObject width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" class="labelBkg" style="display: table-cell; white-space: nowrap; line-height: 1.5; max-width: 200px; text-align: center;"><span class="nodeLabel">{}</span></div></foreignObject></g>"#,
                    fmt(-label_bbox_w / 2.0),
                    fmt(label_y),
                    fmt(label_bbox_w),
                    fmt(label_bbox_h),
                    label_html
                );

                // Outer bbox helper node (transparent fill, no stroke).
                let outer_x0 = -outer_w / 2.0;
                let outer_y0 = -outer_h / 2.0;
                let outer_path = format!(
                    "M{} {} L{} {} L{} {} L{} {}",
                    fmt(outer_x0),
                    fmt(outer_y0),
                    fmt(outer_x0 + outer_w),
                    fmt(outer_y0),
                    fmt(outer_x0 + outer_w),
                    fmt(outer_y0 + outer_h),
                    fmt(outer_x0),
                    fmt(outer_y0 + outer_h)
                );
                let _ = write!(
                    out,
                    r#"<g><path d="{}" stroke="none" stroke-width="0" fill="transparent"/></g>"#,
                    escape_attr(&outer_path)
                );

                // Mermaid CLI baseline at 11.12.2 renders Font Awesome icons via a browser-loaded
                // icon set. In our baselines, the upstream renderer falls back to a placeholder
                // icon SVG (a blue square with a `?`). Mirror that placeholder output here.
                let icon_tx = -icon_size / 2.0;
                let icon_ty = if top_label {
                    label_bbox_h / 2.0 + label_padding / 2.0 - icon_size / 2.0
                } else {
                    -label_bbox_h / 2.0 - label_padding / 2.0 - icon_size / 2.0
                };
                let _ = write!(
                    out,
                    r#"<g transform="translate({},{})" style="color: {};"><g><svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}" viewBox="0 0 80 80"><g><rect width="80" height="80" style="fill: #087ebf; stroke-width: 0px;"/><text transform="translate(21.16 64.67)" style="fill: #fff; font-family: ArialMT, Arial; font-size: 67.75px;"><tspan x="0" y="0">?</tspan></text></g></svg></g></g>"#,
                    fmt(icon_tx),
                    fmt(icon_ty),
                    escape_attr(stroke_color),
                    fmt(icon_size),
                    fmt(icon_size),
                );

                out.push_str("</g>");
                if wrapped_in_a {
                    out.push_str("</a>");
                }
                return;
            } else {
                // Fall back to a normal node if the icon name is missing.
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
            // Mermaid `cylinder.ts` (non-handDrawn): a single `<path>` with arc commands and a
            // `label-offset-y` attribute.
            let w = layout_node.width.max(1.0);
            let rx = w / 2.0;
            let ry = rx / (2.5 + w / 50.0);
            let total_h = layout_node.height.max(1.0);
            let h = (total_h - 2.0 * ry).max(1.0);
            // Mermaid applies an extra downward label shift of `node.padding / 1.5`.
            label_dy = ctx.node_padding / 1.5;

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
                escape_attr(&style),
                fmt(-w / 2.0),
                fmt(-(h / 2.0 + ry))
            );
        }
        "h-cyl" | "das" | "horizontal-cylinder" => {
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
            label_dx = -rx;

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
                escape_attr(&style),
                fmt(-w / 2.0),
                fmt(h / 2.0),
            );
        }
        "win-pane" | "internal-storage" | "window-pane" => {
            // Mermaid `windowPane.ts` (non-handDrawn): RoughJS multi-subpath with `roughness=0` + a
            // fixed `rectOffset=5` and a translation of `(+2.5, +2.5)`.
            let rect_offset = 5.0;
            let out_w = layout_node.width.max(1.0);
            let out_h = layout_node.height.max(1.0);
            let w = (out_w - rect_offset).max(1.0);
            let h = (out_h - rect_offset).max(1.0);
            let x = -w / 2.0;
            let y = -h / 2.0;

            // Label transform includes the same `rectOffset/2` shift as the container.
            label_dx = rect_offset / 2.0;
            label_dy = rect_offset / 2.0;

            let path_data = format!(
                "M{},{} L{},{} L{},{} L{},{} L{},{} M{},{} L{},{} M{},{} L{},{}",
                fmt(x - rect_offset),
                fmt(y - rect_offset),
                fmt(x + w),
                fmt(y - rect_offset),
                fmt(x + w),
                fmt(y + h),
                fmt(x - rect_offset),
                fmt(y + h),
                fmt(x - rect_offset),
                fmt(y - rect_offset),
                fmt(x - rect_offset),
                fmt(y),
                fmt(x + w),
                fmt(y),
                fmt(x),
                fmt(y - rect_offset),
                fmt(x),
                fmt(y + h),
            );

            if let Some((fill_d, stroke_d)) = roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            ) {
                let _ = write!(
                    out,
                    r#"<g transform="translate({}, {})" class="basic label-container">"#,
                    fmt(rect_offset / 2.0),
                    fmt(rect_offset / 2.0)
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
                    fmt(stroke_width as f64),
                    escape_attr(stroke_dasharray),
                    escape_attr(&style)
                );
                out.push_str("</g>");
            }
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
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let radius = 5.0;
            let taper = 5.0;

            let mut pts: Vec<(f64, f64)> = Vec::new();
            pts.push((-w / 2.0 + taper, -h / 2.0));
            pts.push((w / 2.0 - taper, -h / 2.0));
            pts.extend(arc_points(
                w / 2.0 - taper,
                -h / 2.0,
                w / 2.0,
                -h / 2.0 + taper,
                radius,
                radius,
                true,
            ));
            pts.push((w / 2.0, -h / 2.0 + taper));
            pts.push((w / 2.0, h / 2.0 - taper));
            pts.extend(arc_points(
                w / 2.0,
                h / 2.0 - taper,
                w / 2.0 - taper,
                h / 2.0,
                radius,
                radius,
                true,
            ));
            pts.push((w / 2.0 - taper, h / 2.0));
            pts.push((-w / 2.0 + taper, h / 2.0));
            pts.extend(arc_points(
                -w / 2.0 + taper,
                h / 2.0,
                -w / 2.0,
                h / 2.0 - taper,
                radius,
                radius,
                true,
            ));
            pts.push((-w / 2.0, h / 2.0 - taper));
            pts.push((-w / 2.0, -h / 2.0 + taper));
            pts.extend(arc_points(
                -w / 2.0,
                -h / 2.0 + taper,
                -w / 2.0 + taper,
                -h / 2.0,
                radius,
                radius,
                true,
            ));
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                out.push_str(r#"<g class="basic label-container outer-path">"#);
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
                    r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="5" ry="5"/>"#,
                    escape_attr(&style),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0),
                    fmt(w),
                    fmt(h)
                );
            }
        }
        "stadium" => {
            // Port of Mermaid `@11.12.2` `stadium.ts` points + `createPathFromPoints`.
            // Note that Mermaid's `generateCirclePoints()` pushes negated coordinates.
            fn generate_circle_points(
                center_x: f64,
                center_y: f64,
                radius: f64,
                num_points: usize,
                start_angle_deg: f64,
                end_angle_deg: f64,
            ) -> Vec<(f64, f64)> {
                let start = start_angle_deg.to_radians();
                let end = end_angle_deg.to_radians();
                let angle_range = end - start;
                let step = angle_range / (num_points.saturating_sub(1).max(1) as f64);
                let mut pts: Vec<(f64, f64)> = Vec::with_capacity(num_points);
                for i in 0..num_points {
                    let angle = start + (i as f64) * step;
                    let x = center_x + radius * angle.cos();
                    let y = center_y + radius * angle.sin();
                    pts.push((-x, -y));
                }
                pts
            }

            // Mermaid flowchart-v2 updates `node.width/height` from the rendered rough path bbox
            // (`updateNodeBounds`) before running Dagre layout. That bbox is narrower than the
            // theoretical `(text bbox + padding)` width used to generate the stadium points. The
            // SVG path is still generated from the theoretical width, so we recompute it here.
            let mut metrics = crate::flowchart::flowchart_label_metrics_for_layout(
                ctx.measurer,
                &label_text,
                &label_type,
                &ctx.text_style,
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
                    &ctx.text_style,
                );
            }
            let (render_w, render_h) = crate::flowchart::flowchart_node_render_dimensions(
                Some("stadium"),
                metrics,
                ctx.node_padding,
            );

            let w = render_w.max(1.0);
            let h = render_h.max(1.0);
            let radius = h / 2.0;

            let mut pts: Vec<(f64, f64)> = Vec::new();
            pts.push((-w / 2.0 + radius, -h / 2.0));
            pts.push((w / 2.0 - radius, -h / 2.0));
            pts.extend(generate_circle_points(
                -w / 2.0 + radius,
                0.0,
                radius,
                50,
                90.0,
                270.0,
            ));
            pts.push((w / 2.0 - radius, h / 2.0));
            pts.extend(generate_circle_points(
                w / 2.0 - radius,
                0.0,
                radius,
                50,
                270.0,
                450.0,
            ));
            let path_data = path_from_points(&pts);

            if let Some((fill_d, stroke_d)) = rough_timed!(roughjs_paths_for_svg_path(
                &path_data,
                fill_color,
                stroke_color,
                stroke_width,
                stroke_dasharray,
                hand_drawn_seed,
            )) {
                out.push_str(r#"<g class="basic label-container outer-path">"#);
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
                    r#"<rect class="basic label-container" style="{}" x="{}" y="{}" width="{}" height="{}" rx="{}" ry="{}"/>"#,
                    escape_attr(&style),
                    fmt(-w / 2.0),
                    fmt(-h / 2.0),
                    fmt(w),
                    fmt(h),
                    fmt(radius),
                    fmt(radius)
                );
            }
        }
        "hexagon" | "hex" => {
            let w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let half_width = w / 2.0;
            let half_height = h / 2.0;
            let fixed_length = half_height / 2.0;
            let deduced_width = half_width - fixed_length;

            let pts: Vec<(f64, f64)> = vec![
                (-deduced_width, -half_height),
                (0.0, -half_height),
                (deduced_width, -half_height),
                (half_width, 0.0),
                (deduced_width, half_height),
                (0.0, half_height),
                (-deduced_width, half_height),
                (-half_width, 0.0),
            ];
            let path_data = path_from_points(&pts);

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
            } else {
                let _ = write!(
                    out,
                    r#"<polygon points="{},{} {},{} {},{} {},{} {},{} {},{} {},{} {},{}" class="label-container" transform="translate({}, {})"{} />"#,
                    fmt_display(-deduced_width),
                    fmt_display(-half_height),
                    fmt_display(0.0),
                    fmt_display(-half_height),
                    fmt_display(deduced_width),
                    fmt_display(-half_height),
                    fmt_display(half_width),
                    fmt_display(0.0),
                    fmt_display(deduced_width),
                    fmt_display(half_height),
                    fmt_display(0.0),
                    fmt_display(half_height),
                    fmt_display(-deduced_width),
                    fmt_display(half_height),
                    fmt_display(-half_width),
                    fmt_display(0.0),
                    fmt_display(0.0),
                    fmt_display(0.0),
                    OptionalStyleAttr(style.as_str())
                );
            }
        }
        "lean_right" | "lean-r" | "lean-right" => {
            // Mermaid `leanRight.ts` (non-handDrawn): polygon via `insertPolygonShape(...)`.
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            let pts: Vec<(f64, f64)> = vec![(-dx, 0.0), (w, 0.0), (w + dx, -h), (0.0, -h)];
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
        "lean_left" | "lean-l" | "lean-left" => {
            // Mermaid `leanLeft.ts` (non-handDrawn): polygon via `insertPolygonShape(...)`.
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (w + dx, 0.0), (w, -h), (-dx, -h)];
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
        "trapezoid" | "trap-b" => {
            // Mermaid `trapezoid.ts` (non-handDrawn): polygon via `insertPolygonShape(...)`.
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            let pts: Vec<(f64, f64)> = vec![(-dx, 0.0), (w + dx, 0.0), (w, -h), (0.0, -h)];
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
        "inv_trapezoid" | "inv-trapezoid" | "trap-t" => {
            // Mermaid `invertedTrapezoid.ts` (non-handDrawn): polygon via `insertPolygonShape(...)`.
            let total_w = layout_node.width.max(1.0);
            let h = layout_node.height.max(1.0);
            let w = (total_w - h).max(1.0);
            let dx = (3.0 * h) / 6.0;
            let pts: Vec<(f64, f64)> = vec![(0.0, 0.0), (w, 0.0), (w + dx, -h), (-dx, -h)];
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
