use super::super::*;
pub(super) use crate::class::node_binding::{class_node_label_style, class_node_paint_style};

pub(super) fn class_math_html_label(
    text: &str,
    mermaid_config: Option<&merman_core::presentation::PresentationConfig>,
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

pub(super) struct ClassHtmlLabelSpec<'a> {
    pub span_class: &'a str,
    pub text: &'a str,
    pub include_p: bool,
    pub extra_span_class: Option<&'a str>,
    pub span_style: Option<&'a str>,
    pub prepared_xhtml: Option<&'a str>,
    pub mermaid_config: Option<&'a merman_core::presentation::PresentationConfig>,
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

pub(super) fn write_class_svg_plain_node_text<'a>(
    out: &mut impl SvgOutput,
    text: &str,
    terminal_style: &'a str,
) -> &'a str {
    let lines = crate::text::DeterministicTextMeasurer::normalized_text_lines(text)
        .into_iter()
        .map(|line| {
            crate::text::non_markdown_svg_words(&line)
                .map(str::to_owned)
                .collect()
        })
        .collect::<Vec<_>>();
    out.push_str(r#"<g><rect class="background" style="stroke: none"/>"#);
    crate::svg::parity::label::write_svg_text_source_word_lines_with_style(
        out,
        &lines,
        terminal_style,
        false,
        None,
    );
    out.push_str("</g>");
    terminal_style
}

pub(super) fn write_class_svg_edge_text(out: &mut impl SvgOutput, text: &str, include_style: bool) {
    crate::svg::parity::label::write_svg_text_centered(out, text, include_style);
}

pub(super) fn write_class_svg_empty_markdown_fallback(
    out: &mut impl SvgOutput,
    text: &str,
    style: &str,
) {
    out.push_str(r#"<text y="-10.1""#);
    super::super::util::write_style_attribute(out, Some(style));
    out.push_str(r#"><tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em">"#);
    escape_xml_into(out, text);
    out.push_str("</tspan></text>");
}

pub(super) fn write_class_svg_text_markdown_with_style<'a>(
    out: &mut impl SvgOutput,
    markdown: &str,
    terminal_style: &'a str,
) -> Option<&'a str> {
    crate::svg::parity::label::write_svg_text_markdown_with_style(
        out,
        markdown,
        Some(if terminal_style.trim().is_empty() {
            ""
        } else {
            terminal_style
        }),
    )
    .filter(|style| !style.is_empty())
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
    span_style: &str,
) -> crate::text::TextMetrics {
    let style = crate::class::class_html_node_text_style(style, "font-weight:bolder", span_style);
    crate::class::class_html_measure_label_metrics(measurer, &style, text, max_width_px, "")
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
    probe_style: &TextStyle,
    wrap_probe_font_size: f64,
    bold: bool,
) -> String {
    let Some(wrap_width_px) =
        mermaid_class_svg_create_text_width_px(measurer, text, probe_style, wrap_probe_font_size)
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

    #[test]
    fn styled_class_markdown_preserves_unstyled_dom_bytes() {
        // These byte oracles retain the pre-consolidation Class writer's empty and multiline DOM.
        for (markdown, expected) in [
            (
                "",
                r#"<text y="-10.1" style=""><tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/></text>"#,
            ),
            (
                "`first\n\n**bold** *em*`",
                concat!(
                    r#"<text y="-10.1" style=""><tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="normal">first</tspan></tspan>"#,
                    r#"<tspan class="row text-outer-tspan" x="0" y="1em" dy="1.1em"><tspan font-style="normal" class="text-inner-tspan" font-weight="bold">bold</tspan><tspan font-style="italic" class="text-inner-tspan" font-weight="normal"> em</tspan></tspan></text>"#,
                ),
            ),
        ] {
            for style in ["", " \t", "color:#123456;fill:#123456"] {
                let mut output = String::new();
                let receipt =
                    write_class_svg_text_markdown_with_style(&mut output, markdown, style);
                if style.trim().is_empty() {
                    assert_eq!(receipt, None);
                    assert_eq!(output, expected);
                } else {
                    assert_eq!(receipt, Some(style));
                    assert_eq!(output.replacen(style, "", 1), expected);
                    let document = crate::xml_dom::Document::parse(&output).unwrap();
                    assert_eq!(document.root_element().attribute("style"), Some(style));
                }
            }
        }
        for markdown in [
            "",
            "`first\n\n**bold** *em* &amp; &lt;`",
            "plain &nbsp; text",
            "\\\\method()",
        ] {
            let mut plain = String::new();
            crate::svg::parity::label::write_svg_text_markdown_with_style(
                &mut plain,
                markdown,
                Some(""),
            );
            let mut styled = String::new();
            let style = "color:#123456;fill:#123456";
            assert_eq!(
                write_class_svg_text_markdown_with_style(&mut styled, markdown, style),
                Some(style)
            );
            assert_eq!(styled.replacen(style, "", 1), plain, "{markdown:?}");
            for empty in ["", " \t"] {
                let mut empty_style = String::new();
                assert_eq!(
                    write_class_svg_text_markdown_with_style(&mut empty_style, markdown, empty),
                    None
                );
                assert_eq!(empty_style, plain, "{markdown:?}");
            }
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
