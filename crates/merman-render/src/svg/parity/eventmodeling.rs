use super::*;

const BOX_TEXT_LINE_HEIGHT: f64 = 20.0;

pub(super) fn render_eventmodeling_diagram_svg(
    layout: &EventModelingDiagramLayout,
    _semantic: &serde_json::Value,
    effective_config: &serde_json::Value,
    options: &SvgRenderOptions,
) -> Result<String> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("eventmodeling");
    let viewbox_attr = format!(
        "{} {} {} {}",
        fmt(layout.viewbox_x),
        fmt(layout.viewbox_y),
        fmt(layout.total_width),
        fmt(layout.total_height)
    );
    let fixed_width = fmt_string(layout.total_width);
    let fixed_height = fmt_string(layout.total_height);
    let max_width = fmt_string(layout.total_width);
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
    let marker_id = format!("eventmodeling-arrow-{diagram_id}");
    let _ = write!(&mut out, "<style>{css}</style>");
    let _ = write!(&mut out, r#"<g class="eventmodeling"><defs><marker id=""#);
    escape_xml_into(&mut out, &marker_id);
    out.push_str(
        r#"" viewBox="0 0 10 10" refX="9" refY="5" markerWidth="7" markerHeight="7" orient="auto"><path d="M 0 0 L 10 5 L 0 10 Z" class="eventModeling-arrow"></path></marker></defs>"#,
    );

    for swimlane in &layout.swimlanes {
        let _ = write!(
            &mut out,
            r#"<g class="eventModeling-swimlane" data-index="{}"><rect class="eventModeling-swimlane-bg" x="{}" y="{}" width="{}" height="{}"></rect><text class="eventModeling-swimlane-label" x="{}" y="{}">"#,
            swimlane.index,
            fmt(swimlane.x),
            fmt(swimlane.y),
            fmt(swimlane.width),
            fmt(swimlane.height),
            fmt(swimlane.x + 16.0),
            fmt(swimlane.y + swimlane.height / 2.0)
        );
        escape_xml_into(&mut out, &swimlane.label);
        out.push_str("</text></g>");
    }

    for relation in &layout.relations {
        let _ = write!(
            &mut out,
            r#"<line class="eventModeling-relation" data-source="{}" data-target="{}" x1="{}" y1="{}" x2="{}" y2="{}" stroke="{}" marker-end="url(#{})"></line>"#,
            escape_attr_display(&relation.source_frame),
            escape_attr_display(&relation.target_frame),
            fmt(relation.x1),
            fmt(relation.y1),
            fmt(relation.x2),
            fmt(relation.y2),
            escape_attr_display(&relation.stroke),
            escape_attr_display(&marker_id)
        );
    }

    for box_layout in &layout.boxes {
        let class_name = if box_layout.frame_kind == "resetframe" {
            "eventModeling-box eventModeling-reset-box"
        } else {
            "eventModeling-box"
        };
        let _ = write!(
            &mut out,
            r#"<g class="eventModeling-frame" data-frame="{}" data-entity-type="{}"><rect class="{}" x="{}" y="{}" width="{}" height="{}" fill="{}" stroke="{}"></rect>"#,
            escape_attr_display(&box_layout.frame_name),
            escape_attr_display(&box_layout.model_entity_type),
            class_name,
            fmt(box_layout.x),
            fmt(box_layout.y),
            fmt(box_layout.width),
            fmt(box_layout.height),
            escape_attr_display(&box_layout.fill),
            escape_attr_display(&box_layout.stroke)
        );
        push_box_text(&mut out, box_layout);
        out.push_str("</g>");
    }

    out.push_str("</g></svg>\n");
    Ok(out)
}

fn push_box_text(out: &mut String, box_layout: &crate::model::EventModelingBoxLayout) {
    let lines: Vec<&str> = box_layout.text.lines().collect();
    let line_count = lines.len().max(1);
    let x = box_layout.x + box_layout.width / 2.0;
    let first_y = box_layout.y + box_layout.height / 2.0
        - ((line_count.saturating_sub(1)) as f64 * BOX_TEXT_LINE_HEIGHT) / 2.0;
    let _ = write!(
        out,
        r#"<text class="eventModeling-box-text" x="{}" y="{}">"#,
        fmt(x),
        fmt(first_y)
    );
    if lines.is_empty() {
        escape_xml_into(out, &box_layout.entity_identifier);
    } else {
        for (idx, line) in lines.iter().enumerate() {
            let _ = write!(
                out,
                r#"<tspan x="{}" dy="{}">"#,
                fmt(x),
                if idx == 0 {
                    "0".to_string()
                } else {
                    fmt_string(BOX_TEXT_LINE_HEIGHT)
                }
            );
            escape_xml_into(out, line);
            out.push_str("</tspan>");
        }
    }
    out.push_str("</text>");
}

fn eventmodeling_css(effective_config: &serde_json::Value) -> String {
    let font_family = config_string(effective_config, &["fontFamily"])
        .or_else(|| config_string(effective_config, &["themeVariables", "fontFamily"]))
        .map(|s| normalize_css_font_family(&s))
        .unwrap_or_else(|| "trebuchet ms, verdana, arial, sans-serif".to_string());
    let text_color = config_string(effective_config, &["themeVariables", "textColor"])
        .unwrap_or_else(|| "#333".to_string());
    let swimlane_fill = config_string(
        effective_config,
        &["themeVariables", "emSwimlaneBackgroundOdd"],
    )
    .or_else(|| {
        config_string(
            effective_config,
            &["themeVariables", "emSwimlaneBackground"],
        )
    })
    .unwrap_or_else(|| "rgb(250,250,250)".to_string());
    let swimlane_stroke = config_string(
        effective_config,
        &["themeVariables", "emSwimlaneBackgroundStroke"],
    )
    .or_else(|| config_string(effective_config, &["themeVariables", "emSwimlaneBorder"]))
    .unwrap_or_else(|| "rgb(240,240,240)".to_string());
    let label_color = config_string(
        effective_config,
        &["themeVariables", "emSwimlaneLabelColor"],
    )
    .unwrap_or_else(|| text_color.clone());
    let arrow_fill = config_string(effective_config, &["themeVariables", "emArrowhead"])
        .or_else(|| config_string(effective_config, &["themeVariables", "emRelationStroke"]))
        .unwrap_or_else(|| "#000".to_string());

    format!(
        ".eventmodeling text {{ font-family: {font_family}; fill: {text_color}; }}\
.eventmodeling .eventModeling-swimlane-bg {{ fill: {swimlane_fill}; stroke: {swimlane_stroke}; stroke-width: 1; }}\
.eventmodeling .eventModeling-swimlane-label {{ fill: {label_color}; font-size: 14px; font-weight: 600; dominant-baseline: middle; }}\
.eventmodeling .eventModeling-box {{ stroke-width: 2; rx: 4; ry: 4; }}\
.eventmodeling .eventModeling-reset-box {{ stroke-dasharray: 5 3; }}\
.eventmodeling .eventModeling-box-text {{ font-size: 16px; font-weight: 700; text-anchor: middle; dominant-baseline: middle; }}\
.eventmodeling .eventModeling-relation {{ stroke-width: 2; fill: none; }}\
.eventmodeling .eventModeling-arrow {{ fill: {arrow_fill}; }}"
    )
}
