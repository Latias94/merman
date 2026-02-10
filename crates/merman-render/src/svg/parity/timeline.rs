use super::*;

fn timeline_css(diagram_id: &str, effective_config: &serde_json::Value) -> String {
    let id = escape_xml(diagram_id);
    let font = r#""trebuchet ms",verdana,arial,sans-serif"#;

    fn default_c_scale(i: usize) -> &'static str {
        match i {
            0 => "hsl(240, 100%, 76.2745098039%)",
            1 => "hsl(60, 100%, 73.5294117647%)",
            2 => "hsl(80, 100%, 76.2745098039%)",
            3 => "hsl(270, 100%, 76.2745098039%)",
            4 => "hsl(300, 100%, 76.2745098039%)",
            5 => "hsl(330, 100%, 76.2745098039%)",
            6 => "hsl(0, 100%, 76.2745098039%)",
            7 => "hsl(30, 100%, 76.2745098039%)",
            8 => "hsl(90, 100%, 76.2745098039%)",
            9 => "hsl(150, 100%, 76.2745098039%)",
            10 => "hsl(180, 100%, 76.2745098039%)",
            _ => "hsl(210, 100%, 76.2745098039%)",
        }
    }

    fn round_1e10(v: f64) -> f64 {
        let v = (v * 1e10).round() / 1e10;
        if v == -0.0 { 0.0 } else { v }
    }

    fn invert_css_color_to_hex(color: &str) -> Option<String> {
        let color = color.trim();
        if color.is_empty() {
            return None;
        }
        if color.eq_ignore_ascii_case("black") {
            return Some("#ffffff".to_string());
        }
        if color.eq_ignore_ascii_case("white") {
            return Some("#000000".to_string());
        }
        if let Some(hex) = color.strip_prefix('#') {
            let hex = hex.trim();
            let (r, g, b) = match hex.len() {
                3 => {
                    let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
                    let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
                    let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
                    (r, g, b)
                }
                6 => {
                    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                    (r, g, b)
                }
                _ => return None,
            };
            return Some(format!("#{:02x}{:02x}{:02x}", 255 - r, 255 - g, 255 - b));
        }
        None
    }

    fn parse_hsl(s: &str) -> Option<(f64, f64, f64)> {
        let inner = s.trim().strip_prefix("hsl(")?.strip_suffix(')')?;
        let mut parts = inner.split(',').map(|p| p.trim());
        let h = parts.next()?.parse::<f64>().ok()?;
        let s = parts
            .next()?
            .strip_suffix('%')?
            .trim()
            .parse::<f64>()
            .ok()?;
        let l = parts
            .next()?
            .strip_suffix('%')?
            .trim()
            .parse::<f64>()
            .ok()?;
        Some((h, s, l))
    }

    fn fmt_hsl(h: f64, s: f64, l: f64, buf: &mut ryu_js::Buffer) -> String {
        let h = buf.format_finite(round_1e10(h)).to_string();
        let s = buf.format_finite(round_1e10(s)).to_string();
        let l = buf.format_finite(round_1e10(l)).to_string();
        format!("hsl({h}, {s}%, {l}%)")
    }

    fn derive_c_scale_inv_fallback(c_scale: &str, buf: &mut ryu_js::Buffer) -> Option<String> {
        let (h, s, l) = parse_hsl(c_scale)?;
        let h = (h + 180.0) % 360.0;
        let l = (l + 10.0).clamp(0.0, 100.0);
        Some(fmt_hsl(h, s, l, buf))
    }

    // Keep `:root` last (matches upstream Mermaid timeline SVG baselines).
    let root_rule = format!(r#"#{} :root{{--mermaid-font-family:{};}}"#, id, font);
    let mut out = info_css(diagram_id);
    if let Some(prefix) = out.strip_suffix(&root_rule) {
        out = prefix.to_string();
    }

    let label_text_color = theme_color(effective_config, "labelTextColor", "black");
    let label_text_is_calculated = label_text_color.trim() == "calculated";
    let scale_label_color = theme_color(effective_config, "scaleLabelColor", &label_text_color);
    let mut buf = ryu_js::Buffer::new();

    let _ = write!(&mut out, r#"#{} .edge{{stroke-width:3;}}"#, id);
    for i in 0..12usize {
        let section = i as i64 - 1;
        let c_scale = theme_color(effective_config, &format!("cScale{i}"), default_c_scale(i));
        let c_scale_label = config_string(
            effective_config,
            &["themeVariables", &format!("cScaleLabel{i}")],
        )
        .unwrap_or_else(|| {
            if label_text_is_calculated {
                scale_label_color.clone()
            } else if i == 0 || i == 3 {
                invert_css_color_to_hex(&label_text_color)
                    .unwrap_or_else(|| label_text_color.clone())
            } else {
                label_text_color.clone()
            }
        });
        let c_scale_inv = config_string(
            effective_config,
            &["themeVariables", &format!("cScaleInv{i}")],
        )
        .or_else(|| derive_c_scale_inv_fallback(&c_scale, &mut buf))
        .unwrap_or_else(|| c_scale.clone());
        let sw = 17 - 3 * (i as i64);

        let _ = write!(
            &mut out,
            r#"#{} .section-{} rect,#{} .section-{} path,#{} .section-{} circle,#{} .section-{} path{{fill:{};}}#{} .section-{} text{{fill:{};}}#{} .node-icon-{}{{font-size:40px;color:{};}}#{} .section-edge-{}{{stroke:{};}}#{} .edge-depth-{}{{stroke-width:{};}}#{} .section-{} line{{stroke:{};stroke-width:3;}}#{} .lineWrapper line{{stroke:{};}}#{} .disabled,#{} .disabled circle,#{} .disabled text{{fill:lightgray;}}#{} .disabled text{{fill:#efefef;}}"#,
            id,
            section,
            id,
            section,
            id,
            section,
            id,
            section,
            c_scale,
            id,
            section,
            c_scale_label,
            id,
            section,
            c_scale_label,
            id,
            section,
            c_scale,
            id,
            section,
            sw,
            id,
            section,
            c_scale_inv,
            id,
            c_scale_label,
            id,
            id,
            id,
            id,
        );
    }

    let git0 = theme_color(effective_config, "git0", "hsl(240, 100%, 46.2745098039%)");
    let git_branch_label0 = theme_color(effective_config, "gitBranchLabel0", "#ffffff");
    let _ = write!(
        &mut out,
        r#"#{} .section-root rect,#{} .section-root path,#{} .section-root circle{{fill:{};}}#{} .section-root text{{fill:{};}}#{} .icon-container{{height:100%;display:flex;justify-content:center;align-items:center;}}#{} .edge{{fill:none;}}#{} .eventWrapper{{filter:brightness(120%);}}"#,
        id, id, id, git0, id, git_branch_label0, id, id, id
    );

    out.push_str(&root_rule);
    out
}

pub(super) fn render_timeline_diagram_svg(
    layout: &TimelineDiagramLayout,
    semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    _diagram_title: Option<&str>,
    _measurer: &dyn TextMeasurer,
    options: &SvgRenderOptions,
) -> Result<String> {
    let _ = (semantic, effective_config);

    let diagram_id = options.diagram_id.as_deref().unwrap_or("merman");
    let diagram_id_esc = escape_xml(diagram_id);

    let bounds = layout.bounds.clone().unwrap_or(Bounds {
        min_x: 0.0,
        min_y: 0.0,
        max_x: 100.0,
        max_y: 100.0,
    });
    // Mermaid's root viewport is derived from browser `getBBox()` values, which frequently land on
    // a single-precision lattice. Mirror that by quantizing extrema to `f32`, then computing
    // width/height in `f32` space.
    let min_x_f32 = bounds.min_x as f32;
    let min_y_f32 = bounds.min_y as f32;
    let max_x_f32 = bounds.max_x as f32;
    let max_y_f32 = bounds.max_y as f32;

    let vb_min_x = min_x_f32 as f64;
    let vb_min_y = min_y_f32 as f64;
    let vb_w = ((max_x_f32 - min_x_f32).max(1.0)) as f64;
    let vb_h = ((max_y_f32 - min_y_f32).max(1.0)) as f64;

    fn node_line_class(section_class: &str) -> String {
        let rest = section_class
            .strip_prefix("section-")
            .unwrap_or(section_class);
        format!("node-line-{rest}")
    }

    fn render_node(out: &mut String, n: &crate::model::TimelineNodeLayout) {
        let w = n.width.max(1.0);
        let h = n.height.max(1.0);
        let rd = 5.0;
        let d = format!(
            "M0 {y0} v{v1} q0,-5 5,-5 h{hw} q5,0 5,5 v{v2} H0 Z",
            y0 = fmt(h - rd),
            v1 = fmt(-h + 2.0 * rd),
            hw = fmt(w - 2.0 * rd),
            v2 = fmt(h - rd),
        );

        let _ = write!(
            out,
            r#"<g class="timeline-node {section_class}">"#,
            section_class = escape_attr(&n.section_class)
        );
        out.push_str("<g>");
        let _ = write!(
            out,
            r#"<path id="node-undefined" class="node-bkg node-undefined" d="{d}"/>"#,
            d = escape_attr(&d)
        );
        let _ = write!(
            out,
            r#"<line class="{line_class}" x1="0" y1="{y}" x2="{x2}" y2="{y}"/>"#,
            line_class = escape_attr(&node_line_class(&n.section_class)),
            y = fmt(h),
            x2 = fmt(w)
        );
        out.push_str("</g>");

        let tx = w / 2.0;
        let ty = n.padding / 2.0;
        let _ = write!(
            out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(tx),
            y = fmt(ty)
        );
        out.push_str(r#"<text dy="1em" alignment-baseline="middle" dominant-baseline="middle" text-anchor="middle">"#);
        for (idx, line) in n.label_lines.iter().enumerate() {
            let dy = if idx == 0 { "1em" } else { "1.1em" };
            let _ = write!(
                out,
                r#"<tspan x="0" dy="{dy}">{text}</tspan>"#,
                dy = dy,
                text = escape_xml(line)
            );
        }
        out.push_str("</text></g></g>");
    }

    let mut max_w_attr = fmt_max_width_px(vb_w);
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(vb_min_x),
        fmt(vb_min_y),
        fmt(vb_w),
        fmt(vb_h)
    );
    if let Some((viewbox, max_w)) =
        crate::generated::timeline_root_overrides_11_12_2::lookup_timeline_root_viewport_override(
            diagram_id,
        )
    {
        viewbox_attr = viewbox.to_string();
        max_w_attr = max_w.to_string();
    }

    let mut out = String::new();
    let _ = write!(
        &mut out,
        r#"<svg id="{diagram_id_esc}" width="100%" xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" style="max-width: {max_w}px; background-color: white;" viewBox="{viewbox}" role="graphics-document document" aria-roledescription="timeline">"#,
        diagram_id_esc = diagram_id_esc,
        max_w = max_w_attr,
        viewbox = viewbox_attr,
    );
    let css = timeline_css(diagram_id, effective_config);
    let _ = write!(&mut out, r#"<style>{}</style>"#, css);
    out.push_str(r#"<g/>"#);
    out.push_str(r#"<g/>"#);
    out.push_str(
        r#"<defs><marker id="arrowhead" refX="5" refY="2" markerWidth="6" markerHeight="4" orient="auto"><path d="M 0,0 V 4 L6,2 Z"/></marker></defs>"#,
    );

    for section in &layout.sections {
        let node = &section.node;
        let _ = write!(
            &mut out,
            r#"<g transform="translate({x}, {y})">"#,
            x = fmt(node.x),
            y = fmt(node.y)
        );
        render_node(&mut out, node);
        out.push_str("</g>");

        for task in &section.tasks {
            let task_node = &task.node;
            let _ = write!(
                &mut out,
                r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
                x = fmt(task_node.x),
                y = fmt(task_node.y)
            );
            render_node(&mut out, task_node);
            out.push_str("</g>");

            let _ = write!(
                &mut out,
                r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="url(#arrowhead)" stroke-dasharray="5,5"/></g>"#,
                x1 = fmt(task.connector.x1),
                y1 = fmt(task.connector.y1),
                x2 = fmt(task.connector.x2),
                y2 = fmt(task.connector.y2),
            );

            for ev in &task.events {
                let _ = write!(
                    &mut out,
                    r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                    x = fmt(ev.x),
                    y = fmt(ev.y)
                );
                render_node(&mut out, ev);
                out.push_str("</g>");
            }
        }
    }

    for task in &layout.orphan_tasks {
        let task_node = &task.node;
        let _ = write!(
            &mut out,
            r#"<g class="taskWrapper" transform="translate({x}, {y})">"#,
            x = fmt(task_node.x),
            y = fmt(task_node.y)
        );
        render_node(&mut out, task_node);
        out.push_str("</g>");

        let _ = write!(
            &mut out,
            r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="2" stroke="black" marker-end="url(#arrowhead)" stroke-dasharray="5,5"/></g>"#,
            x1 = fmt(task.connector.x1),
            y1 = fmt(task.connector.y1),
            x2 = fmt(task.connector.x2),
            y2 = fmt(task.connector.y2),
        );

        for ev in &task.events {
            let _ = write!(
                &mut out,
                r#"<g class="eventWrapper" transform="translate({x}, {y})">"#,
                x = fmt(ev.x),
                y = fmt(ev.y)
            );
            render_node(&mut out, ev);
            out.push_str("</g>");
        }
    }

    if let Some(title) = layout.title.as_deref().filter(|t| !t.trim().is_empty()) {
        let _ = write!(
            &mut out,
            r#"<text x="{x}" font-size="4ex" font-weight="bold" y="{y}">{text}</text>"#,
            x = fmt(layout.title_x),
            y = fmt(layout.title_y),
            text = escape_xml(title)
        );
    }

    let _ = write!(
        &mut out,
        r#"<g class="lineWrapper"><line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" stroke-width="4" stroke="black" marker-end="url(#arrowhead)"/></g>"#,
        x1 = fmt(layout.activity_line.x1),
        y1 = fmt(layout.activity_line.y1),
        x2 = fmt(layout.activity_line.x2),
        y2 = fmt(layout.activity_line.y2),
    );

    out.push_str("</svg>\n");
    Ok(out)
}
