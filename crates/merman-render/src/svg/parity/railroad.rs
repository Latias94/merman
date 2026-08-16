use super::*;
use crate::model::RailroadElementLayout;
use merman_core::diagrams::railroad::RailroadDiagramRenderModel;

pub(crate) fn render_railroad_diagram_svg_model(
    layout: &RailroadDiagramLayout,
    model: &RailroadDiagramRenderModel,
    effective_config: &serde_json::Value,
    measurer: &dyn TextMeasurer,
    options: &SvgExecution<'_>,
) -> Result<root_svg::RootedSvg> {
    let diagram_id = options.diagram_id.as_deref().unwrap_or("railroad");
    let diagram_id_esc = escape_xml(diagram_id);
    let acc_title = model
        .acc_title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let acc_descr = model
        .acc_descr
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = root_svg::DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
    let root_spec = root_svg::RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let style = crate::railroad::railroad_style(effective_config);

    let mut out = BoundedSvgOutput::new(options.work_meter());
    let mut root_chrome = root_svg::RootChrome::new(diagram_id, &layout.diagram_type);
    root_chrome.class = Some("railroad-diagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document =
        root_svg::RootViewportContext::new(crate::DiagramFamilyId::RAILROAD, diagram_id)
            .write_open(&mut out, root_spec, root_chrome)?;

    if let Some(title) = acc_title {
        let _ = write!(
            &mut out,
            r#"<title id="chart-title-{}">{}</title>"#,
            diagram_id_esc,
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            &mut out,
            r#"<desc id="chart-desc-{}">{}</desc>"#,
            diagram_id_esc,
            escape_xml_display(descr)
        );
        out.checkpoint()?;
    }
    out.push_str("<style>");
    out.checkpoint()?;
    write_railroad_css(&mut out, &style, diagram_id)?;
    out.push_str("</style>");
    out.checkpoint()?;
    out.push_str("<g/>");
    out.checkpoint()?;

    for (rule_index, rule) in layout.rules.iter().enumerate() {
        let model_rule = model
            .rules
            .get(rule_index)
            .ok_or_else(|| Error::InvalidModel {
                message: format!(
                    "railroad layout contains rule {} without a matching semantic rule",
                    rule.name
                ),
            })?;
        let _ = write!(
            &mut out,
            r#"<g class="railroad-rule" transform="translate({}, {})">"#,
            fmt(rule.x),
            fmt(rule.y)
        );
        out.checkpoint()?;
        let (render_node, definition_up) =
            crate::railroad::railroad_render_node(&model_rule.definition, &style, measurer);
        let _ = write!(
            &mut out,
            r#"<g transform="translate({}, {})">"#,
            fmt(rule.definition_x),
            fmt(rule.baseline_y - definition_up)
        );
        out.checkpoint()?;
        push_render_node(&mut out, &render_node)?;
        out.push_str("</g>");
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<g class="railroad-rule-name-group"><text class="railroad-rule-name" x="0" y="{}">{} =</text></g>"#,
            fmt(rule.baseline_y),
            escape_xml_display(&rule.name)
        );
        out.checkpoint()?;
        let _ = write!(
            &mut out,
            r#"<g class="railroad-start"><circle cx="{}" cy="{}" r="{}"></circle></g><g class="railroad-end"><circle cx="{}" cy="{}" r="{}"></circle></g>"#,
            fmt(rule.start_marker_x),
            fmt(rule.baseline_y),
            fmt(rule.marker_radius),
            fmt(rule.end_marker_x),
            fmt(rule.baseline_y),
            fmt(rule.marker_radius)
        );
        out.checkpoint()?;
        for path in rule.paths.iter().rev().take(2).rev() {
            push_path(&mut out, path)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }

    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

fn push_render_node(
    out: &mut impl SvgOutput,
    node: &crate::railroad::RailroadRenderNode,
) -> Result<()> {
    match node {
        crate::railroad::RailroadRenderNode::Group {
            class,
            transform,
            children,
        } => {
            let _ = write!(out, r#"<g class="{}""#, escape_attr_display(class));
            push_optional_transform(out, *transform);
            out.push('>');
            out.checkpoint()?;
            for child in children {
                push_render_node(out, child)?;
            }
            out.push_str("</g>");
            out.checkpoint()
        }
        crate::railroad::RailroadRenderNode::Element { layout, transform } => {
            push_element(out, layout, *transform)
        }
        crate::railroad::RailroadRenderNode::Path(path) => push_path(out, path),
    }
}

fn push_optional_transform(out: &mut impl SvgOutput, transform: Option<(f64, f64)>) {
    if let Some((x, y)) = transform {
        let _ = write!(out, r#" transform="translate({}, {})""#, fmt(x), fmt(y));
    }
}

fn push_path(out: &mut impl SvgOutput, path: &crate::model::RailroadPathLayout) -> Result<()> {
    out.push_str(r#"<path class="railroad-line""#);
    if path.x != 0.0 || path.y != 0.0 {
        let _ = write!(
            out,
            r#" transform="translate({}, {})""#,
            fmt(path.x),
            fmt(path.y)
        );
    }
    let _ = write!(out, r#" d="{}"></path>"#, escape_attr_display(&path.d));
    out.checkpoint()
}

fn push_element(
    out: &mut impl SvgOutput,
    element: &RailroadElementLayout,
    transform: Option<(f64, f64)>,
) -> Result<()> {
    let class = match element.kind.as_str() {
        "terminal" => "railroad-terminal",
        "nonterminal" => "railroad-nonterminal",
        "special" => "railroad-special",
        _ => "railroad-group",
    };
    let _ = write!(out, r#"<g class="{}""#, class);
    push_optional_transform(out, transform);
    out.push('>');
    match element.kind.as_str() {
        "terminal" => {
            let _ = write!(
                out,
                r#"<rect x="0" y="0" width="{}" height="{}" rx="10" ry="10"></rect>"#,
                fmt(element.width),
                fmt(element.height)
            );
        }
        _ => {
            let _ = write!(
                out,
                r#"<rect x="0" y="0" width="{}" height="{}"></rect>"#,
                fmt(element.width),
                fmt(element.height)
            );
        }
    }
    let _ = write!(
        out,
        r#"<text x="{}" y="{}">{}</text></g>"#,
        fmt(element.text_x),
        fmt(element.text_y),
        escape_xml_display(&element.label)
    );
    out.checkpoint()
}

fn write_railroad_css_scope(out: &mut impl SvgOutput, diagram_id: &str) -> Result<()> {
    out.push('#');
    for ch in diagram_id.chars() {
        if matches!(ch, '.' | ':') {
            out.push('\\');
        }
        out.push(ch);
    }
    out.checkpoint()
}

fn write_railroad_css(
    out: &mut impl SvgOutput,
    style: &crate::railroad::RailroadStyle,
    diagram_id: &str,
) -> Result<()> {
    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-diagram{{font-family:{};font-size:{}px;}}",
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-terminal rect{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.terminal_fill,
        style.terminal_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-terminal text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.terminal_text_color,
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-nonterminal rect{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.non_terminal_fill,
        style.non_terminal_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-nonterminal text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.non_terminal_text_color,
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-line{{stroke:{};stroke-width:{}px;fill:none;}}",
        style.line_color,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    out.push_str(" .railroad-start circle,");
    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(out, " .railroad-end circle{{fill:{};}}", style.marker_fill);
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-comment ellipse{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.comment_fill,
        style.comment_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-comment text{{fill:{};font-style:italic;font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.comment_text_color,
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-special rect{{fill:{};stroke:{};stroke-width:{}px;stroke-dasharray:5,3;}}",
        style.special_fill,
        style.special_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-special text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.non_terminal_text_color,
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    let _ = write!(
        out,
        " .railroad-rule-name{{font-weight:bold;fill:{};font-family:{};font-size:{}px;}}",
        style.rule_name_color,
        style.font_family,
        fmt(style.font_size)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, diagram_id)?;
    out.push_str(" .railroad-group{}");
    out.checkpoint()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_font_rule_matches_mermaid_namespacing() {
        let style = crate::railroad::railroad_style(&serde_json::json!({}));
        let mut css = String::new();
        write_railroad_css(&mut css, &style, "railroad.fixture").expect("write Railroad CSS");

        assert!(css.starts_with("#railroad\\.fixture .railroad-diagram{"));
        assert!(!css.starts_with("#railroad\\.fixture.railroad-diagram{"));
    }
}
