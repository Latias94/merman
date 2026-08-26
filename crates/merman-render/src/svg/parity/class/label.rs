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

pub(super) fn render_class_html_label(out: &mut impl SvgOutput, spec: &ClassHtmlLabelSpec<'_>) {
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
    if spec.span_class == "nodeLabel" || !span_style.is_empty() {
        out.push_str(r#"" style=""#);
        super::super::util::escape_attr_into(out, span_style);
        out.push_str(r#"">"#);
    } else {
        out.push_str(r#"">"#);
    }

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
}

pub(super) fn write_class_svg_text_markdown(
    out: &mut impl SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_markdown(out, markdown, include_style);
}

pub(super) fn write_class_svg_text_markdown_with_style(
    out: &mut impl SvgOutput,
    markdown: &str,
    terminal_style: &str,
) {
    if terminal_style.trim().is_empty() {
        write_class_svg_text_markdown(out, markdown, true);
        return;
    }

    let markdown = markdown
        .strip_prefix('\x60')
        .and_then(|value| value.strip_suffix('\x60'))
        .unwrap_or(markdown);
    let lines = crate::text::mermaid_markdown_to_lines(markdown, true);
    out.push_str(r#"<text y="-10.1" style=""#);
    super::super::util::escape_attr_into(out, terminal_style);
    out.push_str(r#"">"#);

    if lines.len() == 1 && lines[0].is_empty() {
        out.push_str(r#"<tspan class="row text-outer-tspan" x="0" y="-0.1em" dy="1.1em"/>"#);
        out.push_str("</text>");
        return;
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
    let wrap_probe_font_size = wrap_probe_font_size.max(1.0);
    // Mermaid `calculateTextWidth(...)` selects between `sans-serif` and the configured font
    // family using `calculateTextDimensions(...)` (it does *not* always take the max width).
    // Replicate that selection logic so SVG-label wrapping matches Mermaid's utility contract.
    #[derive(Clone, Copy)]
    struct Dim {
        width: f64,
        height: f64,
        line_height: f64,
    }
    fn dim_for(measurer: &dyn TextMeasurer, text: &str, style: &TextStyle) -> Dim {
        let width = measurer
            .measure_svg_simple_text_bbox_width_px(text, style)
            .max(0.0)
            .round();
        let height = measurer
            .measure_wrapped(text, style, None, WrapMode::SvgLike)
            .height
            .max(0.0)
            .round();
        Dim {
            width,
            height,
            line_height: height,
        }
    }

    let wrap_probe_style = TextStyle {
        font_family: style
            .font_family
            .clone()
            .or_else(|| Some("Arial".to_string())),
        font_size: wrap_probe_font_size,
        font_weight: None,
        font_style: None,
    };
    let sans_probe_style = TextStyle {
        font_family: Some("sans-serif".to_string()),
        font_size: wrap_probe_font_size,
        font_weight: None,
        font_style: None,
    };
    let dims = [
        dim_for(measurer, text, &sans_probe_style),
        dim_for(measurer, text, &wrap_probe_style),
    ];
    let pick_sans = dims[1].height.is_nan()
        || dims[1].width.is_nan()
        || dims[1].line_height.is_nan()
        || (dims[0].height > dims[1].height
            && dims[0].width > dims[1].width
            && dims[0].line_height > dims[1].line_height);
    let w = dims[if pick_sans { 0 } else { 1 }].width + 50.0;
    if w.is_finite() && w > 0.0 {
        Some(w)
    } else {
        None
    }
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
    prepared_facts: crate::class::ClassNodeLabelStyleFacts,
) -> ClassNodeLabelTerminalTruth {
    ClassNodeLabelTerminalTruth {
        source_owns_paint: prepared_facts.source_owns_every_visible_run(),
        typed_fill_verified: !prepared_facts.has_mixed_color_ownership()
            && !prepared_facts.color_ownership_is_unverified(),
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
