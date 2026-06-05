use super::*;

const BOX_TEXT_PADDING: f64 = 10.0;

pub(super) fn render_eventmodeling_diagram_svg(
    layout: &EventModelingDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("eventmodeling");
    let mut viewbox_attr = format!(
        "{} {} {} {}",
        fmt(layout.viewbox_x),
        fmt(layout.viewbox_y),
        fmt(layout.total_width),
        fmt(layout.total_height)
    );
    let mut fixed_width = fmt_string(layout.total_width);
    let mut fixed_height = fmt_string(layout.total_height);
    let mut max_width = fmt_string(layout.total_width);
    if options.apply_root_overrides {
        apply_root_viewport_override(
            diagram_id,
            &mut viewbox_attr,
            &mut fixed_width,
            &mut fixed_height,
            &mut max_width,
            crate::generated::eventmodeling_root_overrides_11_15_0::lookup_eventmodeling_root_viewport_override,
        );
    }
    let style_attr = format!("max-width: {max_width}px; background-color: white;");

    let mut out = String::new();
    if layout.use_max_width {
        root_svg::push_svg_root_open(
            &mut out,
            root_svg::SvgRootAttrs {
                width: root_svg::SvgRootWidth::Percent100,
                style_attr: Some(style_attr.as_str()),
                viewbox_attr: Some(viewbox_attr.as_str()),
                trailing_newline: false,
                ..root_svg::SvgRootAttrs::new(diagram_id, "eventmodeling")
            },
        );
    } else {
        root_svg::push_svg_root_open(
            &mut out,
            root_svg::SvgRootAttrs {
                width: root_svg::SvgRootWidth::Fixed(fixed_width.as_str()),
                height_attr: Some(fixed_height.as_str()),
                style_attr: Some("background-color: white;"),
                viewbox_attr: Some(viewbox_attr.as_str()),
                trailing_newline: false,
                ..root_svg::SvgRootAttrs::new(diagram_id, "eventmodeling")
            },
        );
    }

    let css = eventmodeling_css(effective_config);
    let marker_id = format!("em-arrowhead-{diagram_id}");
    let _ = write!(&mut out, "<style>{css}</style>");
    out.push_str("<g/>");

    for swimlane in &layout.swimlanes {
        let _ = write!(
            &mut out,
            r#"<g class="em-swimlane"><rect x="{}" y="{}" rx="3" width="{}" height="{}" fill="{}" stroke="{}"></rect><text font-weight="bold" x="{}" y="{}">"#,
            fmt(swimlane.x),
            fmt(swimlane.y),
            fmt(swimlane.width),
            fmt(swimlane.height),
            escape_attr_display(&swimlane_fill(effective_config)),
            escape_attr_display(&swimlane_stroke(effective_config)),
            fmt(swimlane.x + 30.0),
            fmt(swimlane.y + 30.0)
        );
        escape_xml_into(&mut out, &swimlane.label);
        out.push_str("</text></g>");
    }

    for box_layout in &layout.boxes {
        let _ = write!(
            &mut out,
            r#"<g class="em-box"><rect x="{}" y="{}" rx="3" width="{}" height="{}" stroke="{}" fill="{}"></rect><foreignObject x="{}" y="{}" width="{}" height="{}"><div xmlns="http://www.w3.org/1999/xhtml" style="display: table; height: 100%; width: 100%;"><span style="display: table-cell; text-align: center; vertical-align: middle;">"#,
            fmt(box_layout.x),
            fmt(box_layout.y),
            fmt(box_layout.width),
            fmt(box_layout.height),
            escape_attr_display(&box_layout.stroke),
            escape_attr_display(&box_layout.fill),
            fmt(box_layout.x + BOX_TEXT_PADDING),
            fmt(box_layout.y + BOX_TEXT_PADDING),
            fmt((box_layout.width - 2.0 * BOX_TEXT_PADDING).max(1.0)),
            fmt((box_layout.height - 2.0 * BOX_TEXT_PADDING).max(1.0))
        );
        push_box_html_label(&mut out, &box_layout.text);
        out.push_str("</span></div></foreignObject></g>");
    }

    for relation in &layout.relations {
        let _ = write!(
            &mut out,
            r#"<path class="em-relation" fill="none" stroke="{}" stroke-width="1" marker-end="url(#{})" d="M{} {} L{} {}"></path>"#,
            escape_attr_display(&relation.stroke),
            escape_attr_display(&marker_id),
            fmt(relation.x1),
            fmt(relation.y1),
            fmt(relation.x2),
            fmt(relation.y2)
        );
    }

    let marker_fill = eventmodeling_arrow_fill(effective_config);
    let _ = write!(&mut out, r#"<defs><marker id=""#);
    escape_xml_into(&mut out, &marker_id);
    out.push_str(
        r#"" markerWidth="10" markerHeight="7" refX="10" refY="3.5" orient="auto"><polygon points="0 0, 10 3.5, 0 7" fill=""#,
    );
    escape_xml_into(&mut out, &marker_fill);
    out.push_str(r#""></polygon></marker></defs></svg>"#);
    out.push('\n');
    Ok(out)
}

fn push_box_html_label(out: &mut String, text: &str) {
    let mut lines = text.lines();
    let title = lines.next().unwrap_or(text);
    let rest = lines.collect::<Vec<_>>().join("\n");

    out.push_str("<b>");
    escape_xml_into(out, title);
    out.push_str("</b>");

    let code = normalize_eventmodeling_code_text(&rest);
    if code.is_empty() {
        return;
    }

    out.push_str(r#"<br/><br/><code style="text-align: left; display: block;max-width:430px">"#);
    escape_xml_into(out, &code);
    if code.contains('\n') {
        out.push_str("<br/>");
    }
    out.push_str("</code>");
}

fn normalize_eventmodeling_code_text(raw: &str) -> String {
    let trimmed = raw.trim();
    let without_outer_braces = trimmed
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .unwrap_or(trimmed);
    without_outer_braces.trim().to_string()
}

fn eventmodeling_css(effective_config: &serde_json::Value) -> String {
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| "trebuchet ms, verdana, arial, sans-serif".to_string());
    let text_color = config_string(effective_config, &["themeVariables", "textColor"])
        .unwrap_or_else(|| "#333".to_string());

    format!(
        ".em-swimlane text,.em-box span {{ font-family: {font_family}; color: {text_color}; }}\
.em-relation {{ fill: none; }}"
    )
}

fn swimlane_fill(effective_config: &serde_json::Value) -> String {
    config_string(
        effective_config,
        &["themeVariables", "emSwimlaneBackgroundOdd"],
    )
    .or_else(|| {
        config_string(
            effective_config,
            &["themeVariables", "emSwimlaneBackground"],
        )
    })
    .unwrap_or_else(|| "rgb(250,250,250)".to_string())
}

fn swimlane_stroke(effective_config: &serde_json::Value) -> String {
    config_string(
        effective_config,
        &["themeVariables", "emSwimlaneBackgroundStroke"],
    )
    .or_else(|| config_string(effective_config, &["themeVariables", "emSwimlaneBorder"]))
    .unwrap_or_else(|| "rgb(240,240,240)".to_string())
}

fn eventmodeling_arrow_fill(effective_config: &serde_json::Value) -> String {
    config_string(effective_config, &["themeVariables", "emArrowhead"])
        .or_else(|| config_string(effective_config, &["themeVariables", "emRelationStroke"]))
        .unwrap_or_else(|| "#333333".to_string())
}
