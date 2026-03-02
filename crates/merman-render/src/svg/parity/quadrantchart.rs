use super::*;

// QuadrantChart diagram SVG renderer implementation (split from parity.rs).

pub(super) fn render_quadrantchart_diagram_svg(
    layout: &QuadrantChartDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    fn dominant_baseline(horizontal_pos: &str) -> &'static str {
        if horizontal_pos == "top" {
            "hanging"
        } else {
            "middle"
        }
    }

    fn text_anchor(vertical_pos: &str) -> &'static str {
        if vertical_pos == "left" {
            "start"
        } else {
            "middle"
        }
    }

    fn transform(x: f64, y: f64, rotation: f64) -> String {
        format!(
            "translate({}, {}) rotate({})",
            fmt(x),
            fmt(y),
            fmt(rotation)
        )
    }

    let diagram_id = options.diagram_id.as_deref().unwrap_or("quadrantchart");

    let qc_cfg = effective_config.get("quadrantChart");
    let qc_cfg_missing = qc_cfg.is_none()
        || qc_cfg.is_some_and(|v| v.as_object().is_some_and(|m| m.contains_key("$ref")));
    let use_max_width = if qc_cfg_missing {
        true
    } else {
        config_bool(effective_config, &["quadrantChart", "useMaxWidth"]).unwrap_or(true)
    };

    let mut out = String::new();
    let w = layout.width.max(1.0);
    let h = layout.height.max(1.0);
    let w_attr = fmt(w).to_string();
    let h_attr = fmt(h).to_string();
    let viewbox_attr = format!("0 0 {w_attr} {h_attr}");
    if use_max_width {
        let style_attr = format!("max-width: {w_attr}px; background-color: white;");
        root_svg::push_svg_root_open_ex(
            &mut out,
            diagram_id,
            None,
            root_svg::SvgRootWidth::Percent100,
            None,
            Some(style_attr.as_str()),
            Some(viewbox_attr.as_str()),
            root_svg::SvgRootStyleViewBoxOrder::StyleThenViewBox,
            &[],
            "quadrantChart",
            None,
            None,
            false,
        );
    } else {
        let tail_attrs: [(&str, &str); 1] = [("style", "background-color: white;")];
        root_svg::push_svg_root_open_ex2(
            &mut out,
            diagram_id,
            None,
            root_svg::SvgRootWidth::Fixed(&w_attr),
            Some(&h_attr),
            None,
            Some(viewbox_attr.as_str()),
            root_svg::SvgRootStyleViewBoxOrder::ViewBoxThenStyle,
            &[],
            "quadrantChart",
            None,
            None,
            &tail_attrs,
            root_svg::SvgRootFixedHeightPlacement::AfterXmlns,
            false,
        );
    }

    let _ = write!(
        &mut out,
        r#"<style>{}</style>"#,
        info_css_with_config(diagram_id, effective_config)
    );

    // Mermaid always includes an empty `<g/>` placeholder after `<style>`.
    out.push_str(r#"<g/>"#);

    out.push_str(r#"<g class="main">"#);

    // Quadrants.
    out.push_str(r#"<g class="quadrants">"#);
    for q in &layout.quadrants {
        out.push_str(r#"<g class="quadrant">"#);
        let _ = write!(
            &mut out,
            r#"<rect x="{x}" y="{y}" width="{w}" height="{h}" fill="{fill}"/>"#,
            x = fmt(q.x),
            y = fmt(q.y),
            w = fmt(q.width),
            h = fmt(q.height),
            fill = escape_xml(&q.fill),
        );
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&q.text.fill),
            font_size = fmt(q.text.font_size),
            dom = dominant_baseline(&q.text.horizontal_pos),
            anchor = text_anchor(&q.text.vertical_pos),
            transform = escape_xml(&transform(q.text.x, q.text.y, q.text.rotation)),
            text = escape_xml(&q.text.text),
        );
        out.push_str("</g>");
    }
    out.push_str("</g>");

    // Borders.
    out.push_str(r#"<g class="border">"#);
    for l in &layout.border_lines {
        let _ = write!(
            &mut out,
            r#"<line x1="{x1}" y1="{y1}" x2="{x2}" y2="{y2}" style="stroke: {stroke}; stroke-width: {w};"/>"#,
            x1 = fmt(l.x1),
            y1 = fmt(l.y1),
            x2 = fmt(l.x2),
            y2 = fmt(l.y2),
            stroke = escape_xml(&l.stroke_fill),
            w = fmt(l.stroke_width),
        );
    }
    out.push_str("</g>");

    // Points.
    out.push_str(r#"<g class="data-points">"#);
    for p in &layout.points {
        out.push_str(r#"<g class="data-point">"#);
        let _ = write!(
            &mut out,
            r#"<circle cx="{cx}" cy="{cy}" r="{r}" fill="{fill}" stroke="{stroke}" stroke-width="{stroke_width}"/>"#,
            cx = fmt(p.x),
            cy = fmt(p.y),
            r = fmt(p.radius),
            fill = escape_xml(&p.fill),
            stroke = escape_xml(&p.stroke_color),
            stroke_width = escape_xml(&p.stroke_width),
        );
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&p.text.fill),
            font_size = fmt(p.text.font_size),
            dom = dominant_baseline(&p.text.horizontal_pos),
            anchor = text_anchor(&p.text.vertical_pos),
            transform = escape_xml(&transform(p.text.x, p.text.y, p.text.rotation)),
            text = escape_xml(&p.text.text),
        );
        out.push_str("</g>");
    }
    out.push_str("</g>");

    // Axis labels.
    out.push_str(r#"<g class="labels">"#);
    for t in &layout.axis_labels {
        out.push_str(r#"<g class="label">"#);
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&t.fill),
            font_size = fmt(t.font_size),
            dom = dominant_baseline(&t.horizontal_pos),
            anchor = text_anchor(&t.vertical_pos),
            transform = escape_xml(&transform(t.x, t.y, t.rotation)),
            text = escape_xml(&t.text),
        );
        out.push_str("</g>");
    }
    out.push_str("</g>");

    // Title.
    out.push_str(r#"<g class="title">"#);
    if let Some(t) = layout.title.as_ref() {
        let _ = write!(
            &mut out,
            r#"<text x="0" y="0" fill="{fill}" font-size="{font_size}" dominant-baseline="{dom}" text-anchor="{anchor}" transform="{transform}">{text}</text>"#,
            fill = escape_xml(&t.fill),
            font_size = fmt(t.font_size),
            dom = dominant_baseline(&t.horizontal_pos),
            anchor = text_anchor(&t.vertical_pos),
            transform = escape_xml(&transform(t.x, t.y, t.rotation)),
            text = escape_xml(&t.text),
        );
    }
    out.push_str("</g>");

    out.push_str("</g></svg>\n");
    Ok(out)
}
