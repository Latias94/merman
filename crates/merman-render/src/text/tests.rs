use super::*;

#[test]
fn html_br_trims_trailing_space_before_break_for_flowchart_labels() {
    let plain =
        crate::flowchart::flowchart_label_plain_text_for_layout("Hexagon <br> end", "text", true);
    assert_eq!(plain, "Hexagon\nend");

    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let m = measurer.measure_wrapped(&plain, &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(m.width, 60.984375);
    assert_eq!(m.height, 48.0);
}

#[test]
fn markdown_strong_width_matches_flowchart_table() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let regular_html = measurer.measure_wrapped("Two", &style, Some(200.0), WrapMode::HtmlLike);
    assert_eq!(regular_html.width, 27.578125);

    let strong_html = measure_markdown_with_flowchart_bold_deltas(
        &measurer,
        "**Two**",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(strong_html.width, 30.109375);

    let regular_svg = measurer.measure_wrapped("Two", &style, Some(200.0), WrapMode::SvgLike);
    assert_eq!(regular_svg.width, 28.984375);

    let strong_svg = measure_markdown_with_flowchart_bold_deltas(
        &measurer,
        "**Two**",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
    );
    // The whole point of `measure_markdown_with_flowchart_bold_deltas` is that `**...**` uses
    // the flowchart bold delta table consistently across wrap modes.
    assert_eq!(
        strong_svg.width - regular_svg.width,
        strong_html.width - regular_html.width
    );
    assert_eq!(strong_svg.width, 31.515625);
}

#[test]
fn flowchart_html_unwrapped_width_matches_upstream_at_30px() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_bigger_font_from_classes_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 30.0,
        font_weight: None,
    };

    let m = measurer.measure_wrapped("I am a circle", &style, None, WrapMode::HtmlLike);
    assert_eq!(m.width, 167.03125);
    assert_eq!(m.height, 45.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn flowchart_html_fontawesome_icon_width_matches_upstream() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_icons_in_edge_labels_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let html = "<p><i class=\"fa fa-car\"></i> Car</p>";
    let m = measure_html_with_flowchart_bold_deltas(
        &measurer,
        html,
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(m.width, 45.015625);
    assert_eq!(m.height, 24.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn flowchart_label_metrics_for_layout_fontawesome_matches_upstream() {
    // Mermaid upstream fixture:
    // fixtures/upstream-svgs/flowchart/upstream_flowchart_v2_icons_in_edge_labels_spec.svg
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let cfg = merman_core::MermaidConfig::default();

    let m = crate::flowchart::flowchart_label_metrics_for_layout(
        &measurer,
        "fa:fa-car Car",
        "text",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
        &cfg,
        None,
    );
    assert_eq!(m.width, 45.015625);
    assert_eq!(m.height, 24.0);
    assert_eq!(m.line_count, 1);
}

#[test]
fn sequence_svg_overrides_keep_literal_br_with_backslash_t_single_line() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    // Mermaid `lineBreakRegex` should not treat this as a `<br>` break because `\\t` is a
    // literal backslash + `t`, not whitespace.
    let text = "multiline<br \\t/>text";
    let m = measurer.measure_wrapped(text, &style, None, WrapMode::SvgLikeSingleRun);
    assert_eq!(m.line_count, 1);
    assert_eq!(m.width, 132.0);
}

#[test]
fn wrap_label_like_mermaid_does_not_split_escaped_br() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif;".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let lines =
        wrap_label_like_mermaid_lines("multiline<br>using #lt;br#gt;", &measurer, &style, 10_000.0);
    assert_eq!(
        lines,
        vec!["multiline".to_string(), "using #lt;br#gt;".to_string()],
        "wrapLabel should short-circuit when explicit `<br>` breaks are present, and must not treat escaped `#lt;br#gt;` as a break"
    );
}

#[test]
fn markdown_underscore_delimiters_match_mermaid() {
    use MermaidMarkdownWordType::*;

    assert_eq!(
        mermaid_markdown_to_lines("`a__b`", true),
        vec![vec![("a__b".to_string(), Normal)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`_a_b_`", true),
        vec![vec![("a_b".to_string(), Em)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`_a__b_`", true),
        vec![vec![("a__b".to_string(), Em)]]
    );
    assert_eq!(
        mermaid_markdown_to_lines("`__a__`", true),
        vec![vec![("a".to_string(), Strong)]]
    );
}

#[test]
fn markdown_inline_code_suppresses_emphasis_delimiters() {
    use MermaidMarkdownWordType::*;

    // Mermaid CLI baselines (class diagram HTML labels) preserve backticks and do not interpret
    // `**...**` inside them as strong/emphasis.
    assert_eq!(
        mermaid_markdown_to_lines("inline: `**not bold**`", true),
        vec![vec![
            ("inline:".to_string(), Normal),
            ("`**not".to_string(), Normal),
            ("bold**`".to_string(), Normal),
        ]]
    );
}

#[test]
fn flowchart_label_metrics_for_layout_measures_markdown_inline_html_like_mermaid() {
    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };
    let cfg = merman_core::MermaidConfig::default();
    let markdown = "This is **bold** </br>and <strong>strong</strong>";
    assert!(mermaid_markdown_contains_html_tags(markdown));

    let html = mermaid_markdown_to_html_label_fragment(markdown, true);
    let html_metrics = measure_html_with_flowchart_bold_deltas(
        &measurer,
        &html,
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
    );
    assert_eq!(html_metrics.width, 82.09375);
    assert_eq!(html_metrics.height, 48.0);
    assert_eq!(html_metrics.line_count, 2);

    let metrics = crate::flowchart::flowchart_label_metrics_for_layout(
        &measurer,
        markdown,
        "markdown",
        &style,
        Some(200.0),
        WrapMode::HtmlLike,
        &cfg,
        None,
    );
    assert_eq!(metrics.width, 82.09375);
    assert_eq!(metrics.height, 48.0);
    assert_eq!(metrics.line_count, 2);
}

#[test]
fn markdown_svg_wrapping_keeps_raw_html_tags_literal_but_wraps_like_mermaid() {
    use MermaidMarkdownWordType::*;

    let measurer = VendoredFontMetricsTextMeasurer::default();
    let style = TextStyle {
        font_family: Some("\"trebuchet ms\", verdana, arial, sans-serif".to_string()),
        font_size: 16.0,
        font_weight: None,
    };

    let lines = mermaid_markdown_to_wrapped_word_lines(
        &measurer,
        "This is **bold** </br>and <strong>strong</strong>",
        &style,
        Some(200.0),
        WrapMode::SvgLike,
    );
    assert_eq!(
        lines,
        vec![
            vec![
                ("This".to_string(), Normal),
                ("is".to_string(), Normal),
                ("bold".to_string(), Strong),
            ],
            vec![
                ("and".to_string(), Normal),
                ("<strong>".to_string(), Normal),
                ("strong".to_string(), Normal),
            ],
            vec![("</strong>".to_string(), Normal)],
        ]
    );
}

#[test]
fn markdown_html_label_fragment_collapses_mixed_list_blocks_like_browser_dom() {
    let input = "Hello\n  - l1\n  - l2";
    assert!(mermaid_markdown_contains_raw_blocks(input));
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>Hello</p>- l1 - l2"
    );
}

#[test]
fn markdown_xhtml_label_fragment_preserves_inline_br_listish_continuations() {
    let input = "Hello<br/>- l1<br/>- l2";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>Hello<br/>- l1<br/>- l2</p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_normalizes_raw_br_variants() {
    let input = "Hello<br>world";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>Hello<br/>world</p>"
    );
}

#[test]
fn markdown_html_label_fragment_preserves_inline_code_literals() {
    let input = "inline: `**not bold**`";
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>inline: `**not bold**`</p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_preserves_inline_code_literals() {
    let input = "inline: `**not bold**`";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>inline: `**not bold**`</p>"
    );
}

#[test]
fn markdown_html_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
    let input = "+inline: **bold*";
    assert_eq!(
        mermaid_markdown_to_html_label_fragment(input, true),
        "<p>+inline: *<em>bold</em></p>"
    );
}

#[test]
fn markdown_xhtml_label_fragment_reinterprets_partial_star_strong_like_mermaid() {
    let input = "+inline: **bold*";
    assert_eq!(
        mermaid_markdown_to_xhtml_label_fragment(input, true),
        "<p>+inline: *<em>bold</em></p>"
    );
}
