//! Flowchart label rendering helpers (HTML/SVG text).

#[cfg(test)]
use merman_render_flow::svg_label::write_flowchart_svg_source_word_lines;
pub(crate) use merman_render_flow::svg_label::{
    append_native_label_color, flowchart_label_html_with_prepared_label,
    write_flowchart_svg_label_plan_with_style,
    write_flowchart_svg_text_markdown_wrapped_with_style,
};

#[cfg(test)]
use crate::graph_label::{
    is_single_img_label, normalize_flowchart_img_tags, starts_with_ascii_case_insensitive,
};

pub(in crate::svg::parity) fn flowchart_label_plain_text(
    label: &str,
    label_type: &str,
    html_labels: bool,
) -> String {
    crate::flowchart::flowchart_label_plain_text_for_layout(label, label_type, html_labels)
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

#[cfg(test)]
mod tests {
    use super::{
        flowchart_label_html_with_prepared_label, is_single_img_label,
        normalize_flowchart_img_tags, starts_with_ascii_case_insensitive,
        wrap_flowchart_svg_source_word_lines, write_flowchart_svg_source_word_lines,
    };
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn flowchart_label_html_impl(
        label: &str,
        label_type: &str,
        config: &merman_model::presentation::PresentationConfig,
        math_renderer: Option<&(dyn crate::math::MathRenderer + Send + Sync)>,
    ) -> String {
        flowchart_label_html_with_prepared_label(
            label,
            label_type,
            config,
            math_renderer,
            Default::default(),
        )
        .into_owned()
    }

    #[derive(Debug)]
    struct ReentryProbe<'a>(&'a AtomicUsize);

    impl crate::math::MathRenderer for ReentryProbe<'_> {
        fn render_html_label(
            &self,
            _text: &str,
            _config: &merman_model::presentation::PresentationConfig,
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
            &merman_model::presentation::PresentationConfig::default(),
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
            let config = crate::test_support::presentation_config(serde_json::json!({
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
                    merman_render_core::text::mermaid_xhtml_label_text_content(&output).as_deref(),
                    Some(text)
                );
            }
        }
    }

    #[test]
    fn markdown_html_output_decodes_mermaid_entities_before_html_projection() {
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
            merman_render_core::text::mermaid_xhtml_label_text_content(&output).as_deref(),
            Some("Before AVAVAV after")
        );
    }

    #[test]
    fn html_image_prefix_matching_preserves_unicode_and_malformed_tags() {
        for security_level in ["strict", "loose", "antiscript", "sandbox"] {
            let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
        let config = crate::test_support::presentation_config(serde_json::json!({
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
