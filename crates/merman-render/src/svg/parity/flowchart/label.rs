//! Flowchart label rendering helpers (HTML/SVG text).

#[cfg(test)]
use crate::graph_label::{
    is_single_img_label, normalize_flowchart_img_tags, starts_with_ascii_case_insensitive,
};

pub(in crate::svg::parity) fn flowchart_label_html_with_prepared_label<'a>(
    label: &'a str,
    label_type: &str,
    config: &merman_core::presentation::PresentationConfig,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
    prepared_label: crate::flowchart::FlowchartPreparedLabelResolution<'a>,
) -> std::borrow::Cow<'a, str> {
    match prepared_label {
        crate::flowchart::FlowchartPreparedLabelResolution::PreparedMath(prepared) => {
            std::borrow::Cow::Borrowed(prepared.browser_xhtml())
        }
        crate::flowchart::FlowchartPreparedLabelResolution::PreparedHtml(xhtml) => {
            std::borrow::Cow::Borrowed(xhtml)
        }
        crate::flowchart::FlowchartPreparedLabelResolution::Unavailable(_reason) => {
            std::borrow::Cow::Owned(flowchart_label_html_impl(label, label_type, config, None))
        }
        crate::flowchart::FlowchartPreparedLabelResolution::NotPrepared => std::borrow::Cow::Owned(
            flowchart_label_html_impl(label, label_type, config, math_renderer),
        ),
    }
}

fn flowchart_label_html_impl(
    label: &str,
    label_type: &str,
    config: &merman_core::presentation::PresentationConfig,
    math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
) -> String {
    if let Some(html) = crate::math::render_math_html_label(label, config, math_renderer) {
        return html;
    }
    let html = crate::graph_label::flowchart_label_html_source(label, label_type, config);
    merman_core::sanitize::sanitize_xhtml(&html, config.as_config())
}

pub(in crate::svg::parity) fn flowchart_label_plain_text(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    crate::flowchart::flowchart_label_plain_text_for_layout(label, label_type, html_labels)
}

pub(in crate::svg::parity) fn write_flowchart_empty_svg_text_centered(
    out: &mut impl crate::svg::parity::SvgOutput,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_source_word_lines(
        out,
        &[Vec::new()],
        include_style,
        true,
    );
}

pub(in crate::svg::parity) fn write_flowchart_svg_source_word_lines(
    out: &mut impl crate::svg::parity::SvgOutput,
    lines: &[Vec<String>],
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_source_word_lines(out, lines, include_style, false);
}

pub(crate) fn write_flowchart_svg_label_plan(
    out: &mut impl crate::svg::parity::SvgOutput,
    plan: &crate::flowchart::FlowchartSvgLabelRenderPlan<'_>,
    include_style: bool,
) {
    write_flowchart_svg_label_plan_with_style(out, plan, include_style, None);
}

pub(crate) fn write_flowchart_svg_label_plan_with_style(
    out: &mut impl crate::svg::parity::SvgOutput,
    plan: &crate::flowchart::FlowchartSvgLabelRenderPlan<'_>,
    include_style: bool,
    extra_style: Option<&str>,
) {
    let lines = plan.wrapped_lines();
    let style = plan
        .merge_emission_font_style(include_style.then_some(""))
        .map(|style| append_svg_label_style(style, extra_style));
    if let Some(style) = style.as_deref() {
        crate::svg::parity::label::write_svg_text_source_word_lines_with_style(
            out,
            &lines,
            style,
            false,
            plan.font_weight(),
        );
    } else if let Some(extra_style) = extra_style.filter(|style| !style.is_empty()) {
        crate::svg::parity::label::write_svg_text_source_word_lines_with_style(
            out,
            &lines,
            extra_style,
            false,
            plan.font_weight(),
        );
    } else {
        write_flowchart_svg_source_word_lines(out, &lines, include_style);
    }
}

fn append_svg_label_style(mut style: String, extra_style: Option<&str>) -> String {
    let Some(extra_style) = extra_style.filter(|style| !style.is_empty()) else {
        return style;
    };
    if !style.trim_end().ends_with(';') {
        style.push(';');
    }
    style.push_str(extra_style);
    style
}

pub(crate) fn write_flowchart_svg_label_plan_centered_with_style(
    out: &mut impl crate::svg::parity::SvgOutput,
    plan: &crate::flowchart::FlowchartSvgLabelRenderPlan<'_>,
    style: &str,
) {
    let lines = plan.wrapped_lines();
    let style = plan
        .merge_emission_font_style(Some(style))
        .unwrap_or_else(|| style.to_string());
    crate::svg::parity::label::write_svg_text_source_word_lines_with_style(
        out,
        &lines,
        &style,
        true,
        plan.font_weight(),
    );
}

#[cfg(test)]
pub(in crate::svg::parity) fn wrap_flowchart_svg_source_word_lines(
    measurer: &dyn crate::text::TextMeasurer,
    lines: &[Vec<String>],
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
    break_long_words: bool,
) -> Vec<Vec<String>> {
    crate::flowchart::flowchart_wrap_svg_source_word_lines(
        measurer,
        lines,
        style,
        max_width_px,
        break_long_words,
    )
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
) {
    crate::svg::parity::label::write_svg_text_markdown_from_create_text_source(
        out,
        markdown,
        include_style,
    );
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown_wrapped_centered(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    crate::svg::parity::label::write_svg_text_markdown_wrapped_centered_from_create_text_source(
        out,
        markdown,
        include_style,
        measurer,
        style,
        max_width_px,
    );
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown_wrapped(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    include_style: bool,
    measurer: &dyn crate::text::TextMeasurer,
    style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    crate::svg::parity::label::write_svg_text_markdown_wrapped_from_create_text_source(
        out,
        markdown,
        include_style,
        measurer,
        style,
        max_width_px,
    );
}

pub(in crate::svg::parity) fn write_flowchart_svg_text_markdown_wrapped_with_style(
    out: &mut impl crate::svg::parity::SvgOutput,
    markdown: &str,
    style: &str,
    measurer: &dyn crate::text::TextMeasurer,
    text_style: &crate::text::TextStyle,
    max_width_px: Option<f64>,
) {
    crate::svg::parity::label::write_svg_text_markdown_wrapped_from_create_text_source_with_style(
        out,
        markdown,
        style,
        measurer,
        text_style,
        max_width_px,
        false,
    );
}

#[cfg(test)]
mod tests {
    use super::{
        flowchart_label_html_impl, flowchart_label_html_with_prepared_label, is_single_img_label,
        normalize_flowchart_img_tags, starts_with_ascii_case_insensitive,
        wrap_flowchart_svg_source_word_lines, write_flowchart_svg_source_word_lines,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Debug)]
    struct ReentryProbe<'a>(&'a AtomicUsize);

    impl crate::math::MathRenderer for ReentryProbe<'_> {
        fn render_html_label(
            &self,
            _text: &str,
            _config: &merman_core::presentation::PresentationConfig,
        ) -> Option<String> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Some("unexpected backend reentry".to_string())
        }
    }

    #[test]
    fn unavailable_prepared_math_reason_prevents_writer_backend_reentry() {
        let calls = AtomicUsize::new(0);
        let renderer = ReentryProbe(&calls);
        let html = flowchart_label_html_with_prepared_label(
            "value $$x$$",
            "text",
            &merman_core::presentation::PresentationConfig::default(),
            Some(&renderer),
            crate::flowchart::FlowchartPreparedLabelResolution::Unavailable(
                crate::math::MathPreparationUnavailable::BackendDeclined,
            ),
        );

        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(!html.contains("unexpected backend reentry"));
        assert!(html.contains("$$x$$"));
    }

    #[test]
    fn flowchart_img_normalization_matches_ascii_case_without_touching_non_img_tags() {
        assert_eq!(
            normalize_flowchart_img_tags(r#"<IMG SRC='diagram.svg'>"#, true),
            concat!(
                r#"<img src="diagram.svg" style="display: flex; flex-direction: column; "#,
                r#"min-width: 80px; max-width: 80px;">"#,
            ),
        );
        assert!(is_single_img_label(" \n<IMG SRC='diagram.svg'>\t"));
        assert!(!is_single_img_label("<IMG SRC='diagram.svg'><br>"));

        let without_img = r#"<br><im src='x'><div data-kind='IMG'>body</div>"#;
        assert_eq!(
            normalize_flowchart_img_tags(without_img, false),
            without_img,
        );
    }

    #[test]
    fn flowchart_img_normalization_scans_many_prior_tags_once() {
        const BREAK_COUNT: usize = 8_192;

        let mut input = "<br>".repeat(BREAK_COUNT);
        input.push_str(r#"<ImG SrC='tail.svg'>"#);

        let output = normalize_flowchart_img_tags(&input, false);
        assert_eq!(output.matches("<br>").count(), BREAK_COUNT);
        assert!(output.ends_with(concat!(
            r#"<img src="tail.svg" style="display: flex; flex-direction: column; "#,
            r#"width: 100%;">"#,
        )));
    }

    #[test]
    fn flowchart_img_prefix_match_handles_mixed_case_and_truncated_boundaries() {
        for input in ["<img", "<IMG", "<ImG src='diagram.svg'>"] {
            assert!(starts_with_ascii_case_insensitive(input, b"<img"));
        }
        for input in ["", "<", "<i", "<im", "<imx"] {
            assert!(!starts_with_ascii_case_insensitive(input, b"<img"));
            assert_eq!(normalize_flowchart_img_tags(input, false), input);
            assert!(!is_single_img_label(input));
        }

        assert_eq!(normalize_flowchart_img_tags("<IMG", false), "<IMG");
        assert!(!is_single_img_label("<IMG"));
    }

    #[test]
    fn markdown_html_output_follows_upstream_raw_tokens_and_escapes() {
        for security_level in ["strict", "loose", "antiscript", "sandbox"] {
            let config =
                merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
                    "htmlLabels": true,
                    "securityLevel": security_level,
                }));
            for (input, text) in [
                ("`<span style='font-size:64px'>AVAVAV</span>`", "`AVAVAV`"),
                (
                    "``<span style='font-size:64px'>`**AVAVAV**`</span>``",
                    "```**AVAVAV**```",
                ),
                (r"\<span style='font-size:64px'>AVAVAV\</span>", "AVAVAV"),
                (r"\\<span style='font-size:64px'>AVAVAV\\</span>", "AVAVAV"),
            ] {
                let output = flowchart_label_html_impl(input, "markdown", &config, None);
                let document = crate::xml_dom::Document::parse(&output).unwrap();
                let span = document
                    .descendants()
                    .find(|node| node.has_tag_name("span"))
                    .unwrap_or_else(|| panic!("expected effective HTML: {input}: {output}"));
                assert_eq!(span.attribute("style"), Some("font-size:64px"));
                assert_eq!(
                    crate::text::mermaid_xhtml_label_text_content(&output).as_deref(),
                    Some(text)
                );
            }
        }
    }

    #[test]
    fn markdown_html_output_decodes_mermaid_entities_before_html_projection() {
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "htmlLabels": true,
            "securityLevel": "loose"
        }));
        for input in [
            "`#60;span style=#quot;font-size:64px#quot;#62;AVAVAV#60;/span#62;`\n\n",
            "`ﬂ°°60¶ßspan style=ﬂ°quot¶ßfont-size:64pxﬂ°quot¶ßﬂ°°62¶ßAVAVAVﬂ°°60¶ß/spanﬂ°°62¶ß`\n\n",
        ] {
            let output = flowchart_label_html_impl(input, "markdown", &config, None);
            assert_eq!(
                output,
                "<p>`<span style=\"font-size:64px\">AVAVAV</span>`</p>"
            );
        }
    }

    #[test]
    fn markdown_html_output_preserves_authored_raw_tags() {
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "htmlLabels": true,
            "securityLevel": "loose"
        }));
        let input = "Before <span style='font-size:64px'>AVAVAV</span> after\n\n";
        let output = flowchart_label_html_impl(input, "markdown", &config, None);
        let document = crate::xml_dom::Document::parse(&output).unwrap();
        let span = document
            .descendants()
            .find(|node| node.has_tag_name("span"))
            .unwrap();
        assert_eq!(span.attribute("style"), Some("font-size:64px"));
        assert_eq!(span.text(), Some("AVAVAV"));
        assert_eq!(
            crate::text::mermaid_xhtml_label_text_content(&output).as_deref(),
            Some("Before AVAVAV after")
        );
    }

    #[test]
    fn html_image_prefix_matching_preserves_unicode_and_malformed_tags() {
        for security_level in ["strict", "loose", "antiscript", "sandbox"] {
            let config =
                merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
                    "htmlLabels": true,
                    "securityLevel": security_level
                }));
            for surrounding in ["", "<span>中文😀</span>", "&lt;img&gt;", "<é>α</é>"] {
                for suffix in [" src='x'>", "inary src='x'>", ">", " src=x>"] {
                    let lower = format!("{surrounding}<img{suffix}");
                    let expected = flowchart_label_html_impl(&lower, "string", &config, None);
                    for prefix in ["<IMG", "<Img", "<iMg", "<imG"] {
                        let input = format!("{surrounding}{prefix}{suffix}");
                        assert_eq!(
                            flowchart_label_html_impl(&input, "string", &config, None),
                            expected,
                            "security={security_level}, input={input:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn html_image_after_many_tags_preserves_text_and_image_style() {
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "htmlLabels": true,
            "securityLevel": "strict"
        }));
        let spans = "<span>中文😀</span>".repeat(1_600);
        let input = format!("{spans}<ImG src='https://example.com/x.svg'>");
        let output = flowchart_label_html_impl(&input, "string", &config, None);
        assert_eq!(output.matches("中文😀").count(), 1_600);
        assert_eq!(output.matches("<img ").count(), 1);
        assert!(output.contains("https://example.com/x.svg"), "{output}");
        assert!(output.contains("width: 100%;"), "{output}");
        assert!(output.ends_with(" /></p>"), "{output}");
    }

    #[test]
    fn html_unclosed_image_recovers_before_xhtml_serialization() {
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "htmlLabels": true,
            "securityLevel": "strict"
        }));
        assert_eq!(
            flowchart_label_html_impl("<IMG src='x'", "string", &config, None),
            r#"<p><img src="x" /></p>"#
        );
    }

    #[test]
    fn html_label_output_preserves_entity_and_direct_nbsp_boundaries() {
        let config = merman_core::presentation::PresentationConfig::from_value(serde_json::json!({
            "htmlLabels": true,
            "securityLevel": "loose"
        }));
        let cases = [
            ("&nbsp;A", "<p>\u{00A0}A</p>"),
            ("A&nbsp;", "<p>A\u{00A0}</p>"),
            ("&nbsp;", "<p>\u{00A0}</p>"),
            ("\u{00A0}A", "<p>\u{00A0}A</p>"),
            ("A\u{00A0}", "<p>A\u{00A0}</p>"),
            ("\u{00A0}", "<p>\u{00A0}</p>"),
            ("A<br>&nbsp;", "<p>A<br />\u{00A0}</p>"),
        ];

        for label_type in ["string", "markdown"] {
            for (input, expected) in cases {
                assert_eq!(
                    flowchart_label_html_impl(input, label_type, &config, None),
                    expected,
                    "label_type={label_type}, input={input:?}",
                );
            }
        }
    }

    #[test]
    fn svg_source_word_wrapping_keeps_attribute_spaces_inside_the_tag_word() {
        let source = crate::flowchart::flowchart_non_markdown_svg_source_word_lines(
            "<span class='foo bar'>X</span>",
        );
        let wrapped = wrap_flowchart_svg_source_word_lines(
            &crate::text::DeterministicTextMeasurer::default(),
            &source,
            &crate::text::TextStyle::default(),
            Some(1_000.0),
            true,
        );
        assert_eq!(wrapped, source);

        let mut svg = String::new();
        write_flowchart_svg_source_word_lines(&mut svg, &wrapped, false);
        assert_eq!(svg.matches("text-inner-tspan").count(), 3, "{svg}");
        assert!(
            svg.contains(">&lt;span class=&#39;foo bar&#39;&gt;</tspan>"),
            "{svg}"
        );
    }
}
