use merman_model::families::railroad::RailroadDiagramModel;
use merman_render_core::{
    Error, Result,
    number_format::canonical_number as fmt,
    resources::OperationWorkMeter,
    root_svg::{DiagramBounds, RootChrome, RootViewportContext, RootViewportSpec, RootedSvg},
    svg_css::{SvgId, escape_css_identifier},
    svg_output::{BoundedSvgOutput, SvgOutput, XmlTextOutput},
    text::TextMeasurer,
    xml::{escape_xml_display, is_xml_1_0_char},
};

use super::{
    RailroadDiagramLayout, RailroadElementLayout, RailroadPathLayout, RailroadStyle,
    layout::{RailroadRenderNode, railroad_render_node},
};

/// Emits the complete Railroad SVG before shared canvas and terminal admission.
pub fn render_railroad_svg(
    layout: &RailroadDiagramLayout,
    model: &RailroadDiagramModel,
    style: &RailroadStyle,
    diagram_id: SvgId<'_>,
    root_viewport: &RootViewportContext<'_>,
    work_meter: &OperationWorkMeter,
    measurer: &dyn TextMeasurer,
) -> Result<RootedSvg> {
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    let aria_labelledby = acc_title.map(|_| format!("chart-title-{diagram_id}"));
    let aria_describedby = acc_descr.map(|_| format!("chart-desc-{diagram_id}"));
    let root_bounds = DiagramBounds::from_view_box(0.0, 0.0, layout.width, layout.height);
    let root_spec = RootViewportSpec::mermaid(root_bounds, layout.use_max_width);
    let mut out = BoundedSvgOutput::new(work_meter);
    let mut root_chrome = RootChrome::new(diagram_id, &layout.diagram_type);
    root_chrome.class = Some("railroad-diagram");
    root_chrome.aria_labelledby = aria_labelledby.as_deref();
    root_chrome.aria_describedby = aria_describedby.as_deref();
    root_chrome.dom.trailing_newline = false;
    let root_document = root_viewport.write_open(&mut out, root_spec, root_chrome)?;

    write_svg_contents(&mut out, layout, model, style, measurer, diagram_id)?;
    out.push_str("</svg>\n");
    root_document.complete(out.finish()?)
}

fn write_svg_contents(
    out: &mut impl SvgOutput,
    layout: &RailroadDiagramLayout,
    model: &RailroadDiagramModel,
    style: &RailroadStyle,
    measurer: &dyn TextMeasurer,
    diagram_id: SvgId<'_>,
) -> Result<()> {
    let acc_title = model.acc_title.as_deref().filter(|value| !value.is_empty());
    let acc_descr = model.acc_descr.as_deref().filter(|value| !value.is_empty());
    if let Some(title) = acc_title {
        let _ = write!(
            out,
            r#"<title id="chart-title-{diagram_id}">{}</title>"#,
            escape_xml_display(title)
        );
        out.checkpoint()?;
    }
    if let Some(descr) = acc_descr {
        let _ = write!(
            out,
            r#"<desc id="chart-desc-{diagram_id}">{}</desc>"#,
            escape_xml_display(descr)
        );
        out.checkpoint()?;
    }
    out.push_str("<style>");
    out.checkpoint()?;
    write_railroad_css(
        &mut XmlTextOutput::new(out),
        style,
        diagram_id.semantic_str(),
    )?;
    out.checkpoint()?;
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
            out,
            r#"<g class="railroad-rule" transform="translate({}, {})">"#,
            fmt(rule.x),
            fmt(rule.y)
        );
        out.checkpoint()?;
        let (render_node, definition_up) =
            railroad_render_node(&model_rule.definition, style, measurer);
        let _ = write!(
            out,
            r#"<g transform="translate({}, {})">"#,
            fmt(rule.definition_x),
            fmt(rule.baseline_y - definition_up)
        );
        out.checkpoint()?;
        push_render_node(out, &render_node, style.terminal_radius)?;
        out.push_str("</g>");
        out.checkpoint()?;
        let _ = write!(
            out,
            r#"<g class="railroad-rule-name-group"><text class="railroad-rule-name" x="0" y="{}">{} =</text></g>"#,
            fmt(rule.baseline_y),
            escape_xml_display(&rule.name)
        );
        out.checkpoint()?;
        let _ = write!(
            out,
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
            push_path(out, path)?;
        }
        out.push_str("</g>");
        out.checkpoint()?;
    }
    Ok(())
}

fn push_render_node(
    out: &mut impl SvgOutput,
    node: &RailroadRenderNode,
    terminal_radius: f64,
) -> Result<()> {
    match node {
        RailroadRenderNode::Group {
            class,
            transform,
            children,
        } => {
            let _ = write!(out, r#"<g class="{}""#, SvgAttribute(class));
            push_optional_transform(out, *transform);
            out.push('>');
            out.checkpoint()?;
            for child in children {
                push_render_node(out, child, terminal_radius)?;
            }
            out.push_str("</g>");
            out.checkpoint()
        }
        RailroadRenderNode::Element { layout, transform } => {
            push_element(out, layout, *transform, terminal_radius)
        }
        RailroadRenderNode::Path(path) => push_path(out, path),
    }
}

fn push_optional_transform(out: &mut impl SvgOutput, transform: Option<(f64, f64)>) {
    if let Some((x, y)) = transform {
        let _ = write!(out, r#" transform="translate({}, {})""#, fmt(x), fmt(y));
    }
}

fn push_path(out: &mut impl SvgOutput, path: &RailroadPathLayout) -> Result<()> {
    out.push_str(r#"<path class="railroad-line""#);
    if path.x != 0.0 || path.y != 0.0 {
        let _ = write!(
            out,
            r#" transform="translate({}, {})""#,
            fmt(path.x),
            fmt(path.y)
        );
    }
    let _ = write!(out, r#" d="{}"></path>"#, SvgAttribute(&path.d));
    out.checkpoint()
}

fn push_element(
    out: &mut impl SvgOutput,
    element: &RailroadElementLayout,
    transform: Option<(f64, f64)>,
    terminal_radius: f64,
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
                r#"<rect x="0" y="0" width="{}" height="{}" rx="{radius}" ry="{radius}"></rect>"#,
                fmt(element.width),
                fmt(element.height),
                radius = fmt(terminal_radius),
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

fn write_railroad_css_scope(out: &mut impl SvgOutput, diagram_id_css: &str) -> Result<()> {
    out.push('#');
    out.push_str(diagram_id_css);
    out.checkpoint()
}

fn write_railroad_css(
    out: &mut impl SvgOutput,
    style: &RailroadStyle,
    diagram_id: &str,
) -> Result<()> {
    let diagram_id_css = escape_css_identifier(diagram_id);
    let font_size_css = fmt(style.font_size).to_string();
    let title_weight = style.rule_name_weight().map(|weight| weight.to_string());

    let _ = write!(
        out,
        "#{diagram_id_css}.railroad-diagram{{font-family:{};font-size:{}px;}}",
        style.font_family, font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-terminal rect{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.terminal_fill,
        style.terminal_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-terminal text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.terminal_text_color,
        style.terminal_font_family(),
        font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-nonterminal rect{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.non_terminal_fill,
        style.non_terminal_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-nonterminal text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.non_terminal_text_color, style.font_family, font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-line{{stroke:{};stroke-width:{}px;fill:none;}}",
        style.line_color,
        fmt(style.connector_width.unwrap_or(style.stroke_width))
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    out.push_str(" .railroad-start circle,");
    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(out, " .railroad-end circle{{fill:{};}}", style.marker_fill);
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-comment ellipse{{fill:{};stroke:{};stroke-width:{}px;}}",
        style.comment_fill,
        style.comment_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-comment text{{fill:{};font-style:italic;font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.comment_text_color, style.font_family, font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-special rect{{fill:{};stroke:{};stroke-width:{}px;stroke-dasharray:5,3;}}",
        style.special_fill,
        style.special_stroke,
        fmt(style.stroke_width)
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-special text{{fill:{};font-family:{};font-size:{}px;text-anchor:middle;dominant-baseline:middle;}}",
        style.non_terminal_text_color, style.font_family, font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    let _ = write!(
        out,
        " .railroad-rule-name{{font-weight:{};fill:{};font-family:{};font-size:{}px;}}",
        title_weight.as_deref().unwrap_or("bold"),
        style.rule_name_color,
        style.rule_name_font_family(),
        font_size_css
    );
    out.checkpoint()?;

    write_railroad_css_scope(out, &diagram_id_css)?;
    out.push_str(" .railroad-group{}");
    out.checkpoint()?;

    // Observe cancellation here even when the writes did not reach the sink's work interval.
    out.observe_operation()
}

struct SvgAttribute<'a>(&'a str);

impl std::fmt::Display for SvgAttribute<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut start = 0;
        for (index, ch) in self.0.char_indices() {
            let replacement = match ch {
                '\n' => "&#10;",
                '\r' => "&#13;",
                '\t' => "&#9;",
                '&' => "&amp;",
                '<' => "&lt;",
                '"' => "&quot;",
                '\'' => "&#39;",
                _ if !is_xml_1_0_char(ch) => "",
                _ => continue,
            };
            if start < index {
                formatter.write_str(&self.0[start..index])?;
            }
            formatter.write_str(replacement)?;
            start = index + ch.len_utf8();
        }
        if start < self.0.len() {
            formatter.write_str(&self.0[start..])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use merman_model::{
        SourceSpan,
        families::railroad::{RailroadAstNode, RailroadRuleModel},
        family::DiagramFamilyId,
        presentation::PresentationConfig,
    };
    use merman_render_core::{
        resources::RenderResourcePolicy,
        text::{TextMetrics, TextStyle},
    };

    use super::*;
    use crate::railroad::{prepare_railroad_diagram, style::railroad_style};

    #[derive(Default)]
    struct RailroadBBoxMeasurer {
        calls: RefCell<Vec<(&'static str, String)>>,
    }

    impl TextMeasurer for RailroadBBoxMeasurer {
        fn measure(&self, _text: &str, _style: &TextStyle) -> TextMetrics {
            panic!("Railroad must request the raw SVG bbox probes");
        }

        fn measure_svg_raw_text_bbox_width_px(&self, text: &str, _style: &TextStyle) -> f64 {
            self.calls.borrow_mut().push(("width", text.to_owned()));
            match text {
                "other" => 31.25,
                "? anything ?" => 62.75,
                _ => 40.0,
            }
        }

        fn measure_svg_raw_text_bbox_height_px(&self, text: &str, style: &TextStyle) -> f64 {
            self.calls.borrow_mut().push(("height", text.to_owned()));
            style.font_size * 1.1
        }
    }

    #[test]
    fn railroad_variants_use_raw_svg_bbox_width_then_height_in_layout_and_svg() {
        let span = SourceSpan::new(0, 0);
        let model = RailroadDiagramModel {
            acc_title: Some("Grammar & tokens".to_owned()),
            acc_descr: Some("Terms < expressions".to_owned()),
            title: None,
            rules: vec![RailroadRuleModel {
                name: "expr".to_owned(),
                definition: RailroadAstNode::Choice {
                    alternatives: vec![
                        RailroadAstNode::NonTerminal {
                            name: "other".to_owned(),
                            span,
                            selection: span,
                        },
                        RailroadAstNode::Special {
                            text: "anything".to_owned(),
                            span,
                            selection: span,
                        },
                    ],
                    span,
                },
            }],
        };
        let config = PresentationConfig::empty_object();
        let id = SvgId::new("railroad-bbox").unwrap();
        let viewport = RootViewportContext::new(DiagramFamilyId::RAILROAD, id);
        let definition_calls = [
            ("width", "other"),
            ("height", "other"),
            ("width", "? anything ?"),
            ("height", "? anything ?"),
        ];
        let mut layout_calls = vec![("width", "expr ="), ("height", "expr =")];
        layout_calls.extend(definition_calls);

        for diagram_type in ["railroad", "railroadEbnf", "railroadAbnf", "railroadPeg"] {
            let measurer = RailroadBBoxMeasurer::default();
            let (layout, style) =
                prepare_railroad_diagram(&model, diagram_type, &config, None, &measurer).unwrap();
            let elements = &layout.rules[0].elements;
            let other = elements
                .iter()
                .find(|element| element.label == "other")
                .unwrap();
            let special = elements
                .iter()
                .find(|element| element.label == "? anything ?")
                .unwrap();
            assert_eq!(layout.diagram_type, diagram_type);
            assert_eq!(other.width, 31.25 + 20.0, "{diagram_type}");
            assert_eq!(special.width, 62.75 + 20.0, "{diagram_type}");
            assert_eq!(other.height, 14.0 * 1.1 + 20.0, "{diagram_type}");
            assert_eq!(special.height, other.height, "{diagram_type}");
            let calls = measurer.calls.take();
            assert_eq!(
                calls
                    .iter()
                    .map(|(op, text)| (*op, text.as_str()))
                    .collect::<Vec<_>>(),
                layout_calls
            );

            let meter =
                OperationWorkMeter::new(RenderResourcePolicy::unbounded_for_trusted_input());
            let svg =
                render_railroad_svg(&layout, &model, &style, id, &viewport, &meter, &measurer)
                    .unwrap();
            let calls = measurer.calls.into_inner();
            assert_eq!(
                calls
                    .iter()
                    .map(|(op, text)| (*op, text.as_str()))
                    .collect::<Vec<_>>(),
                definition_calls
            );
            assert!(svg.as_str().ends_with("</svg>\n"));
            let document = roxmltree::Document::parse(svg.as_str()).unwrap();
            let root = document.root_element();
            assert_eq!(root.attribute("aria-roledescription"), Some(diagram_type));
            assert_eq!(
                root.attribute("aria-labelledby"),
                Some("chart-title-railroad-bbox")
            );
            assert_eq!(
                root.attribute("aria-describedby"),
                Some("chart-desc-railroad-bbox")
            );
            let mut contents = root.children().filter(roxmltree::Node::is_element);
            assert_eq!(
                contents.next().and_then(|node| node.text()),
                Some("Grammar & tokens")
            );
            assert_eq!(
                contents.next().and_then(|node| node.text()),
                Some("Terms < expressions")
            );
        }
    }

    #[test]
    fn root_font_rule_matches_mermaid_namespacing() {
        let style = railroad_style(&serde_json::json!({}));
        let mut css = String::new();
        write_railroad_css(&mut css, &style, "railroad.fixture").expect("write Railroad CSS");

        assert!(css.starts_with("#railroad\\.fixture.railroad-diagram{"));
        assert!(!css.starts_with("#railroad\\.fixture .railroad-diagram{"));
    }

    #[test]
    fn railroad_writer_preserves_literal_font_entities_through_xml_text_output() {
        let mut style = railroad_style(&serde_json::json!({}));
        style.font_family = "\"A&amp;B\", monospace".to_owned();
        let mut serialized = String::from("<style>");
        write_railroad_css(&mut XmlTextOutput::new(&mut serialized), &style, "rr")
            .expect("write XML-encoded Railroad CSS");
        serialized.push_str("</style>");

        let document = roxmltree::Document::parse(&serialized).unwrap();
        let logical_css = document.root_element().text().unwrap();
        assert_eq!(
            logical_css
                .matches(&format!("font-family:{};", style.font_family))
                .count(),
            6
        );
        assert_eq!(serialized.matches("A&amp;amp;B").count(), 6);
    }
}
