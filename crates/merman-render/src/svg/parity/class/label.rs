use super::super::*;

pub(super) fn class_math_html_label(
    text: &str,
    mermaid_config: Option<&merman_core::MermaidConfig>,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
) -> Option<String> {
    if !crate::math::contains_delimited_math(text) {
        return None;
    }
    let (Some(config), Some(renderer)) = (mermaid_config, math_renderer) else {
        return None;
    };
    crate::math::render_math_html_label(text, config, Some(renderer))
        .map(crate::math::mark_math_html_native_unavailable)
}

pub(super) struct ClassInlineStyles<'a> {
    pub style_attr: String,
    pub color: Option<&'a str>,
    pub fill: Option<&'a str>,
    pub stroke: Option<&'a str>,
    pub stroke_width: Option<&'a str>,
    pub stroke_dasharray: Option<&'a str>,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ClassNodeLabelTerminalTruth {
    source_owns_paint: bool,
    typed_fill_verified: bool,
}

impl ClassNodeLabelTerminalTruth {
    pub(super) const fn source_owns_paint(self) -> bool {
        self.source_owns_paint
    }

    pub(super) const fn typed_fill_verified(self) -> bool {
        self.typed_fill_verified
    }
}

pub(super) struct ClassHtmlLabelSpec<'a> {
    pub span_class: &'a str,
    pub text: &'a str,
    pub include_p: bool,
    pub extra_span_class: Option<&'a str>,
    pub span_style: Option<&'a str>,
    pub prepared_xhtml: Option<&'a str>,
    pub mermaid_config: Option<&'a merman_core::MermaidConfig>,
    pub math_renderer: Option<&'a (dyn crate::math::MathRenderer + Send + Sync)>,
}

pub(super) fn render_class_html_label<'a>(
    out: &mut impl SvgOutput,
    spec: &ClassHtmlLabelSpec<'a>,
) -> Option<&'a str> {
    out.push_str(r#"<span class=""#);
    escape_xml_into(out, spec.span_class);
    if let Some(extra) = spec
        .extra_span_class
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        out.push(' ');
        escape_xml_into(out, extra);
    }
    let span_style = spec.span_style.map(str::trim).unwrap_or("");
    out.push('"');
    let emitted_style = super::super::util::write_style_attribute(
        out,
        (spec.span_class == "nodeLabel" || !span_style.is_empty()).then_some(span_style),
    );
    out.push('>');

    let html = spec
        .prepared_xhtml
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| {
            std::borrow::Cow::Owned(
                class_math_html_label(spec.text, spec.mermaid_config, spec.math_renderer)
                    .unwrap_or_else(|| {
                        crate::text::mermaid_markdown_to_xhtml_label_fragment(spec.text, true)
                    }),
            )
        });
    if spec.include_p {
        out.push_str(&html);
    } else {
        let inner = html
            .strip_prefix("<p>")
            .and_then(|s| s.strip_suffix("</p>"))
            .unwrap_or(html.as_ref());
        out.push_str(inner);
    }
    out.push_str("</span>");
    emitted_style
}

pub(super) fn write_class_svg_plain_node_text(out: &mut impl SvgOutput, text: &str) {
    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text)
        .into_iter()
        .map(|line| {
            crate::text::non_markdown_svg_words(&line)
                .map(str::to_owned)
                .collect()
        })
        .collect::<Vec<_>>();
    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
    crate::svg::parity::label::write_svg_text_source_word_lines(out, &lines, true, false);
    out.push_str("</g>");
}

pub(super) fn write_class_svg_text_markdown(
    out: &mut impl SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_markdown(out, markdown, include_style);
}

pub(super) fn write_class_svg_edge_text(out: &mut impl SvgOutput, text: &str, include_style: bool) {
    crate::svg::parity::label::write_svg_text_centered(out, text, include_style);
}

pub(super) fn write_class_svg_edge_text_markdown(
    out: &mut impl SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_markdown_centered(out, markdown, include_style);
}

pub(super) fn write_class_svg_text_markdown_with_style<'a>(
    out: &mut impl SvgOutput,
    markdown: &str,
    terminal_style: &'a str,
) -> Option<&'a str> {
    if terminal_style.trim().is_empty() {
        write_class_svg_text_markdown(out, markdown, true);
        return None;
    }

    let markdown = markdown
        .strip_prefix('\x60')
        .and_then(|value| value.strip_suffix('\x60'))
        .unwrap_or(markdown);
    let lines = crate::text::mermaid_markdown_to_lines(markdown, true);
    out.push_str(r#"<text y="-10.1""#);
    let emitted_style = super::super::util::write_style_attribute(out, Some(terminal_style));
    out.push('>');

    if lines.len() == 1 && lines[0].is_empty() {
        out.push_str(r#"<tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return emitted_style;
    }

    for (line_index, words) in lines.iter().enumerate() {
        if line_index == 0 {
            out.push_str(r#"<tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em">"#);
        } else {
            let y_em = if line_index == 1 {
                "1em".to_string()
            } else {
                format!("{:.1}em", 1.0 + (line_index as f64 - 1.0) * 1.1)
            };
            let _ = write!(
                out,
                r#"<tspan class="row text-outer-tspan" x="0" y="{}" dy="1.1em">"#,
                y_em
            );
        }
        for (word_index, (word, kind)) in words.iter().enumerate() {
            let font_style = if *kind == crate::text::MermaidMarkdownWordType::Em {
                "italic"
            } else {
                "normal"
            };
            let font_weight = if *kind == crate::text::MermaidMarkdownWordType::Strong {
                "bold"
            } else {
                "normal"
            };
            let _ = write!(
                out,
                r#"<tspan font-style="{}" class="text-inner-tspan" font-weight="{}">"#,
                font_style, font_weight
            );
            if word_index > 0 {
                out.push(' ');
            }
            escape_xml_into(out, word);
            out.push_str("</tspan>");
        }
        out.push_str("</tspan>");
    }
    out.push_str("</text>");
    emitted_style
}

pub(super) fn write_class_svg_edge_text_with_style<'a>(
    out: &mut impl SvgOutput,
    text: &str,
    style: Option<&'a str>,
) -> Option<&'a str> {
    match style {
        Some(style) => {
            crate::svg::parity::label::write_svg_text_centered_with_style(out, text, style)
        }
        None => {
            write_class_svg_edge_text(out, text, false);
            None
        }
    }
}

pub(super) fn write_class_svg_edge_text_markdown_with_style<'a>(
    out: &mut impl SvgOutput,
    markdown: &str,
    style: Option<&'a str>,
) -> Option<&'a str> {
    crate::svg::parity::label::write_svg_text_markdown_centered_with_style(
        out,
        markdown,
        Some(style.unwrap_or("")),
    )
}

pub(super) fn write_class_diagram_title<'a>(
    out: &mut impl SvgOutput,
    x: f64,
    y: f64,
    text: &str,
    style: Option<&'a str>,
) -> Option<&'a str> {
    let _ = write!(
        out,
        r#"<text text-anchor="middle" x="{}" y="{}" class="classDiagramTitleText""#,
        fmt(x),
        fmt(y)
    );
    let emitted_style = super::super::util::write_style_attribute(out, style);
    out.push('>');
    escape_xml_into(out, text);
    out.push_str("</text>");
    emitted_style
}

pub(super) fn open_class_cardinality_label<'a>(
    out: &mut impl SvgOutput,
    style: Option<&'a str>,
) -> Option<&'a str> {
    out.push_str(r#"<span class="edgeLabel""#);
    let emitted_style = super::super::util::write_style_attribute(out, style);
    out.push('>');
    emitted_style
}

pub(super) fn open_class_note_label<'a>(
    out: &mut impl SvgOutput,
    style: Option<&'a str>,
) -> Option<&'a str> {
    out.push_str("<span");
    let emitted_style = super::super::util::write_style_attribute(out, style);
    out.push_str(r#" class="nodeLabel markdown-node-label">"#);
    emitted_style
}

pub(super) fn class_html_div_style(width: f64, max_width_px: i64) -> String {
    let max_width_px = max_width_px.max(0);
    if width >= max_width_px as f64 - 0.01 {
        format!(
            "display: table; white-space: break-spaces; line-height: 1.5; max-width: {max_width_px}px; text-align: center; width: {max_width_px}px;"
        )
    } else {
        format!(
            "display: table-cell; white-space: nowrap; line-height: 1.5; max-width: {max_width_px}px; text-align: center;"
        )
    }
}

pub(super) fn class_note_html_div_style(width: f64, max_width_px: i64) -> String {
    let max_width_px = max_width_px.max(0);
    if width >= max_width_px as f64 - 0.01 {
        format!(
            "text-align: center; white-space: break-spaces; display: table; line-height: 1.5; max-width: {max_width_px}px; width: {max_width_px}px;"
        )
    } else {
        format!(
            "text-align: center; white-space: nowrap; display: table-cell; line-height: 1.5; max-width: {max_width_px}px;"
        )
    }
}

pub(super) fn class_html_label_metrics(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
    max_width_px: i64,
    css_style: &str,
) -> crate::text::TextMetrics {
    crate::class::class_html_measure_label_metrics(measurer, style, text, max_width_px, css_style)
}

pub(super) fn class_html_title_metrics(
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    text: &str,
    max_width_px: i64,
) -> crate::text::TextMetrics {
    let markdown = crate::text::DeterministicTextMeasurer::normalized_text_lines(text)
        .into_iter()
        .map(|line| format!("**{line}**"))
        .collect::<Vec<_>>()
        .join("\n");
    crate::text::measure_markdown_with_inline_styles(
        measurer,
        markdown.as_str(),
        style,
        Some(max_width_px.max(1) as f64),
        WrapMode::HtmlLike,
    )
}

pub(super) fn class_svg_label_rect(
    metrics: &crate::text::TextMetrics,
    y_offset: f64,
) -> Option<super::Rect> {
    if !(metrics.width.is_finite() && metrics.height.is_finite()) {
        return None;
    }
    let w = metrics.width.max(0.0);
    let h = metrics.height.max(0.0);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let lines = metrics.line_count.max(1) as f64;
    let y = y_offset - (h / (2.0 * lines));
    Some(super::Rect::from_min_max(0.0, y, w, y + h))
}

pub(super) fn wrap_class_svg_text_like_mermaid(
    text: &str,
    measurer: &dyn TextMeasurer,
    style: &TextStyle,
    wrap_probe_font_size: f64,
    bold: bool,
) -> String {
    let Some(wrap_width_px) =
        mermaid_class_svg_create_text_width_px(measurer, text, style, wrap_probe_font_size)
    else {
        return text.to_string();
    };

    let mut lines: Vec<String> = Vec::new();
    for line in crate::text::DeterministicTextMeasurer::normalized_text_lines(text) {
        let mut tokens = std::collections::VecDeque::from(
            crate::text::DeterministicTextMeasurer::split_line_to_words(&line),
        );
        let mut cur = String::new();

        while let Some(tok) = tokens.pop_front() {
            if cur.is_empty() && tok == " " {
                continue;
            }

            let candidate = format!("{cur}{tok}");
            let candidate_w =
                class_svg_text_computed_length_px(measurer, candidate.trim_end(), style, bold);
            if candidate_w <= wrap_width_px {
                cur = candidate;
                continue;
            }

            if !cur.trim().is_empty() {
                lines.push(cur.trim_end().to_string());
                cur.clear();
                tokens.push_front(tok);
                continue;
            }

            if tok == " " {
                continue;
            }

            let chars = tok.chars().collect::<Vec<_>>();
            let mut cut = 1usize;
            while cut < chars.len() {
                let head: String = chars[..cut].iter().collect();
                let head_w =
                    class_svg_text_computed_length_px(measurer, head.as_str(), style, bold);
                if head_w > wrap_width_px {
                    break;
                }
                cut += 1;
            }
            cut = cut.saturating_sub(1).max(1);
            let head: String = chars[..cut].iter().collect();
            let tail: String = chars[cut..].iter().collect();
            lines.push(head);
            if !tail.is_empty() {
                tokens.push_front(tail);
            }
        }

        if !cur.trim().is_empty() {
            lines.push(cur.trim_end().to_string());
        }
    }

    if lines.len() <= 1 {
        text.to_string()
    } else {
        lines.join("\n")
    }
}

fn mermaid_class_svg_create_text_width_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    wrap_probe_font_size: f64,
) -> Option<f64> {
    let wrap_probe_style = TextStyle {
        font_family: style
            .font_family
            .clone()
            .or_else(|| Some("Arial".to_string())),
        font_size: wrap_probe_font_size.max(1.0),
        font_weight: None,
        font_style: None,
    };
    // Reuse the same body-attached calculateTextDimensions operation as Class layout.
    // Combining a generic bbox width and wrapped-text height bypasses host probe semantics.
    let width = crate::class::class_html_create_text_width_px(text, measurer, &wrap_probe_style);
    (width > 0).then_some(width as f64)
}

fn class_svg_text_computed_length_px(
    measurer: &dyn TextMeasurer,
    text: &str,
    style: &TextStyle,
    bold: bool,
) -> f64 {
    if bold {
        let bold_style = TextStyle {
            font_family: style.font_family.clone(),
            font_size: style.font_size,
            font_weight: Some("bolder".to_string()),
            font_style: None,
        };
        measurer.measure_svg_text_computed_length_px(text, &bold_style)
    } else {
        measurer.measure_svg_text_computed_length_px(text, style)
    }
}

pub(super) fn class_apply_inline_styles<'a>(
    node: &'a super::ClassSvgNode,
) -> ClassInlineStyles<'a> {
    let mut style_attr = String::new();
    let mut color: Option<&str> = None;
    let mut fill: Option<&str> = None;
    let mut stroke: Option<&str> = None;
    let mut stroke_width: Option<&str> = None;
    let mut stroke_dasharray: Option<&str> = None;

    for raw in &node.styles {
        let Some(parsed) = crate::mermaid_style::parse_style_declaration(raw) else {
            continue;
        };
        if !style_attr.is_empty() {
            style_attr.push(';');
        }
        style_attr.push_str(parsed.property_css());
        style_attr.push(':');
        style_attr.push_str(parsed.source_value());

        match parsed.property() {
            "color" => color = Some(parsed.value()),
            "fill" => fill = Some(parsed.value()),
            "stroke" => stroke = Some(parsed.value()),
            "stroke-width" => stroke_width = Some(parsed.value()),
            "stroke-dasharray" => stroke_dasharray = Some(parsed.value()),
            _ => {}
        }
    }

    ClassInlineStyles {
        style_attr,
        color,
        fill,
        stroke,
        stroke_width,
        stroke_dasharray,
    }
}

pub(super) fn class_node_label_terminal_truth(
    terminal_facts: crate::class::ClassTextTerminalFacts,
) -> ClassNodeLabelTerminalTruth {
    ClassNodeLabelTerminalTruth {
        source_owns_paint: terminal_facts.source_owns_every_visible_run(),
        typed_fill_verified: terminal_facts.paint_ownership_is_unambiguous(),
    }
}

pub(super) fn class_source_label_style(color: Option<&str>) -> String {
    color.map_or_else(String::new, |color| format!("color:{color};fill:{color}"))
}

pub(super) fn class_node_label_style(source_style: &str, typed_fill: Option<&str>) -> String {
    let mut style = source_style.trim().trim_end_matches(';').to_string();
    let Some(fill) = typed_fill else {
        return style;
    };
    if !style.is_empty() {
        style.push(';');
    }
    style.push_str("color:");
    style.push_str(fill);
    style.push_str(" !important;fill:");
    style.push_str(fill);
    style.push_str(" !important");
    style
}

pub(super) fn class_node_paint_style(
    source_style: &str,
    property: &str,
    typed_paint: Option<&str>,
) -> String {
    let mut style = source_style.trim().trim_end_matches(';').to_string();
    let Some(paint) = typed_paint else {
        return style;
    };
    if !style.is_empty() {
        style.push(';');
    }
    style.push_str(property);
    style.push(':');
    style.push_str(paint);
    style.push_str(" !important");
    style
}

#[cfg(test)]
mod tests {
    use super::{ClassHtmlLabelSpec, render_class_html_label};

    #[test]
    fn class_svg_wrap_width_uses_the_same_host_dimensions_probe_as_layout() {
        struct HostProbe;

        impl crate::text::TextMeasurer for HostProbe {
            fn measure(
                &self,
                _text: &str,
                _style: &crate::text::TextStyle,
            ) -> crate::text::TextMetrics {
                panic!("Class width must use the dedicated calculateTextDimensions operation");
            }

            fn measure_mermaid_calculate_text_dimensions(
                &self,
                text: &str,
                style: &crate::text::TextStyle,
            ) -> crate::text::TextMetrics {
                assert_eq!(text, "member");
                assert_eq!(style.font_size, 10.0);
                assert_eq!(style.font_weight, None);
                assert_eq!(style.font_style, None);
                let (width, height) = match style.font_family.as_deref() {
                    Some("sans-serif") => (120.4, 20.0),
                    Some("HostFont") => (80.4, 30.0),
                    family => panic!("unexpected probe font: {family:?}"),
                };
                crate::text::TextMetrics {
                    width,
                    height,
                    line_count: 1,
                }
            }
        }

        let render_style = crate::text::TextStyle {
            font_family: Some("HostFont".to_string()),
            font_size: 24.0,
            font_weight: Some("bold".to_string()),
            font_style: Some("italic".to_string()),
        };
        let width = super::mermaid_class_svg_create_text_width_px(
            &HostProbe,
            "member",
            &render_style,
            10.0,
        );
        // Mermaid keeps the configured font when sans-serif is wider but not taller.
        // Its rounded width (80) then receives shapeUtil's 50px wrapping allowance.
        assert_eq!(width, Some(130.0));
    }

    #[test]
    fn class_html_label_serializes_raw_html_and_escaped_generics_structurally() {
        let mut rich = String::new();
        render_class_html_label(
            &mut rich,
            &ClassHtmlLabelSpec {
                span_class: "nodeLabel",
                text: "<a href='https://example.com'><code>Entity</code></a>",
                include_p: true,
                extra_span_class: Some("markdown-node-label"),
                span_style: None,
                prepared_xhtml: None,
                mermaid_config: None,
                math_renderer: None,
            },
        );
        assert!(rich.contains(r#"<a href='https://example.com'><code>Entity</code></a>"#));
        assert!(!rich.contains("&lt;code&gt;"));

        let mut generic = String::new();
        render_class_html_label(
            &mut generic,
            &ClassHtmlLabelSpec {
                span_class: "nodeLabel",
                text: "Generic&lt;T&gt; driver_license",
                include_p: true,
                extra_span_class: Some("markdown-node-label"),
                span_style: None,
                prepared_xhtml: None,
                mermaid_config: None,
                math_renderer: None,
            },
        );
        assert!(generic.contains("<p>Generic&lt;T&gt; driver_license</p>"));
        assert!(!generic.contains("&amp;lt;"));
    }
}

#[cfg(test)]
mod edge_text_tests {
    use super::*;

    #[test]
    fn fallback_edge_text_preserves_literal_markdown_with_optional_paint() {
        let text = "**literal** & <value>";
        let mut original = String::new();
        write_class_svg_edge_text(&mut original, text, false);
        let mut unstyled = String::new();
        write_class_svg_edge_text_with_style(&mut unstyled, text, None);
        assert_eq!(unstyled, original);

        let style = "color:#123456 !important;fill:#123456 !important;";
        let mut styled = String::new();
        write_class_svg_edge_text_with_style(&mut styled, text, Some(style));
        assert_eq!(
            styled.replace(&format!(r#" style="{style}""#), ""),
            original
        );
        assert!(styled.contains("**literal**"));
    }
}

#[cfg(test)]
mod paint_emission_tests {
    use super::*;
    use crate::class::{ClassTextTerminalFacts, ClassTextThemePlan};
    use crate::diagram_theme::{
        CanvasPaint, DiagramThemeCompiler, DiagramThemeSpec, ThemeRule, ThemeRuleSet,
        ThemeStylePatch, ThemeTarget,
    };

    #[derive(Clone, Copy, Debug)]
    enum Channel {
        HtmlEdge,
        SvgEdge,
        PlainEdge,
        StartCardinality,
        EndCardinality,
        Title,
        HtmlNote,
        SvgNote,
    }

    fn verify_mutations(channel: Channel) {
        let theme = DiagramThemeCompiler::new()
            .compile(
                DiagramThemeSpec::new().with_styles(
                    ThemeRuleSet::default().with_rule(ThemeRule::new(
                        ThemeTarget::Text,
                        ThemeStylePatch::default()
                            .with_fill(CanvasPaint::solid("#123456").unwrap()),
                    )),
                ),
            )
            .unwrap()
            .resolve(crate::DiagramFamilyId::CLASS);
        let parsed = merman_core::Engine::new()
            .parse_diagram_for_render_model_sync(
                "classDiagram\n",
                merman_core::ParseOptions::default(),
            )
            .unwrap()
            .unwrap();
        let merman_core::RenderSemanticModel::Class(empty_model) = parsed.model() else {
            panic!("Class model")
        };
        let mut model = empty_model.clone();
        let edge = matches!(
            channel,
            Channel::HtmlEdge | Channel::SvgEdge | Channel::PlainEdge
        );
        let start = matches!(channel, Channel::StartCardinality);
        let end = matches!(channel, Channel::EndCardinality);
        let title = matches!(channel, Channel::Title);
        let note = matches!(channel, Channel::HtmlNote | Channel::SvgNote);
        // Only text checkpoint identity matters in this writer/receipt unit fixture.
        if !title && !note {
            model
                .relations
                .push(merman_core::models::class_diagram::ClassRelation {
                    id: "rel".into(),
                    id1: "A".into(),
                    id2: "B".into(),
                    relation_title_1: start.then(|| "Label".into()),
                    relation_title_2: end.then(|| "Label".into()),
                    title: if edge { "Label" } else { "" }.into(),
                    relation: merman_core::models::class_diagram::RelationShape {
                        type1: -1,
                        type2: -1,
                        line_type: 0,
                    },
                });
        }
        if note {
            model
                .notes
                .push(merman_core::models::class_diagram::ClassNote {
                    id: "note".into(),
                    class_id: None,
                    text: "Label".into(),
                    parent: None,
                });
        }
        for style in [
            None,
            Some("color:#123456"),
            Some("color:#123456;fill:#000000"),
            Some("color:#000000;fill:#123456"),
            Some("color:#123456;fill:#123456"),
        ] {
            let plan =
                ClassTextThemePlan::resolve(Some(&theme), &parsed.metadata().effective_config);
            assert!(plan.seal_node_style_facts(Default::default()));
            let mut receipt = plan
                .begin_terminal_receipt(
                    &model,
                    title.then_some("Label"),
                    false,
                    matches!(channel, Channel::HtmlEdge),
                    None,
                )
                .unwrap();
            plan.bind_paint_expectations(&mut receipt, &[], None, note.then_some("note"));
            let mut css = String::new();
            receipt.record_css_emission(
                super::super::css::write_class_css(
                    &mut css,
                    "receipt-test",
                    parsed.metadata().effective_config.as_value(),
                    plan.stylesheet_font_family_css(),
                    plan.font_size_css(),
                    true,
                )
                .unwrap()
                .unwrap(),
            );
            let mut svg = String::new();
            let emitted = match channel {
                Channel::HtmlEdge => render_class_html_label(
                    &mut svg,
                    &ClassHtmlLabelSpec {
                        span_class: "edgeLabel",
                        text: "Label",
                        include_p: true,
                        extra_span_class: None,
                        span_style: style,
                        prepared_xhtml: None,
                        mermaid_config: None,
                        math_renderer: None,
                    },
                ),
                Channel::SvgEdge => {
                    write_class_svg_edge_text_markdown_with_style(&mut svg, "Label", style)
                }
                Channel::PlainEdge => {
                    write_class_svg_edge_text_with_style(&mut svg, "Label", style)
                }
                Channel::StartCardinality | Channel::EndCardinality => {
                    let emitted = open_class_cardinality_label(&mut svg, style);
                    svg.push_str("<p>Label</p></span>");
                    emitted
                }
                Channel::HtmlNote => {
                    let emitted = open_class_note_label(&mut svg, style);
                    svg.push_str("<p>Label</p></span>");
                    emitted
                }
                Channel::SvgNote => {
                    write_class_svg_text_markdown_with_style(&mut svg, "Label", style.unwrap_or(""))
                }
                Channel::Title => write_class_diagram_title(&mut svg, 20.0, 30.0, "Label", style),
            };
            svg.checkpoint().unwrap();
            let doc = roxmltree::Document::parse(&svg).unwrap();
            assert_eq!(
                doc.root_element()
                    .attribute("style")
                    .filter(|style| !style.is_empty()),
                emitted.filter(|style| !style.is_empty()),
                "{channel:?}"
            );
            assert_eq!(emitted.unwrap_or(""), style.unwrap_or(""), "{channel:?}");
            let paint = if title {
                plan.title_paint()
            } else if note {
                plan.note_paint()
            } else {
                plan.edge_paint()
            }
            .unwrap();
            let facts = paint.observe(ClassTextTerminalFacts::inherited_text("Label"), emitted);
            let empty = ClassTextTerminalFacts::default();
            receipt.record_diagram_title(if title { facts } else { empty });
            if note {
                receipt.record_node("note", facts);
            } else if !title {
                receipt.record_edge_label("rel", if edge { facts } else { empty });
                for slot in 0..4 {
                    receipt.record_cardinality(
                        "rel",
                        slot,
                        if (slot == 1 && start) || (slot == 2 && end) {
                            facts
                        } else {
                            empty
                        },
                    );
                }
            }
            assert!(plan.record_terminal(Some(receipt)));
            let evidence = plan.finish_evidence();
            let expected = usize::from(style == Some("color:#123456;fill:#123456"));
            assert_eq!(evidence.required_mechanisms().len(), 1);
            assert_eq!(evidence.applied().len(), expected, "{channel:?}: {style:?}");
            assert!(evidence.not_applicable_mechanisms().is_empty());
            assert!(evidence.residuals().is_empty());
        }
    }

    #[test]
    fn edge_paint_receipt_rejects_missing_or_wrong_writer_styles() {
        for channel in [Channel::HtmlEdge, Channel::SvgEdge, Channel::PlainEdge] {
            verify_mutations(channel);
        }
    }

    #[test]
    fn cardinality_paint_receipt_rejects_missing_or_wrong_writer_styles() {
        for channel in [Channel::StartCardinality, Channel::EndCardinality] {
            verify_mutations(channel);
        }
    }

    #[test]
    fn title_paint_receipt_rejects_missing_or_wrong_writer_styles() {
        verify_mutations(Channel::Title);
    }

    #[test]
    fn note_paint_receipt_rejects_missing_or_wrong_writer_styles() {
        for channel in [Channel::HtmlNote, Channel::SvgNote] {
            verify_mutations(channel);
        }
    }

    #[test]
    fn failed_attribute_write_has_no_emission() {
        use crate::resources::{OperationWorkMeter, RenderResourcePolicy, ResourceLimitId};
        let style = "color:#123456;fill:#123456";
        let mut prefix = String::new();
        let emitted = open_class_cardinality_label(&mut prefix, Some(style));
        assert_eq!(emitted, Some(style));
        // Fail before the attribute, inside its value, and before its closing quote.
        for budget in [1, r#"<span class="edgeLabel""#.len() + 10, prefix.len() - 2] {
            let policy = RenderResourcePolicy::unbounded_for_trusted_input()
                .with_limit(ResourceLimitId::MaxSvgBytes, budget)
                .unwrap();
            let meter = OperationWorkMeter::new(policy);
            let mut out = crate::svg::parity::BoundedSvgOutput::new(&meter);
            assert!(
                open_class_cardinality_label(&mut out, Some(style)).is_none(),
                "budget={budget}"
            );
            assert!(out.checkpoint().is_err());
            assert!(out.finish().is_err());
        }
        let policy = RenderResourcePolicy::unbounded_for_trusted_input()
            .with_limit(ResourceLimitId::MaxSvgBytes, prefix.len())
            .unwrap();
        let meter = OperationWorkMeter::new(policy);
        let mut out = crate::svg::parity::BoundedSvgOutput::new(&meter);
        assert_eq!(
            open_class_cardinality_label(&mut out, Some(style)),
            Some(style)
        );
        out.checkpoint().unwrap();
        out.push_str("<p>Label</p></span>");
        assert!(out.checkpoint().is_err());
        assert!(
            out.finish().is_err(),
            "an accepted style cannot certify a failed body write"
        );
    }
}
